//! Settings from the environment (and from `.env` in the current directory).

use dotenvy::dotenv_override;
use reqwest::Url;
use serde::Deserialize;

/// A token shorter than this still works but is logged as a warning: the relay and every server share it.
pub const MIN_TOKEN_LEN: usize = 16;

#[derive(Deserialize)]
struct RawEnv {
    host: String,
    port: u16,
    workers: usize,
    origin: String,
    relay_url: String,
    token: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    /// The address under which the other servers reach this one, as written in the settings.
    pub origin: String,
    pub relay_url: Url,
    pub token: String,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        // `.env` wins over the real environment (as it always did here)
        dotenv_override().ok();
        let raw = envy::from_env::<RawEnv>().map_err(|e| e.to_string())?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawEnv) -> Result<Self, String> {
        if raw.host.trim().is_empty() {
            return Err("HOST is empty".into());
        }
        if raw.workers == 0 {
            return Err("WORKERS must be 1 or more".into());
        }
        if raw.token.is_empty() {
            return Err("TOKEN is empty".into());
        }
        if raw.token.chars().any(char::is_control) {
            return Err("TOKEN contains a control character".into());
        }
        let origin = web_url("ORIGIN", &raw.origin)?;
        let relay_url = web_url("RELAY_URL", &raw.relay_url)?;

        Ok(Self {
            host: raw.host,
            port: raw.port,
            workers: raw.workers,
            origin: origin.to_string().trim_end_matches('/').to_owned(),
            relay_url,
            token: raw.token,
        })
    }

    pub fn weak_token(&self) -> bool {
        self.token.chars().count() < MIN_TOKEN_LEN
    }

    /// The token goes to the relay in a header: over plain http it can be read on the way.
    pub fn relay_is_plain_http(&self) -> bool {
        self.relay_url.scheme() == "http"
            && !matches!(
                self.relay_url.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]")
            )
    }
}

/// A URL with the scheme http or https (anything else, such as `javascript:`, is refused).
pub fn web_url(name: &str, value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|e| format!("{name} is not a URL: {e}"))?;
    match url.scheme() {
        "http" | "https" if url.host_str().is_some() => Ok(url),
        _ => Err(format!("{name} must be an http or https URL")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> RawEnv {
        RawEnv {
            host: "localhost".into(),
            port: 8080,
            workers: 1,
            origin: "http://localhost:8080/".into(),
            relay_url: "https://relay.example.com".into(),
            token: "0123456789abcdef".into(),
        }
    }

    #[test]
    fn a_good_setting_is_accepted_and_the_origin_loses_its_trailing_slash() {
        let config = Config::from_raw(raw()).unwrap();
        assert_eq!(config.origin, "http://localhost:8080");
        assert!(!config.weak_token());
        assert!(!config.relay_is_plain_http());
    }

    #[test]
    fn bad_settings_are_refused() {
        let mut r = raw();
        r.token = String::new();
        assert!(Config::from_raw(r).is_err());

        let mut r = raw();
        r.origin = "javascript:alert(1)".into();
        assert!(Config::from_raw(r).is_err());

        let mut r = raw();
        r.relay_url = "ftp://relay.example.com".into();
        assert!(Config::from_raw(r).is_err());

        let mut r = raw();
        r.workers = 0;
        assert!(Config::from_raw(r).is_err());
    }

    #[test]
    fn a_short_token_and_a_plain_http_relay_are_noticed() {
        let mut r = raw();
        r.token = "a".into();
        r.relay_url = "http://relay.example.com".into();
        let config = Config::from_raw(r).unwrap();
        assert!(config.weak_token());
        assert!(config.relay_is_plain_http());

        let mut r = raw();
        r.relay_url = "http://localhost:8787".into();
        assert!(!Config::from_raw(r).unwrap().relay_is_plain_http());
    }
}
