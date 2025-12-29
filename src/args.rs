use crate::network::constants::DEFAULT_NETWORK_PORT;
use clap::{ArgAction, Parser};

#[derive(PartialEq)]
pub enum AppMode {
    Game,
    #[cfg(feature = "ssh")]
    SSHServer,
    #[cfg(feature = "relayer")]
    Relayer,
}

#[derive(Parser, Debug)]
#[clap(name="Rebels in the sky", about = "P(lanet)2P(lanet) basketball", author, version, long_about = None)]
pub struct AppArgs {
    #[clap(long,  action=ArgAction::Set, help = "Set random seed for team generation")]
    pub random_seed: Option<u64>,
    #[clap(long, short='l', action=ArgAction::SetTrue, help = "Disable networking")]
    disable_network: bool,
    #[cfg(feature = "audio")]
    #[clap(long, short='a', action=ArgAction::SetTrue, help = "Disable audio")]
    disable_audio: bool,
    #[clap(long, short='r', action=ArgAction::SetTrue, help = "Reset all save files")]
    pub reset_world: bool,
    #[clap(long="disable_local_world", short='f', action=ArgAction::SetFalse, help = "Disable generating local teams")]
    pub generate_local_world: bool,
    #[clap(long, short='u', action=ArgAction::SetTrue, help = "Disable UI and input reader")]
    disable_ui: bool,
    #[cfg(feature = "relayer")]
    #[clap(long, short='n', action=ArgAction::SetTrue, help = "Run a network relayer")]
    relayer_mode: bool,
    #[cfg(feature = "ssh")]
    #[clap(long, short='j', action=ArgAction::SetTrue, help = "Run SSH server")]
    ssh_server: bool,
    #[clap(long, short = 'i', action=ArgAction::Set, help = "Set ip of seed node")]
    pub seed_node_ip: Option<String>,
    #[clap(long, short = 'p', action=ArgAction::Set, help = "Set network port")]
    network_port: Option<u16>,
    #[clap(long, action=ArgAction::Set, help = "Set store prefix")]
    store_prefix: Option<String>,
    #[clap(long, action=ArgAction::SetTrue, help = "Save game to uncompressed json")]
    pub store_uncompressed: bool,
    #[clap(long, short = 'q', action=ArgAction::Set, help = "Set auto quit after value in seconds")]
    pub auto_quit_after: Option<u64>,
}

impl AppArgs {
    pub fn ssh_client(
        store_prefix: Option<String>,
        network_port: Option<u16>,
        auto_quit_after: Option<u64>,
    ) -> Self {
        Self {
            random_seed: None,
            disable_network: true,
            #[cfg(feature = "audio")]
            disable_audio: true,
            reset_world: false,
            generate_local_world: true,
            disable_ui: false,
            #[cfg(feature = "relayer")]
            relayer_mode: false,
            #[cfg(feature = "ssh")]
            ssh_server: false,
            seed_node_ip: None,
            network_port,
            store_prefix,
            store_uncompressed: false,
            auto_quit_after,
        }
    }
    pub fn test() -> Self {
        // seed: Option<u64>,
        // disable_network: bool,
        // #[cfg(feature = "audio")] disable_audio: bool,
        // generate_local_world: bool,
        // reset_world: bool,
        // seed_node_ip: Option<String>,
        // store_prefix: Option<String>,
        // store_uncompressed: bool,
        Self {
            random_seed: Some(0),
            disable_network: true,
            #[cfg(feature = "audio")]
            disable_audio: true,
            reset_world: false,
            generate_local_world: true,
            disable_ui: false,
            #[cfg(feature = "relayer")]
            relayer_mode: false,
            #[cfg(feature = "ssh")]
            ssh_server: false,
            seed_node_ip: None,
            network_port: None,
            store_prefix: None,
            store_uncompressed: false,
            auto_quit_after: None,
        }
    }

    pub fn app_mode(&self) -> AppMode {
        #[cfg(feature = "ssh")]
        if self.ssh_server {
            return AppMode::SSHServer;
        }
        #[cfg(feature = "relayer")]
        if self.relayer_mode {
            return AppMode::Relayer;
        }

        AppMode::Game
    }
    #[cfg(feature = "audio")]
    pub fn is_audio_disabled(&self) -> bool {
        self.disable_audio || self.disable_ui
    }

    pub fn is_ui_disabled(&self) -> bool {
        self.disable_ui
    }

    pub fn is_network_disabled(&self) -> bool {
        self.disable_network
    }

    pub fn network_port(&self) -> Option<u16> {
        if self.disable_network {
            None
        } else {
            Some(self.network_port.unwrap_or(DEFAULT_NETWORK_PORT))
        }
    }

    pub fn store_prefix(&self) -> &str {
        if let Some(prefix) = self.store_prefix.as_ref() {
            prefix
        } else {
            "local"
        }
    }
}
