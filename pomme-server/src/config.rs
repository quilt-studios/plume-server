use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Dedicated server command-line configuration.
#[derive(Debug, Clone, Parser)]
#[command(name = "pomme-server", about = "Pomme dedicated server")]
pub struct ServerConfig {
    /// Address on which the server accepts connections.
    #[arg(long, default_value_t = Ipv4Addr::UNSPECIFIED.into())]
    pub host: IpAddr,

    /// TCP port on which the server accepts connections.
    #[arg(long, default_value_t = 25565)]
    pub port: u16,

    /// Directory containing plugin manifest files.
    #[arg(long, default_value = "plugins")]
    pub plugins_dir: PathBuf,

    /// Enable or disable the terminal status renderer.
    #[arg(long, value_enum, default_value_t = Rendering::On)]
    pub rendering: Rendering,
}

impl ServerConfig {
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Rendering {
    On,
    Off,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Rendering, ServerConfig};

    #[test]
    fn rendering_can_be_disabled() {
        let config = ServerConfig::try_parse_from(["server", "--rendering", "off"])
            .expect("valid arguments");
        assert_eq!(config.rendering, Rendering::Off);
        assert_eq!(config.port, 25565);
    }
}
