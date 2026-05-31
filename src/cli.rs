//! Shared CLI / configuration for MCP servers built on this crate.
//!
//! Consumers flatten [`ServerArgs`] into their own clap parser and hand it to the
//! web/server harness; [`ServerConfig`] is the plain equivalent for callers that
//! configure the harness programmatically (e.g. from environment-derived config)
//! rather than from the command line.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::time::Duration;

/// Default listen addresses: loopback only (IPv4 + IPv6), per the bind-localhost rule.
fn default_listen() -> Vec<IpAddr> {
    vec![
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ]
}

/// Shared server CLI flags: mode selection (`--stdio`/`--sse`/`--web`), listen address,
/// and authentication. Flatten into a consumer's clap parser. Mode is opt-in.
#[derive(Debug, Clone, clap::Args)]
pub struct ServerArgs {
    /// Force MCP over stdio (exclusive). Suppresses logging; errors go to stderr.
    #[arg(long, conflicts_with_all = ["sse", "web"])]
    pub stdio: bool,

    /// Serve the MCP SSE endpoint over HTTP.
    #[arg(long)]
    pub sse: bool,

    /// Serve the web frontend and REST API over HTTP.
    #[arg(long)]
    pub web: bool,

    /// With no mode flag and a non-TTY stdin, wait this many milliseconds for an MCP
    /// client to initialize over stdin before exiting with an error.
    #[arg(long, default_value = "100", value_name = "MS")]
    pub wait: u64,

    /// HTTP port for `--sse`/`--web`.
    #[arg(long, default_value = "8080")]
    pub http_port: u16,

    /// IP address to listen on; repeatable. Defaults to loopback (127.0.0.1 and ::1).
    #[arg(long = "http-listen", value_name = "IP")]
    pub http_listen: Vec<IpAddr>,

    /// Authentication token (Bearer for MCP, Basic for web/API). Enables auth when set.
    #[arg(long, env = "AUTH_TOKEN")]
    pub auth_token: Option<String>,

    /// Directory of static files to serve in `--web` mode.
    #[arg(long)]
    pub static_dir: Option<PathBuf>,
}

impl ServerArgs {
    /// Configured listen addresses, falling back to loopback (v4 + v6) when none given.
    pub fn listen_addrs(&self) -> Vec<IpAddr> {
        if self.http_listen.is_empty() {
            default_listen()
        } else {
            self.http_listen.clone()
        }
    }

    /// `--wait` as a [`Duration`].
    pub fn wait(&self) -> Duration {
        Duration::from_millis(self.wait)
    }
}

/// Programmatic equivalent of [`ServerArgs`] for non-clap callers.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub stdio: bool,
    pub sse: bool,
    pub web: bool,
    pub wait: Duration,
    pub http_port: u16,
    pub http_listen: Vec<IpAddr>,
    pub auth_token: Option<String>,
    pub static_dir: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            stdio: false,
            sse: false,
            web: false,
            wait: Duration::from_millis(100),
            http_port: 8080,
            http_listen: default_listen(),
            auth_token: None,
            static_dir: None,
        }
    }
}

impl From<ServerArgs> for ServerConfig {
    fn from(a: ServerArgs) -> Self {
        Self {
            stdio: a.stdio,
            sse: a.sse,
            web: a.web,
            wait: a.wait(),
            http_port: a.http_port,
            http_listen: a.listen_addrs(),
            auth_token: a.auth_token,
            static_dir: a.static_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listen_defaults_to_loopback_v4_and_v6() {
        let cfg = ServerConfig::default();
        assert!(cfg.http_listen.contains(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(cfg.http_listen.contains(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert_eq!(cfg.http_port, 8080);
    }
}
