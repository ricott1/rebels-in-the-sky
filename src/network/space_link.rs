use crate::{
    app::AppEvent,
    space_adventure::wire::{decode, encode, LinkSender, SessionMessage, MAX_FRAME_BYTES},
    types::AppResult,
};
use anyhow::anyhow;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::{PeerId, StreamProtocol};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub type LinkId = u64;

pub const SPACE_ADVENTURE_PROTOCOL: StreamProtocol =
    StreamProtocol::new("/rebels/space-adventure/1");
const SNAPSHOT_QUEUE: usize = 4;

static NEXT_LINK_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_link_id() -> LinkId {
    NEXT_LINK_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug)]
pub enum SpaceLinkEvent {
    Opened {
        link_id: LinkId,
        peer_id: PeerId,
        handle: SpaceLinkHandle,
        inbound: bool,
    },
    Message {
        link_id: LinkId,
        message: SessionMessage,
    },
    Closed {
        link_id: LinkId,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct SpaceLinkHandle {
    control_tx: mpsc::UnboundedSender<SessionMessage>,
    snapshot_tx: mpsc::Sender<SessionMessage>,
}

impl LinkSender for SpaceLinkHandle {
    fn send_control(&self, message: SessionMessage) -> bool {
        self.control_tx.send(message).is_ok()
    }

    fn try_send_snapshot(&self, message: SessionMessage) -> bool {
        self.snapshot_tx.try_send(message).is_ok()
    }
}

#[cfg(test)]
impl SpaceLinkHandle {
    pub fn test_pair() -> (
        Self,
        mpsc::UnboundedReceiver<SessionMessage>,
        mpsc::Receiver<SessionMessage>,
    ) {
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (snapshot_tx, snapshot_rx) = mpsc::channel(SNAPSHOT_QUEUE);
        (
            Self {
                control_tx,
                snapshot_tx,
            },
            control_rx,
            snapshot_rx,
        )
    }
}

pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    message: &SessionMessage,
) -> AppResult<()> {
    let payload = encode(message)?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(anyhow!("Frame too large: {} bytes", payload.len()));
    }
    writer
        .write_all(&(payload.len() as u32).to_le_bytes())
        .await?;
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> AppResult<SessionMessage> {
    let mut length = [0u8; 4];
    reader.read_exact(&mut length).await?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(anyhow!("Frame too large: {length} bytes"));
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    decode(&payload)
}

pub fn spawn_link<S>(stream: S, link_id: LinkId, events: mpsc::Sender<AppEvent>) -> SpaceLinkHandle
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, mut writer) = stream.split();
    let (control_tx, mut control_rx) = mpsc::unbounded_channel();
    let (snapshot_tx, mut snapshot_rx) = mpsc::channel(SNAPSHOT_QUEUE);
    let token = CancellationToken::new();

    let writer_token = token.clone();
    tokio::spawn(async move {
        loop {
            let message = tokio::select! {
                biased;
                _ = writer_token.cancelled() => break,
                message = control_rx.recv() => message,
                Some(message) = snapshot_rx.recv() => Some(message),
            };
            let Some(message) = message else {
                break;
            };
            if write_frame(&mut writer, &message).await.is_err() {
                break;
            }
        }
        let _ = writer.close().await;
        writer_token.cancel();
    });

    tokio::spawn(async move {
        let reason = loop {
            tokio::select! {
                _ = token.cancelled() => break "closed".to_string(),
                frame = read_frame(&mut reader) => match frame {
                    Ok(message) => {
                        let event = AppEvent::SpaceLink(SpaceLinkEvent::Message { link_id, message });
                        if events.send(event).await.is_err() {
                            break "app stopped".to_string();
                        }
                    }
                    Err(err) => break err.to_string(),
                },
            }
        };
        token.cancel();
        let _ = events
            .send(AppEvent::SpaceLink(SpaceLinkEvent::Closed { link_id, reason }))
            .await;
    });

    SpaceLinkHandle {
        control_tx,
        snapshot_tx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::io::Cursor;
    use tokio_util::compat::TokioAsyncReadCompatExt;

    #[tokio::test]
    async fn test_frames_round_trip() -> AppResult<()> {
        let mut buffer = Cursor::new(Vec::new());
        write_frame(&mut buffer, &SessionMessage::Heartbeat).await?;
        write_frame(&mut buffer, &SessionMessage::Leave).await?;
        buffer.set_position(0);
        assert_eq!(read_frame(&mut buffer).await?, SessionMessage::Heartbeat);
        assert_eq!(read_frame(&mut buffer).await?, SessionMessage::Leave);
        Ok(())
    }

    #[tokio::test]
    async fn test_oversized_frame_is_rejected() {
        let mut bytes = ((MAX_FRAME_BYTES + 1) as u32).to_le_bytes().to_vec();
        bytes.extend([0u8; 8]);
        assert!(read_frame(&mut Cursor::new(bytes)).await.is_err());
    }

    #[tokio::test]
    async fn test_linked_handles_exchange_messages_and_close() -> AppResult<()> {
        let (a, b) = tokio::io::duplex(1 << 16);
        let (a_events, _a_rx) = mpsc::channel(16);
        let (b_events, mut b_rx) = mpsc::channel(16);
        let a_handle = spawn_link(a.compat(), 1, a_events);
        let _b_handle = spawn_link(b.compat(), 2, b_events);

        assert!(a_handle.send_control(SessionMessage::Heartbeat));
        assert!(a_handle.try_send_snapshot(SessionMessage::Leave));
        let mut received = vec![];
        while received.len() < 2 {
            match b_rx.recv().await {
                Some(AppEvent::SpaceLink(SpaceLinkEvent::Message {
                    link_id: 2,
                    message,
                })) => received.push(message),
                other => panic!("unexpected event {other:?}"),
            }
        }
        assert!(received.contains(&SessionMessage::Heartbeat));
        assert!(received.contains(&SessionMessage::Leave));

        drop(a_handle);
        assert!(matches!(
            b_rx.recv().await,
            Some(AppEvent::SpaceLink(SpaceLinkEvent::Closed { link_id: 2, .. }))
        ));
        Ok(())
    }
}
