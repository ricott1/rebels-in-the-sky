use clap::{ArgAction, Parser};
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use rebels::app::App;
use rebels::network::constants::DEFAULT_PORT;
use rebels::store::store_path;
use rebels::tui::Tui;
use rebels::types::AppResult;

#[cfg(feature = "relayer")]
use rebels::relayer::Relayer;

#[cfg(feature = "ssh")]
use rebels::ssh::AppServer;

#[derive(Parser, Debug)]
#[clap(name="Rebels in the sky", about = "P(lanet)2P(lanet) basketball", author, version, long_about = None)]
struct Args {
    #[clap(long,  action=ArgAction::Set, help = "Set random seed for team generation")]
    seed: Option<u64>,
    #[clap(long, short='l', action=ArgAction::SetTrue, help = "Disable networking")]
    disable_network: bool,
    #[clap(long, short='a', action=ArgAction::SetTrue, help = "Disable audio")]
    disable_audio: bool,
    #[clap(long, short='r', action=ArgAction::SetTrue, help = "Reset all save files")]
    reset_world: bool,
    #[clap(long="disable_local_world", short='f', action=ArgAction::SetFalse, help = "Disable generating local teams")]
    generate_local_world: bool,
    #[clap(long, short='u', action=ArgAction::SetTrue, help = "Disable UI and input reader")]
    disable_ui: bool,
    #[cfg(feature = "relayer")]
    #[clap(long, short='n', action=ArgAction::SetTrue, help = "Run a network relayer")]
    relayer_mode: bool,
    #[cfg(feature = "ssh")]
    #[clap(long, short='j', action=ArgAction::SetTrue, help = "Run SSH server")]
    ssh_server: bool,
    #[clap(long, short = 'i', action=ArgAction::Set, help = "Set ip of seed node")]
    seed_ip: Option<String>,
    #[clap(long, short = 'p', action=ArgAction::Set, help = "Set network port")]
    network_port: Option<u16>,
    #[clap(long, action=ArgAction::Set, help = "Set store prefix")]
    store_prefix: Option<String>,
    #[clap(long, action=ArgAction::Set, help = "Set target FPS")]
    target_fps: Option<u8>,
}

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
    let args = Args::parse();

    if let Some(fps) = args.target_fps {
        if fps == 0 {
            eprintln!(
                "error: invalid value '{fps}' for '--target-fps <TARGET_FPS>': {fps} is not in 1..=255"
            );
            return Ok(());
        }
    }

    #[cfg(feature = "ssh")]
    if args.ssh_server {
        return AppServer::new().run().await;
    }

    #[cfg(feature = "relayer")]
    if args.relayer_mode {
        return Relayer::new().run().await;
    }

    let network_port = if args.disable_network {
        None
    } else {
        Some(args.network_port.unwrap_or(DEFAULT_PORT))
    };

    let disable_audio = args.disable_audio || args.disable_ui;

    let mut app = App::new(
        args.seed,
        args.disable_network,
        disable_audio,
        args.generate_local_world,
        args.reset_world,
        args.seed_ip,
        network_port,
        args.store_prefix,
    );

    if args.disable_ui {
        app.load_world();
    }

    if args.disable_ui {
        let tui = Tui::new_dummy()?;
        app.run(tui).await?;
    } else {
        let tui = Tui::new_local(args.target_fps)?;
        app.run(tui).await?;
    };

    Ok(())
}
