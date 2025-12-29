use clap::Parser;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use rebels::app::App;
use rebels::args::AppArgs;
#[cfg(any(feature = "relayer", feature = "ssh"))]
use rebels::args::AppMode;
#[cfg(feature = "relayer")]
use rebels::relayer::Relayer;
#[cfg(feature = "ssh")]
use rebels::ssh::AppServer;
use rebels::store::store_path;
use rebels::tui::Tui;
use rebels::types::AppResult;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> AppResult<()> {
    let logfile_path = store_path("rebels.log")?;
    let logfile = FileAppender::builder()
        .append(false)
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build(logfile_path)?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;
    log4rs::init_config(config)?;

    let args = AppArgs::parse();

    #[cfg(any(feature = "relayer", feature = "ssh"))]
    let mode = args.app_mode();

    #[cfg(feature = "ssh")]
    if mode == AppMode::SSHServer {
        return AppServer::new().run().await;
    }

    #[cfg(feature = "relayer")]
    if mode == AppMode::Relayer {
        return Relayer::new().run().await;
    }

    let ui_disabled = args.is_ui_disabled();
    let mut app = App::new(args)?;

    if ui_disabled {
        let tui = Tui::new_dummy()?;
        app.run(tui).await?;
    } else {
        let tui = Tui::new_local()?;
        app.run(tui).await?;
    };

    Ok(())
}
