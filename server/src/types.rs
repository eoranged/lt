use std::fmt;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum AuthMode {
    NOAUTH,
    CLOUDFLARE,
    PLAINTEXT,
}

impl Default for AuthMode {
    fn default() -> Self {
        AuthMode::NOAUTH
    }
}

impl fmt::Display for AuthMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthMode::NOAUTH => write!(f, "NOAUTH"),
            AuthMode::CLOUDFLARE => write!(f, "CLOUDFLARE"),
            AuthMode::PLAINTEXT => write!(f, "PLAINTEXT"),
        }
    }
}
