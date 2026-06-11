//! Consul service discovery and KV store client

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsulError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
}

pub type Result<T> = std::result::Result<T, ConsulError>;

#[derive(Debug, Clone)]
pub struct ConsulConfig {
    pub address: String,
    pub token: Option<String>,
    pub timeout: Duration,
    pub datacenter: Option<String>,
}

impl Default for ConsulConfig {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:8500".into(),
            token: None,
            timeout: Duration::from_secs(5),
            datacenter: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceEntry {
    #[serde(rename = "ServiceID")]
    pub service_id: String,
    #[serde(rename = "ServiceName")]
    pub service_name: String,
    #[serde(rename = "ServiceAddress")]
    pub address: String,
    #[serde(rename = "ServicePort")]
    pub port: u16,
    #[serde(rename = "ServiceTags")]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KvPair {
    pub Key: String,
    pub Value: Option<String>,
    pub ModifyIndex: u64,
}

pub struct ConsulClient {
    config: ConsulConfig,
    http: reqwest::Client,
}

impl ConsulClient {
    pub fn new(config: ConsulConfig) -> Result<Self> {
        let mut builder = reqwest::Client::builder().timeout(config.timeout);
        if let Some(ref token) = config.token {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("X-Consul-Token", token.parse().map_err(|_| ConsulError::ServiceNotFound("invalid token".into()))?);
            builder = builder.default_headers(headers);
        }
        Ok(Self { config, http: builder.build()? })
    }

    fn url(&self, path: &str) -> std::result::Result<String, url::ParseError> {
        let base = url::Url::parse(&self.config.address)?;
        base.join(path).map(|u| u.to_string())
    }

    pub async fn register_service(&self, entry: &ServiceEntry) -> Result<()> {
        let url = self.url("/v1/agent/service/register")?;
        self.http.put(&url).json(entry).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn deregister_service(&self, service_id: &str) -> Result<()> {
        let url = self.url(&format!("/v1/agent/service/deregister/{service_id}"))?;
        self.http.put(&url).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn get_service(&self, name: &str) -> Result<Vec<ServiceEntry>> {
        let url = self.url(&format!("/v1/health/service/{name}"))?;
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        let raw: Vec<serde_json::Value> = resp.json().await?;
        let services = raw.iter().filter_map(|v| serde_json::from_value(v.get("Service")?.clone()).ok()).collect();
        Ok(services)
    }

    pub async fn kv_get(&self, key: &str) -> Result<KvPair> {
        let url = self.url(&format!("/v1/kv/{key}"))?;
        let resp = self.http.get(&url).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ConsulError::KeyNotFound(key.into()));
        }
        let mut pairs: Vec<KvPair> = resp.error_for_status()?.json().await?;
        pairs.pop().ok_or_else(|| ConsulError::KeyNotFound(key.into()))
    }

    pub async fn kv_put(&self, key: &str, value: &str) -> Result<bool> {
        let url = self.url(&format!("/v1/kv/{key}"))?;
        Ok(self.http.put(&url).body(value.to_owned()).send().await?.error_for_status()?.json().await?)
    }

    pub async fn kv_delete(&self, key: &str) -> Result<bool> {
        let url = self.url(&format!("/v1/kv/{key}"))?;
        Ok(self.http.delete(&url).send().await?.error_for_status()?.json().await?)
    }

    pub async fn health(&self) -> Result<bool> {
        let url = self.url("/v1/status/leader")?;
        let resp = self.http.get(&url).send().await?;
        Ok(resp.status().is_success())
    }
}
