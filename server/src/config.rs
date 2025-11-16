use serde::Deserialize;

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    // Cloudflare credentials
    pub cloudflare_account: Option<String>,
    pub cloudflare_namespace: Option<String>,
    pub cloudflare_auth_email: Option<String>,
    pub cloudflare_auth_key: Option<String>,
    // Plaintext password
    pub plaintext_password: Option<String>,
}
