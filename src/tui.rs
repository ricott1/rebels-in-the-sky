#[cfg(feature = "audio")]
use crate::audio;
#[cfg(feature = "ssh")]
use crate::ssh::SSHWriterProxy;
use crate::types::AppResult;
use crate::ui::ui::Ui;
use crate::ui::UI_SCREEN_SIZE;
use crate::world::world::World;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, MouseEvent};
use crossterm::terminal::Clear;
use crossterm::terminal::SetTitle;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use ratatui::TerminalOptions;
use ratatui::Viewport;
use std::io::{self};
use std::panic;
use std::time::{Duration, Instant};

const MAX_DRAW_FPS: u8 = 30;

pub trait WriterProxy: io::Write + std::fmt::Debug {
    fn send(&mut self) -> impl std::future::Future<Output = std::io::Result<usize>> + Send {
        async { Ok(0) }
    }
}

impl WriterProxy for io::Stdout {}

#[derive(Debug)]
pub struct DummyWriter {}

impl io::Write for DummyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl WriterProxy for DummyWriter {}

#[derive(Clone, Copy, Debug)]
pub enum TerminalEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TuiType {
    Local,
    #[cfg(feature = "ssh")]
    SSH,
    Dummy,
}

#[derive(Debug)]
pub struct Tui<W>
where
    W: WriterProxy,
{
    tui_type: TuiType,
    terminal: Terminal<CrosstermBackend<W>>,
    last_draw: Instant,
    min_duration_between_draws: Duration,
}

impl Tui<io::Stdout> {
    pub fn new_local() -> AppResult<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        let mut tui = Self {
            tui_type: TuiType::Local,
            terminal,
            last_draw: Instant::now(),
            min_duration_between_draws: Duration::from_secs_f32(1.0 / MAX_DRAW_FPS as f32),
        };
        tui.init()?;
        Ok(tui)
    }
}

#[cfg(feature = "ssh")]
impl Tui<SSHWriterProxy> {
    pub fn new_ssh(writer: SSHWriterProxy) -> AppResult<Self> {
        let backend = CrosstermBackend::new(writer);
        let opts = TerminalOptions {
            viewport: Viewport::Fixed(Rect {
                x: 0,
                y: 0,
                width: UI_SCREEN_SIZE.0,
                height: UI_SCREEN_SIZE.1,
            }),
        };

        let terminal = Terminal::with_options(backend, opts)?;
        let mut tui = Self {
            tui_type: TuiType::SSH,
            terminal,
            last_draw: Instant::now(),
            min_duration_between_draws: Duration::from_secs_f32(1.0 / MAX_DRAW_FPS as f32),
        };

        tui.init()?;
        Ok(tui)
    }
}

impl Tui<DummyWriter> {
    pub fn new_dummy() -> AppResult<Self> {
        let writer = DummyWriter {};
        let backend = CrosstermBackend::new(writer);
        let opts = TerminalOptions {
            viewport: Viewport::Fixed(Rect {
                x: 0,
                y: 0,
                width: UI_SCREEN_SIZE.0,
                height: UI_SCREEN_SIZE.1,
            }),
        };

        let terminal = Terminal::with_options(backend, opts)?;
        let mut tui = Self {
            tui_type: TuiType::Dummy,
            terminal,
            last_draw: Instant::now(),
            min_duration_between_draws: Duration::default(),
        };

        tui.init()?;
        Ok(tui)
    }
}

impl<W> Tui<W>
where
    W: WriterProxy,
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
        if self.tui_type == TuiType::Local {
            let panic_hook = panic::take_hook();
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
            SetTitle(""),
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
        #[cfg(feature = "audio")] audio_player: Option<&audio::music_player::MusicPlayer>,
    ) -> AppResult<()> {
        if self.tui_type == TuiType::Dummy {
            return Ok(());
        }

        // Draw at most at MAX_FPS
        if self.last_draw.elapsed() >= self.min_duration_between_draws {
            self.terminal.draw(|frame| {
                ui.render(
                    frame,
                    world,
                    #[cfg(feature = "audio")]
                    audio_player,
                )
            })?;

            #[cfg(feature = "ssh")]
            if self.tui_type == TuiType::SSH {
                self.terminal.backend_mut().writer_mut().send().await?;
            }

            self.last_draw = Instant::now();
        }

        Ok(())
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

        #[cfg(feature = "ssh")]
        if self.tui_type == TuiType::SSH {
            self.terminal.backend_mut().writer_mut().send().await?;
        }

        Ok(())
    }
}
