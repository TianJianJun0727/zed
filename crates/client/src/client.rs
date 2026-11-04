pub mod os_info;

use gpui::App;
use http_client::{HttpClientWithUrl, read_proxy_from_env};
use serde::Deserialize;
use settings::{RegisterSetting, Settings};
use std::sync::Arc;
use url::Url;

pub use rpc::*;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct ProjectId(pub u64);

impl ProjectId {
    pub fn to_proto(self) -> u64 {
        self.0
    }
}

pub fn new_http_client(cx: &App) -> Arc<HttpClientWithUrl> {
    Arc::new(HttpClientWithUrl::new_url(
        cx.http_client(),
        "https://zed.dev",
        cx.http_client().proxy().cloned(),
    ))
}

#[derive(Deserialize, Default, RegisterSetting)]
pub struct ProxySettings {
    pub proxy: Option<String>,
}

impl ProxySettings {
    pub fn proxy_url(&self) -> Option<Url> {
        self.proxy
            .as_deref()
            .map(str::trim)
            .filter(|input| !input.is_empty())
            .and_then(|input| {
                input
                    .parse::<Url>()
                    .inspect_err(|e| log::error!("Error parsing proxy settings: {}", e))
                    .ok()
            })
            .or_else(read_proxy_from_env)
    }
}

impl Settings for ProxySettings {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        Self {
            proxy: content
                .proxy
                .as_deref()
                .map(str::trim)
                .filter(|proxy| !proxy.is_empty())
                .map(ToOwned::to_owned),
        }
    }
}
