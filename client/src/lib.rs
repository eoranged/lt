use std::sync::Arc;
use tokio::sync::Semaphore;

use anyhow::Result;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use socket2::{SockRef, TcpKeepalive};
use tokio::io;
use tokio::net::TcpStream;
pub use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};

pub const PROXY_SERVER: &str = "https://localtunnel.me";
pub const LOCAL_HOST: &str = "127.0.0.1";

// See https://tldp.org/HOWTO/html_single/TCP-Keepalive-HOWTO to understand how keepalive work.
const TCP_KEEPALIVE_TIME: Duration = Duration::from_secs(30);
const TCP_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "windows"))]
const TCP_KEEPALIVE_RETRIES: u32 = 5;

#[derive(Debug, Serialize, Deserialize)]
struct ProxyResponse {
    id: String,
    port: u16,
    #[serde(default = "default_max_conn_count")]
    max_conn_count: u8,
    url: String,
    #[serde(default)]
    cached_url: Option<String>,
    #[serde(default)]
    ip: Option<String>,
}

const fn default_max_conn_count() -> u8 {
    1
}

/// The server detail for client to connect
#[derive(Clone, Debug)]
pub struct TunnelServerInfo {
    pub remote_host: String,
    pub remote_port: u16,
    pub remote_ip: Option<String>,
    pub max_conn_count: u8,
    pub url: String,
    pub cached_url: Option<String>,
}

pub struct ClientConfig {
    pub server: Option<String>,
    pub subdomain: Option<String>,
    pub local_host: Option<String>,
    pub local_port: u16,
    pub shutdown_signal: broadcast::Sender<()>,
    pub max_conn: u8,
    pub credential: Option<String>,
}

/// Open tunnels directly between server and localhost
pub async fn open_tunnel(config: ClientConfig) -> Result<String> {
    let ClientConfig {
        server,
        subdomain,
        local_host,
        local_port,
        shutdown_signal,
        max_conn,
        credential,
    } = config;
    let tunnel_info = get_tunnel_endpoint(server.clone(), subdomain, credential).await?;

    // TODO check the connect is failed and restart the proxy.
    tunnel_to_endpoint(
        tunnel_info.clone(),
        local_host,
        local_port,
        shutdown_signal,
        max_conn,
    )
    .await;

    if let Some(cached_url) = &tunnel_info.cached_url {
        log::info!("Cached tunnel url: {}", cached_url);
    }

    // Try to fetch the tunnel password
    fetch_tunnel_password(server).await;

    Ok(tunnel_info.url)
}

async fn get_tunnel_endpoint(
    server: Option<String>,
    subdomain: Option<String>,
    credential: Option<String>,
) -> Result<TunnelServerInfo> {
    let server = server
        .as_deref()
        .unwrap_or(PROXY_SERVER)
        .trim_end_matches('/');
    let assigned_domain = subdomain.as_deref().unwrap_or("?new");
    let mut uri = format!("{}/{}", server, assigned_domain);
    if let Some(credential) = credential {
        let separator = if uri.contains('?') { '&' } else { '?' };
        uri = format!("{}{}credential={}", uri, separator, credential);
    }
    log::info!("Request for assign domain: {}", uri);

    let resp = reqwest::get(&uri).await?.json::<ProxyResponse>().await?;
    log::info!("Response from server: {:#?}", resp);

    let remote_host = parse_remote_host(server).unwrap_or_else(|| LOCAL_HOST.to_string());
    let remote_ip = resp.ip.clone();

    let tunnel_info = TunnelServerInfo {
        remote_host,
        remote_port: resp.port,
        remote_ip,
        max_conn_count: resp.max_conn_count,
        url: resp.url,
        cached_url: resp.cached_url,
    };

    Ok(tunnel_info)
}

async fn fetch_tunnel_password(server: Option<String>) {
    let server = server
        .as_deref()
        .unwrap_or(PROXY_SERVER)
        .trim_end_matches('/');
    let password_uri = format!("{}/mytunnelpassword", server);

    match reqwest::get(&password_uri).await {
        Ok(resp) => match resp.text().await {
            Ok(password) => {
                println!("Tunnel password: {}", password);
            }
            Err(err) => {
                log::info!("Failed to read tunnel password response: {:?}", err);
            }
        },
        Err(err) => {
            log::info!("Failed to fetch tunnel password: {:?}", err);
        }
    }
}

async fn tunnel_to_endpoint(
    server: TunnelServerInfo,
    local_host: Option<String>,
    local_port: u16,
    shutdown_signal: broadcast::Sender<()>,
    max_conn: u8,
) {
    log::info!("Tunnel server info: {:?}", server);
    let remote_host = server.remote_host.clone();
    let remote_ip = server.remote_ip.clone();
    let server_port = server.remote_port;
    let local_host = local_host.unwrap_or(LOCAL_HOST.to_string());

    let count = std::cmp::min(server.max_conn_count, max_conn);
    log::info!("Max connection count: {}", count);
    let limit_connection = Arc::new(Semaphore::new(count.into()));

    let mut shutdown_receiver = shutdown_signal.subscribe();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = limit_connection.clone().acquire_owned() => {
                    let permit = match res {
                        Ok(permit) => permit,
                        Err(err) => {
                            log::error!("Acquire limit connection failed: {:?}", err);
                            return;
                        },
                    };
                    let remote_host = remote_host.clone();
                    let remote_ip = remote_ip.clone();
                    let local_host = local_host.clone();

                    let mut shutdown_receiver = shutdown_signal.subscribe();

                    tokio::spawn(async move {
                        log::info!("Create a new proxy connection.");
                        tokio::select! {
                            res = handle_connection(remote_host.clone(), remote_ip.clone(), server_port, local_host, local_port) => {
                                match res {
                                    Ok(_) => log::info!("Connection result: {:?}", res),
                                    Err(err) => {
                                        log::error!("Failed to connect to proxy or local server: {:?}", err);
                                        sleep(Duration::from_secs(10)).await;
                                    }
                                }
                            }
                            _ = shutdown_receiver.recv() => {
                                log::info!("Shutting down the connection immediately");
                            }
                        }

                        drop(permit);
                    });
                }
                _ = shutdown_receiver.recv() => {
                    log::info!("Shuttign down the loop immediately");
                    return;
                }
            };
        }
    });
}

async fn handle_connection(
    remote_host: String,
    remote_ip: Option<String>,
    remote_port: u16,
    local_host: String,
    local_port: u16,
) -> Result<()> {
    let target_host = remote_ip.unwrap_or(remote_host);
    log::debug!("Connect to remote: {}, {}", target_host, remote_port);
    let mut remote_stream = TcpStream::connect(format!("{}:{}", target_host, remote_port)).await?;
    log::debug!("Connect to local: {}, {}", local_host, local_port);
    let mut local_stream = TcpStream::connect(format!("{}:{}", local_host, local_port)).await?;

    // configure keepalive on remote socket to early detect network issues and attempt to re-establish the connection.
    let ka = TcpKeepalive::new()
        .with_time(TCP_KEEPALIVE_TIME)
        .with_interval(TCP_KEEPALIVE_INTERVAL);
    #[cfg(not(target_os = "windows"))]
    let ka = ka.with_retries(TCP_KEEPALIVE_RETRIES);
    let sf = SockRef::from(&remote_stream);
    sf.set_tcp_keepalive(&ka)?;

    io::copy_bidirectional(&mut remote_stream, &mut local_stream).await?;
    Ok(())
}

fn parse_remote_host(server: &str) -> Option<String> {
    if let Ok(parsed) = Url::parse(server) {
        if let Some(host) = parsed.host_str() {
            return Some(host.to_string());
        }
    }

    let (_, remainder) = server.split_once("://").unwrap_or(("", server));
    let host = remainder.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}
