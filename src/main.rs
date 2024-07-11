use clap::{ArgAction, Parser};
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use rebels::app::App;
use rebels::relayer::Relayer;
use rebels::ssh::server::AppServer;
use rebels::store::store_path;
use rebels::types::AppResult;

#[derive(Parser, Debug)]
#[clap(name="Rebels in the sky", about = "P(lanet)2P(lanet) basketball", author, version, long_about = None)]
struct Args {
    #[clap(long, short = 's', action=ArgAction::Set, help = "Set random seed for team generation")]
    seed: Option<u64>,
    #[clap(long, short='l', action=ArgAction::SetTrue, help = "Run in local mode (disable networking)")]
    disable_network: bool,
    #[clap(long, short='a', action=ArgAction::SetTrue, help = "Disable audio")]
    disable_audio: bool,
    #[clap(long, short='r', action=ArgAction::SetTrue, help = "Reset all save files")]
    reset_world: bool,
    #[clap(long, short='f', action=ArgAction::SetFalse, help = "Disable generating local teams")]
    generate_local_world: bool,
    #[clap(long, short='n', action=ArgAction::SetTrue, help = "Run in network relayer mode (no game)")]
    relayer_mode: bool,
    #[clap(long, short='j', action=ArgAction::SetTrue, help = "Run SSH server")]
    ssh_server: bool,
    #[clap(long, short = 'i', action=ArgAction::Set, help = "Set ip of seed node")]
    seed_ip: Option<String>,
    #[clap(long, short = 'p', action=ArgAction::Set, help = "Set network port")]
    network_port: Option<u16>,
}

#[tokio::main]
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
    if args.ssh_server {
        AppServer::new().run().await?;
    } else if args.relayer_mode {
        Relayer::new().run().await?;
    } else {
        App::new(
            args.seed,
            args.disable_network,
            args.disable_audio,
            args.generate_local_world,
            args.reset_world,
            args.seed_ip,
            args.network_port,
            None,
        )
        .run()
        .await?;
    }

    Ok(())
}
