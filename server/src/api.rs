use actix_web::{get, web, HttpResponse, Responder};
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::auth::{Auth, CfWorkerStore, PlaintextPassword};
use crate::state::State;
use crate::AuthMode;

#[get("/api/status")]
pub async fn api_status(state: web::Data<State>) -> impl Responder {
    let manager = state.manager.lock().await;

    // Get memory usage using jemalloc stats
    let mem = get_memory_usage();

    let status = ApiStatus {
        tunnels: manager.tunnels,
        mem,
    };

    HttpResponse::Ok().json(status)
}

#[get("/api/tunnels/{id}/status")]
pub async fn api_tunnel_status(
    tunnel_id: web::Path<String>,
    state: web::Data<State>,
) -> impl Responder {
    let manager = state.manager.lock().await;

    match manager.get_client(&tunnel_id) {
        Some(client) => {
            let client = client.lock().await;
            let stats = client.stats().await;
            let status = TunnelStatus {
                connected_sockets: stats.connected_sockets,
            };
            HttpResponse::Ok().json(status)
        }
        None => HttpResponse::NotFound().finish(),
    }
}

fn get_memory_usage() -> MemoryUsage {
    // Try to get memory stats from jemalloc if available, otherwise use simple approximation
    #[cfg(feature = "jemalloc")]
    {
        use jemalloc_ctl::{epoch, stats};

        if let (Ok(e), Ok(allocated), Ok(resident)) = (
            epoch::mib(),
            stats::allocated::mib(),
            stats::resident::mib(),
        ) {
            if let (Ok(_), Ok(allocated_val), Ok(resident_val)) = (
                e.advance(),
                allocated.read(),
                resident.read(),
            ) {
                return MemoryUsage {
                    rss: resident_val,
                    heap_total: allocated_val,
                    heap_used: allocated_val,
                    external: 0,
                };
            }
        }
    }

    // Fallback: basic memory usage
    MemoryUsage {
        rss: 0,
        heap_total: 0,
        heap_used: 0,
        external: 0,
    }
}

async fn validate_credentials(
    endpoint: &web::Path<String>,
    info: &web::Query<AuthInfo>,
    state: &web::Data<State>,
) -> Result<bool, actix_web::Error> {
    if state.auth_mode == AuthMode::NOAUTH {
        return Ok(true);
    }
    let credential = match info.credential.clone() {
        Some(val) => val,
        None => {
            return Err(actix_web::error::ErrorUnauthorized(
                "Credentials not provided",
            ))
        }
    };
    let credential_is_valid = match &state.auth_mode {
        AuthMode::CLOUDFLARE => {
            CfWorkerStore
                .credential_is_valid(&credential, &endpoint)
                .await
        }
        AuthMode::PLAINTEXT => PlaintextPassword.credential_is_valid(&credential, "").await,
        mode => {
            log::error!("Invalid AuthMode: {:?}", mode);
            return Err(actix_web::error::ErrorInternalServerError(
                "Invalid configuration",
            ));
        }
    };

    match credential_is_valid {
        Ok(val) => Ok(val),
        Err(err) => {
            log::error!("Error while validating creds: {:?}", err);
            Err(actix_web::error::ErrorInternalServerError(
                "Internal Server Error",
            ))
        }
    }
}

/// Request proxy endpoint
#[get("/{endpoint}")]
pub async fn request_endpoint(
    endpoint: web::Path<String>,
    info: web::Query<AuthInfo>,
    state: web::Data<State>,
) -> impl Responder {
    log::debug!("Request proxy endpoint, {}", endpoint);
    log::debug!("Require auth: {}", state.auth_mode);

    match validate_endpoint(&endpoint) {
        Ok(true) => (),
        Ok(false) => {
            return HttpResponse::BadRequest().body(
                "Request subdomain is invalid, only chars in lowercase and numbers are allowed",
            );
        }
        Err(err) => {
            return HttpResponse::InternalServerError().body(format!("Server Error: {:?}", err));
        }
    }

    match validate_credentials(&endpoint, &info, &state).await {
        Ok(true) => (),
        Ok(false) => return HttpResponse::Unauthorized().body("Invalid credentials"),
        Err(err) => return err.error_response(),
    }

    let mut manager = state.manager.lock().await;
    match manager.put(endpoint.to_string()).await {
        Ok(port) => {
            let schema = if state.secure { "https" } else { "http" };
            let info = ProxyInfo {
                id: endpoint.to_string(),
                port,
                max_conn_count: state.max_sockets,
                url: format!("{}://{}.{}", schema, endpoint, state.domain),
            };

            log::debug!("Proxy info, {:?}", info);
            HttpResponse::Ok().json(info)
        }
        Err(e) => {
            log::error!("Client manager failed to put proxy endpoint: {:?}", e);
            HttpResponse::InternalServerError().body(format!("Error: {:?}", e))
        }
    }
}

fn validate_endpoint(endpoint: &str) -> Result<bool> {
    // Don't allow A-Z uppercase since it will convert to lowercase in browser
    let re = Regex::new("^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$")?;
    Ok(re.is_match(endpoint))
}

#[derive(Debug, Deserialize)]
pub struct AuthInfo {
    credential: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiStatus {
    tunnels: u16,
    mem: MemoryUsage,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemoryUsage {
    rss: usize,
    heap_total: usize,
    heap_used: usize,
    external: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct TunnelStatus {
    connected_sockets: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProxyInfo {
    id: String,
    port: u16,
    max_conn_count: u8,
    url: String,
}

#[cfg(test)]
mod tests {
    use crate::api::validate_endpoint;

    #[test]
    fn validate_endpoint_works() {
        let endpoints = [
            "demo",
            "123",
            "did-key-zq3shkkuzlvqefghdgzgfmux8vgkgvwsla83w2oekhzxocw2n",
        ];

        for endpoint in endpoints {
            assert!(validate_endpoint(endpoint).unwrap());
        }
    }
}
