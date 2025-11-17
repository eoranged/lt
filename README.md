# Localtunnel

Updated fork of [rlt](https://github.com/kaichaosun/rlt) by [kaichaosun](https://github.com/kaichaosun) with [localtunnel](https://github.com/localtunnel/localtunnel) compatibility.

Localtunnel exposes your localhost endpoint to the world, use cases are:
- API testing
- multiple devices access to single data store
- peer to peer connection, workaround for NAT hole punching.

## Client Usage

Use in CLI:

```shell
cargo install --git https://github.com/eoranged/lt localtunnel

# host defaults to https://localtunnel.me, subdomain is optional.
# Running `lt --port 3000` is equivalent to `lt client --port 3000`
lt --port 3000
lt --subdomain my-api --port 3000
lt --host https://your-domain.com --subdomain kaichao --port 3000
```

Use as a Rust library:

```shell
cargo add --git https://github.com/eoranged/lt localtunnel-client
```

```Rust
use localtunnel_client::{open_tunnel, broadcast, ClientConfig};

let (notify_shutdown, _) = broadcast::channel(1);

let config = ClientConfig {
    server: Some("https://your-domain.com".to_string()),
    subdomain: Some("demo".to_string()),
    local_host: Some("localhost".to_string()),
    local_port: 3000,
    shutdown_signal: notify_shutdown.clone(),
    max_conn: 10,
    credential: None,
};
let result = open_tunnel(config).await?;

// Shutdown the background tasks by sending a signal.
let _ = notify_shutdown.send(());
```

## Server Usage

Use in CLI:

```shell
lt server --domain your-domain.com --port 3000 --proxy-port 3001 --secure
```

Use as a Rust library,

```shell
cargo add --git https://github.com/eoranged/lt localtunnel-server
```

```Rust
use localtunnel_server::{start, ServerConfig, AuthMode};

let config = ServerConfig {
    domain: "your-domain.com".to_string(),
    api_port: 3000,
    secure: true,
    max_sockets: 10,
    proxy_port: 3001,
    auth_mode: AuthMode::NOAUTH,
};

start(config).await?
```

## Configuration

The server supports the following environment variables:

- `RUST_LOG`: Logging level (e.g., `info`, `debug`, `warn`, `error`). Defaults to `info` if not set.
- `PLAINTEXT_PASSWORD`: Password for authentication when `auth_mode` is PLAINTEXT_PASSWORD (required when using plaintext authentication).
- `CLOUDFLARE_ACCOUNT`: Cloudflare account ID for KV storage (required when using Cloudflare authentication).
- `CLOUDFLARE_NAMESPACE`: Cloudflare KV namespace ID for storing authentication tokens (required when using Cloudflare authentication).
- `CLOUDFLARE_AUTH_EMAIL`: Cloudflare account email for API authentication (required when using Cloudflare authentication).
- `CLOUDFLARE_AUTH_KEY`: Cloudflare API key for authentication (required when using Cloudflare authentication).

You can create a `.env` file in the project root with these variables. See `.env.example` for reference.

## Sponsor

Support author of original project 👉 [GitHub Sponsors](https://github.com/sponsors/kaichaosun)

## Resources

- [localtunnel](https://github.com/localtunnel/localtunnel)