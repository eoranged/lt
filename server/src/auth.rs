use anyhow::Result;
use async_trait::async_trait;

use crate::error::ServerError;
use crate::CONFIG;

#[async_trait]
pub trait Auth {
    async fn credential_is_valid(&self, credential: &str, value: &str) -> Result<bool>;
}

#[async_trait]
impl Auth for () {
    async fn credential_is_valid(&self, _credential: &str, _value: &str) -> Result<bool> {
        Ok(true)
    }
}

pub struct PlaintextPassword;
pub struct CfWorkerStore;

#[async_trait]
impl Auth for PlaintextPassword {
    async fn credential_is_valid(&self, credential: &str, _value: &str) -> Result<bool> {
        let password = CONFIG
            .plaintext_password
            .as_ref()
            .ok_or(ServerError::InvalidConfig)?;

        Ok(credential == password)
    }
}

#[async_trait]
impl Auth for CfWorkerStore {
    async fn credential_is_valid(&self, credential: &str, value: &str) -> Result<bool> {
        let account = CONFIG
            .cloudflare_account
            .as_ref()
            .ok_or(ServerError::InvalidConfig)?;
        let namespace = CONFIG
            .cloudflare_namespace
            .as_ref()
            .ok_or(ServerError::InvalidConfig)?;
        let email = CONFIG
            .cloudflare_auth_email
            .as_ref()
            .ok_or(ServerError::InvalidConfig)?;
        let key = CONFIG
            .cloudflare_auth_key
            .as_ref()
            .ok_or(ServerError::InvalidConfig)?;

        let client = reqwest::Client::new();
        let resp = client.get(
            format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/storage/kv/namespaces/{}/values/{}",
                account, namespace, value
            ))
            .header("X-Auth-Email", email)
            .header("X-Auth-Key", key)
            .send()
            .await?
            .text()
            .await?;
        log::info!("{:#?}", resp);

        Ok(credential == resp)
    }
}

pub fn validate(mode: &crate::AuthMode, config: &crate::Config) -> Result<()> {
    match mode {
        crate::AuthMode::PLAINTEXT => {
            if config.plaintext_password.is_none() {
                return Err(anyhow::anyhow!("Missing PLAINTEXT_PASSWORD env var"));
            }
        }
        crate::AuthMode::CLOUDFLARE => {
            let mut missing = Vec::new();
            if config.cloudflare_account.is_none() {
                missing.push("CLOUDFLARE_ACCOUNT");
            }
            if config.cloudflare_namespace.is_none() {
                missing.push("CLOUDFLARE_NAMESPACE");
            }
            if config.cloudflare_auth_email.is_none() {
                missing.push("CLOUDFLARE_AUTH_EMAIL");
            }
            if config.cloudflare_auth_key.is_none() {
                missing.push("CLOUDFLARE_AUTH_KEY");
            }

            if !missing.is_empty() {
                return Err(anyhow::anyhow!(
                    "Missing CLOUDFLARE credentials: {}",
                    missing.join(", ")
                ));
            }
        }
        crate::AuthMode::NOAUTH => {}
    }
    Ok(())
}
