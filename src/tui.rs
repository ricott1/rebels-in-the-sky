use crate::app::App;
use crate::audio;
use crate::crossterm_event_handler::CrosstermEventHandler;
use crate::ssh::SSHEventHandler;
use crate::ssh::SSHWriterProxy;
use crate::types::{AppResult, Tick};
use crate::ui::ui::Ui;
use crate::world::world::World;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, MouseEvent};
use crossterm::terminal::Clear;
use crossterm::terminal::SetTitle;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use futures::Future;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use ratatui::TerminalOptions;
use ratatui::Viewport;
use std::io::{self};
use std::panic;
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait WriterProxy: io::Write + std::fmt::Debug {
    fn send(&mut self) -> impl std::future::Future<Output = std::io::Result<usize>> + Send {
        async { Ok(0) }
    }
}

impl WriterProxy for io::Stdout {}

#[derive(Clone, Copy, Debug)]
pub enum TerminalEvent {
    Tick { tick: Tick },
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Quit,
}

impl Future for TerminalEvent {
    type Output = Self;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(*self)
    }
}

pub trait EventHandler: Send + Sync {
    fn next(&mut self) -> impl std::future::Future<Output = TerminalEvent> + Send;
    fn simulation_update_interval(&self) -> Tick;
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TuiType {
    Local,
    SSH,
}

#[derive(Debug)]
pub struct Tui<W, E>
where
    W: WriterProxy,
    E: EventHandler,
{
    tui_type: TuiType,
    pub terminal: Terminal<CrosstermBackend<W>>,
    pub events: E,
}

impl Tui<io::Stdout, CrosstermEventHandler> {
    pub fn new_local(events: CrosstermEventHandler) -> AppResult<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        let mut tui = Self {
            tui_type: TuiType::Local,
            terminal,
            events,
        };
        tui.init()?;
        Ok(tui)
    }
}

impl Tui<SSHWriterProxy, SSHEventHandler> {
    pub fn new_ssh(writer: SSHWriterProxy, events: SSHEventHandler) -> AppResult<Self> {
        let backend = CrosstermBackend::new(writer);
        let opts = TerminalOptions {
            viewport: Viewport::Fixed(Rect {
                x: 0,
                y: 0,
                width: 160,
                height: 48,
            }),
        };

        let terminal = Terminal::with_options(backend, opts)?;
        let mut tui = Self {
            tui_type: TuiType::SSH,
            terminal,
            events,
        };

        tui.init()?;
        Ok(tui)
    }
}

impl<W, E> Tui<W, E>
where
    W: WriterProxy,
    E: EventHandler,
{
    fn init(&mut self) -> AppResult<()> {
        if self.tui_type == TuiType::Local {
            terminal::enable_raw_mode()?;
        }

        crossterm::execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            SetTitle("Rebels in the sky"),
            Clear(crossterm::terminal::ClearType::All),
            Hide
        )?;

        // Define a custom panic hook to reset the terminal properties.
        // This way, you won't have your terminal messed up if an unexpected error happens.
        let panic_hook = panic::take_hook();
        if self.tui_type == TuiType::Local {
            panic::set_hook(Box::new(move |panic| {
                Self::reset().expect("failed to reset the terminal");
                panic_hook(panic);
            }));
        }

        Ok(())
    }

    fn reset() -> AppResult<()> {
        crossterm::execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            Clear(crossterm::terminal::ClearType::All),
            Show
        )?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    pub async fn draw(
        &mut self,
        ui: &mut Ui,
        world: &World,
        audio_player: Option<&audio::music_player::MusicPlayer>,
    ) -> AppResult<()> {
        self.terminal
            .draw(|frame| App::render(ui, world, audio_player, frame))?;

        if self.tui_type == TuiType::SSH {
            self.terminal.backend_mut().writer_mut().send().await?;
        }

        Ok(())
    }

    pub fn simulation_update_interval(&self) -> Tick {
        self.events.simulation_update_interval()
    }

    pub fn resize(&mut self, size: (u16, u16)) -> AppResult<()> {
        self.terminal.resize(Rect {
            x: 0,
            y: 0,
            width: size.0,
            height: size.1,
        })?;
        Ok(())
    }

    pub async fn exit(&mut self) -> AppResult<()> {
        crossterm::execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            Clear(crossterm::terminal::ClearType::All),
            Show
        )?;

        if self.tui_type == TuiType::Local {
            terminal::disable_raw_mode()?;
        }

        if self.tui_type == TuiType::SSH {
            self.terminal.backend_mut().writer_mut().send().await?;
        }

        Ok(())
    }
}
