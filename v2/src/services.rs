use crate::core::{ip_version, Interface};
use serde_json::Value;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tezbar_extension_sdk::BoxFuture;

pub const GEO_COOLDOWN: &str =
    "Geolocation service is cooling down. Select Retry Lookup after the server cooldown.";
pub trait Services: Send + Sync + 'static {
    fn interfaces(&self) -> Result<Vec<Interface>, String>;
    fn public_ip(&self, family: u8) -> BoxFuture<'_, Result<String, String>>;
    fn geolocation<'a>(
        &'a self,
        query: &'a str,
        language: &'a str,
    ) -> BoxFuture<'a, Result<Value, String>>;
    fn now_ms(&self) -> u64;
}
#[derive(Default)]
pub struct Production {
    pub(crate) geo: crate::geo::GeoService,
}
impl Services for Production {
    fn interfaces(&self) -> Result<Vec<Interface>, String> {
        crate::interfaces::read()
    }
    fn public_ip(&self, family: u8) -> BoxFuture<'_, Result<String, String>> {
        Box::pin(async move {
            let local = if family == 6 {
                IpAddr::V6(Ipv6Addr::UNSPECIFIED)
            } else {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            };
            let client = reqwest::Client::builder()
                .https_only(true)
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .local_address(local)
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| "Cannot initialize HTTPS client")?;
            public_ip_from(&ClientTransport(client), family).await
        })
    }
    fn geolocation<'a>(
        &'a self,
        query: &'a str,
        language: &'a str,
    ) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(self.geo.lookup(query, language))
    }
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64
    }
}
pub fn public_urls(family: u8) -> [&'static str; 2] {
    [
        "https://icanhazip.com/",
        if family == 6 {
            "https://api6.ipify.org/"
        } else {
            "https://api.ipify.org/"
        },
    ]
}
pub async fn https_body(client: &reqwest::Client, url: &str) -> Result<String, String> {
    if ![
        "https://icanhazip.com/",
        "https://api.ipify.org/",
        "https://api6.ipify.org/",
    ]
    .contains(&url)
    {
        return Err("Unapproved public IP origin".into());
    }
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Public IP HTTPS request failed")?;
    decode_public_response(response).await
}
pub async fn decode_public_response(mut response: reqwest::Response) -> Result<String, String> {
    if response.status().as_u16() != 200 {
        return Err(if response.status().as_u16() == 429 {
            "Public IP service rate limited"
        } else {
            "Public IP service returned an unexpected status"
        }
        .into());
    }
    if response.content_length().is_some_and(|n| n > 1024) {
        return Err("Public IP response exceeds 1024 bytes".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Public IP response failed")?
    {
        if bytes.len() + chunk.len() > 1024 {
            return Err("Public IP response exceeds 1024 bytes".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| "Invalid public IP response encoding".into())
}
// Original fixed production contract. Component encoding prevents
// query delimiters / credentials supplied as user input from changing the request policy.
pub fn source_geo_request(query: &str, language: &str) -> String {
    format!(
        "http://ip-api.com/json/{}?lang={}&fields=585727",
        tezbar_extension_sdk::encode_component(query),
        tezbar_extension_sdk::encode_component(language)
    )
}

/// public-ip@7 uses is-ip@5/ip-regex, whose IPv6 zone is alphanumeric only.
pub fn public_ip_valid(ip: &str, family: u8) -> bool {
    ip_version(ip) == Some(family)
        && ip.len() <= if family == 4 { 15 } else { 45 }
        && ip
            .split_once('%')
            .is_none_or(|(_, zone)| zone.chars().all(|c| c.is_ascii_alphanumeric()))
}

#[derive(Clone, Debug)]
pub struct PublicRequest {
    pub url: &'static str,
    pub family: u8,
    pub timeout_ms: u64,
    pub retry_limit: u8,
}
pub trait PublicTransport: Send + Sync {
    fn request(&self, request: PublicRequest) -> BoxFuture<'_, Result<String, String>>;
}
struct ClientTransport(reqwest::Client);
impl PublicTransport for ClientTransport {
    fn request(&self, request: PublicRequest) -> BoxFuture<'_, Result<String, String>> {
        Box::pin(async move { https_body(&self.0, request.url).await })
    }
}
pub async fn public_ip_from(transport: &dyn PublicTransport, family: u8) -> Result<String, String> {
    for url in public_urls(family) {
        if let Ok(body) = transport
            .request(PublicRequest {
                url,
                family,
                timeout_ms: 5000,
                retry_limit: 0,
            })
            .await
        {
            let ip = crate::core::js_trim(&body);
            if public_ip_valid(ip, family) {
                return Ok(ip.into());
            }
        }
    }
    Err("Failed to get public IP address".into())
}
