use std::env;

use anyhow::Result;
use clap::{Parser, Subcommand};
use localtunnel_client::{broadcast, open_tunnel, ClientConfig};
use localtunnel_server::{start, AuthMode, ServerConfig};
use tokio::signal;

mod config;

#[derive(Parser)]
#[clap(author, version, about)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Builds connection between remote proxy server and local api.
    Client {
        /// Address of proxy server
        #[clap(long, default_value = "https://localtunnel.me")]
        host: String,
        /// Subdomain of the proxied url. Optional; a random one will be assigned when omitted.
        #[clap(long)]
        subdomain: Option<String>,
        /// The local host to expose.
        #[clap(long, default_value = "127.0.0.1")]
        local_host: String,
        /// The local port to expose.
        #[clap(short, long)]
        port: u16,
        /// Max connections allowed to server.
        #[clap(long, default_value = "10")]
        max_conn: u8,
        #[clap(long)]
        credential: Option<String>,
    },

    /// Starts proxy server to accept user connections and proxy setup connection.
    Server {
        /// Domain name of the proxy server, required if use subdomain like lt.example.com.
        #[clap(long)]
        domain: String,
        /// The port to accept initialize proxy endpoint.
        #[clap(short, long, default_value = "3000")]
        port: u16,
        /// The flag to indicate proxy over https.
        #[clap(long)]
        secure: bool,
        /// Maximum number of tcp sockets each client to establish at one time.
        #[clap(long, default_value = "10")]
        max_sockets: u8,
        /// The port to accept user request for proxying.
        #[clap(long, default_value = "3001")]
        proxy_port: u16,
        #[clap(long)]
        auth_mode: AuthMode,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    config::setup();
    log::info!("Run localtunnel CLI!");

    let command = parse_cli().command;

    match command {
        Command::Client {
            host,
            subdomain,
            local_host,
            port,
            max_conn,
            credential,
        } => {
            let (notify_shutdown, _) = broadcast::channel(1);
            let config = ClientConfig {
                server: Some(host),
                subdomain,
                local_host: Some(local_host),
                local_port: port,
                shutdown_signal: notify_shutdown.clone(),
                max_conn,
                credential,
            };
            let result = open_tunnel(config).await?;
            log::info!("Tunnel url: {:?}", result);
            println!("Tunnel url: {}", result);

            signal::ctrl_c().await?;
            log::info!("Quit");
        }
        Command::Server {
            domain,
            port,
            secure,
            max_sockets,
            proxy_port,
            auth_mode,
        } => {
            let config = ServerConfig {
                domain,
                api_port: port,
                secure,
                max_sockets,
                proxy_port,
                auth_mode,
            };
            start(config).await?;
        }
    }

    Ok(())
}

fn parse_cli() -> Cli {
    let args = env::args().collect::<Vec<_>>();
    Cli::parse_from(cli_args_with_default_subcommand(args))
}

fn cli_args_with_default_subcommand(mut args: Vec<String>) -> Vec<String> {
    let needs_default_client = match args.get(1).map(|s| s.as_str()) {
        None => true,
        Some("-h") | Some("--help") | Some("-V") | Some("--version") => false,
        Some(arg) => arg.starts_with('-'),
    };

    if needs_default_client {
        args.insert(1, "client".to_string());
    }

    args
}
