//! Fixed original free-provider HTTP contract. No configurable product endpoint.
use crate::services::{source_geo_request, GEO_COOLDOWN};
use serde_json::Value;
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use tezbar_extension_sdk::BoxFuture;

const REQUEST_LIMIT: Duration = Duration::from_secs(5);
const OPERATION_LIMIT: Duration = Duration::from_secs(10);
const MAX_BODY: usize = 65536;
const MAX_COOLDOWN_MS: u64 = 3_600_000;
const BAD_RESPONSE: &str = "Invalid geolocation service response";

trait Transport: Send + Sync {
    fn get(&self, url: reqwest::Url) -> BoxFuture<'_, Result<reqwest::Response, String>>;
}
trait Clock: Send + Sync {
    fn ms(&self) -> u64;
    fn wall(&self) -> std::time::SystemTime {
        std::time::SystemTime::now()
    }
}
struct Monotonic(Instant);
impl Clock for Monotonic {
    fn ms(&self) -> u64 {
        self.0.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
}
#[derive(Default)]
struct FixedTransport {
    client: OnceLock<Result<reqwest::Client, String>>,
    #[cfg(test)]
    loopback: Option<std::net::SocketAddr>,
    #[cfg(test)]
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}
fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(timeout)
        .timeout(timeout)
        .build()
        .map_err(|_| "Cannot initialize geolocation client".into())
}
impl Transport for FixedTransport {
    fn get(&self, url: reqwest::Url) -> BoxFuture<'_, Result<reqwest::Response, String>> {
        Box::pin(async move {
            let client = self
                .client
                .get_or_init(|| client(REQUEST_LIMIT))
                .as_ref()
                .map_err(|_| "Cannot initialize geolocation client")?;
            #[cfg(test)]
            let (url, routed) = {
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut url = url;
                if let Some(address) = self.loopback {
                    assert_eq!(url.host_str(), Some("ip-api.com"));
                    assert_eq!(url.port_or_known_default(), Some(80));
                    url.set_host(Some("127.0.0.1")).unwrap();
                    url.set_port(Some(address.port())).unwrap();
                }
                (url, self.loopback.is_some())
            };
            let request = client.get(url);
            #[cfg(test)]
            let request = if routed {
                request.header("host", "ip-api.com")
            } else {
                request
            };
            request
                .send()
                .await
                .map_err(|_| "Geolocation request failed".into())
        })
    }
}
pub(crate) struct GeoService {
    transport: Box<dyn Transport>,
    clock: Box<dyn Clock>,
    // Also serializes requests: a known server cooldown cannot race another send.
    cooldown: tokio::sync::Mutex<u64>,
    operation_limit: Duration,
}
impl Default for GeoService {
    fn default() -> Self {
        Self {
            transport: Box::<FixedTransport>::default(),
            clock: Box::new(Monotonic(Instant::now())),
            cooldown: tokio::sync::Mutex::new(0),
            operation_limit: OPERATION_LIMIT,
        }
    }
}
fn request_url(query: &str, language: &str) -> Result<reqwest::Url, String> {
    if query.len() > 2048
        || !["en", "de", "es", "pt-BR", "fr", "ja", "zh-CN", "ru"].contains(&language)
    {
        return Err("Invalid geolocation request".into());
    }
    let raw = source_geo_request(query, language);
    let url = reqwest::Url::parse(&raw).map_err(|_| "Invalid geolocation request")?;
    // URL parsers normalize dot segments. Refuse any normalization of our encoded path.
    if url.as_str() != raw
        || url.scheme() != "http"
        || url.host_str() != Some("ip-api.com")
        || url.port_or_known_default() != Some(80)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Invalid geolocation request".into());
    }
    Ok(url)
}
fn seconds(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u64> {
    let mut values = headers.get_all(name).iter();
    let v = values.next()?.to_str().ok()?;
    if values.next().is_some()
        || v.is_empty()
        || v.len() > 10
        || !v.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let n: u64 = v.parse().ok()?;
    Some(n.saturating_mul(1000).min(MAX_COOLDOWN_MS))
}
fn retry_after(headers: &reqwest::header::HeaderMap, now: std::time::SystemTime) -> Option<u64> {
    if let Some(ms) = seconds(headers, "retry-after") {
        return Some(ms);
    }
    let mut values = headers.get_all("retry-after").iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() || value.len() > 64 {
        return None;
    }
    let at = httpdate::parse_http_date(value).ok()?;
    Some(
        at.duration_since(now)
            .unwrap_or_default()
            .as_millis()
            .min(MAX_COOLDOWN_MS as u128) as u64,
    )
}
impl GeoService {
    pub(crate) async fn lookup(&self, query: &str, language: &str) -> Result<Value, String> {
        let url = request_url(query, language)?;
        tokio::time::timeout(self.operation_limit, async {
            let mut until = self.cooldown.lock().await;
            let now = self.clock.ms();
            if now < *until {
                return Err(GEO_COOLDOWN.into());
            }
            let mut response = self.transport.get(url).await?;
            let status = response.status().as_u16();
            let headers = response.headers();
            let exhausted = headers.get("x-rl").is_some_and(|v| v == "0");
            let ttl = if exhausted || status == 429 {
                seconds(headers, "x-ttl")
            } else {
                None
            };
            let retry = if status == 429 {
                retry_after(headers, self.clock.wall())
            } else {
                None
            };
            if let Some(delay) = ttl.into_iter().chain(retry).max() {
                *until = self.clock.ms().saturating_add(delay);
            }
            if status != 200 {
                return Err(if status == 429 {
                    GEO_COOLDOWN
                } else {
                    "Geolocation service returned an unexpected status"
                }
                .into());
            }
            if response
                .content_length()
                .is_some_and(|n| n > MAX_BODY as u64)
            {
                return Err(BAD_RESPONSE.into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| BAD_RESPONSE)? {
                if bytes.len().saturating_add(chunk.len()) > MAX_BODY {
                    return Err(BAD_RESPONSE.into());
                }
                bytes.extend_from_slice(&chunk);
            }
            parse(&bytes)
        })
        .await
        .map_err(|_| "Geolocation operation timed out")?
    }
}
fn parse(bytes: &[u8]) -> Result<Value, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| BAD_RESPONSE)?;
    fn bounded(v: &Value, depth: usize, nodes: &mut usize) -> bool {
        *nodes += 1;
        if depth > 16 || *nodes > 256 {
            return false;
        }
        match v {
            Value::String(s) => s.len() <= 8192,
            Value::Array(a) => a.iter().all(|v| bounded(v, depth + 1, nodes)),
            Value::Object(o) => o
                .iter()
                .all(|(k, v)| k.len() <= 128 && bounded(v, depth + 1, nodes)),
            _ => true,
        }
    }
    if !v.is_object()
        || !bounded(&v, 0, &mut 0)
        || !matches!(v["status"].as_str(), Some("success" | "fail"))
    {
        return Err(BAD_RESPONSE.into());
    }
    if v["status"] == "success" {
        for key in ["query", "timezone", "as", "isp", "org"] {
            if !v[key].is_string() {
                return Err(BAD_RESPONSE.into());
            }
        }
        // Source formatting supports missing/null/boolean/numeric scalar values.
        for key in [
            "country",
            "regionName",
            "city",
            "district",
            "zip",
            "lat",
            "lon",
        ] {
            if v[key].is_array() || v[key].is_object() {
                return Err(BAD_RESPONSE.into());
            }
        }
    } else if v.get("message").is_some_and(|v| !v.is_string())
        || v.get("query").is_some_and(|v| !v.is_string())
    {
        return Err(BAD_RESPONSE.into());
    }
    Ok(v)
}

#[cfg(test)]
#[path = "geo_tests.rs"]
mod tests;
