//! Talking to the relay: tell it which files this server has, and that this server has gone.

use crate::{config::Config, model::Entry};
use reqwest::{header, Client, Url};
use serde_json::json;
use std::time::Duration;

pub struct Relay {
    client: Client,
    url: Url,
    origin: String,
    token: String,
}

impl Relay {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        // one client for the whole run, and a request that gets no answer does not stay for ever
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            client,
            url: config.relay_url.clone(),
            origin: config.origin.clone(),
            token: config.token.clone(),
        })
    }

    /// `first` is true at the start, so that the relay also sends the files of the other servers.
    pub async fn announce(&self, first: bool, files: &[Entry]) {
        let mut url = self.url.clone();
        if first {
            url.query_pairs_mut().append_pair("first", "true");
        }
        let body = json!({ "origin": self.origin, "files": files });
        self.send(self.client.put(url), body, "announce").await;
    }

    pub async fn withdraw(&self) {
        let body = json!({ "origin": self.origin });
        self.send(self.client.delete(self.url.clone()), body, "withdraw")
            .await;
    }

    async fn send(&self, request: reqwest::RequestBuilder, body: serde_json::Value, what: &str) {
        let result = request
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .json(&body)
            .send()
            .await;
        match result {
            Ok(response) if response.status().is_success() => log::debug!("relay: {what} done"),
            Ok(response) => log::warn!("relay: {what} refused with {}", response.status()),
            Err(e) => log::warn!("relay: {what} failed: {e}"),
        }
    }
}
