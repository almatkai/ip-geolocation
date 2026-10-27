//! V1 semantics. No I/O, network, clock or host access.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::IpAddr;

pub type Rows = Vec<(String, String)>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub address: String,
    pub family: u8,
    pub internal: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preferences {
    pub language: String,
    pub coordinates_format: String,
    pub show_ipv6: bool,
}
impl Preferences {
    pub fn parse(v: &Value) -> Result<Self, String> {
        let language = v.get("language").and_then(Value::as_str).unwrap_or("en");
        if !["en", "de", "es", "pt-BR", "fr", "ja", "zh-CN", "ru"].contains(&language) {
            return Err("Invalid language preference; reopen after configuring Extension".into());
        }
        let format = v
            .get("coordinatesFormat")
            .and_then(Value::as_str)
            .unwrap_or("latLon");
        if !["latLon", "lonLat"].contains(&format) {
            return Err("Invalid coordinate preference".into());
        }
        Ok(Self {
            language: language.into(),
            coordinates_format: format.into(),
            show_ipv6: v["showIPv6"].as_bool().unwrap_or(false),
        })
    }
}
// JS String conversion used by template literals/isEmpty, including null and missing.
pub fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => ryu_js::Buffer::new()
            .format(n.as_f64().unwrap_or(0.0))
            .to_string(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(Some(v))
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}
pub fn empty(v: Option<&Value>) -> bool {
    v.is_none_or(|v| v.is_null() || js_string(Some(v)).is_empty())
}
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c,'\u{9}'..='\u{d}'|'\u{20}'|'\u{a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}'))
}
pub fn ip_version(s: &str) -> Option<u8> {
    // Node net.isIP accepts IPv6 zone IDs; the address must still be IPv6.
    let (address, zone) = s.split_once('%').map_or((s, None), |(a, z)| (a, Some(z)));
    if zone.is_some_and(|z| {
        z.is_empty()
            || !z
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".:-".contains(c))
    }) {
        return None;
    }
    match address.parse::<IpAddr>().ok()? {
        IpAddr::V4(_) if zone.is_none() => Some(4),
        IpAddr::V6(_) => Some(6),
        _ => None,
    }
}
pub fn parse_query(input: &str) -> Result<Option<String>, String> {
    if input.len() > 2048 {
        return Err("Query exceeds 2048 bytes".into());
    }
    let mut s = js_trim(input).to_string();
    if s.is_empty() || (!s.contains('.') && !s.contains(':')) {
        return Ok(None);
    }
    let ipv4_like =
        s.matches('.').count() == 3 && s.bytes().all(|c| c.is_ascii_digit() || c == b'.');
    if (ipv4_like || s.matches(':').count() >= 2) && ip_version(&s).is_none() {
        return Ok(None);
    }
    if s.starts_with("https://") || s.starts_with("http://") {
        s = s.replacen("https://", "", 1).replacen("http://", "", 1);
    }
    if let Some((first, _)) = s.split_once('/') {
        s = first.to_string();
    }
    // The source delegates domains (including IDNA) to ip-api. No local domain DNS.
    Ok(Some(s))
}
pub fn local(interfaces: &[Interface], family: u8) -> String {
    // Node objects enumerate interface names in first-seen order, then aliases.
    let mut names = Vec::new();
    for i in interfaces {
        if !names.contains(&i.name.as_str()) {
            names.push(i.name.as_str());
        }
    }
    names.sort_by_key(|name| match name.parse::<u32>() {
        Ok(n) if n < u32::MAX && n.to_string() == *name => (0, n),
        _ => (1, 0),
    });
    for name in names {
        for i in interfaces.iter().filter(|i| i.name == name) {
            if i.family == family
                && !i.internal
                && i.address != if family == 4 { "127.0.0.1" } else { "::1" }
            {
                return i.address.clone();
            }
        }
    }
    String::new()
}
pub fn geo_rows(v: &Value, prefs: &Preferences) -> Result<Rows, String> {
    if v["status"] != "success" {
        return Ok(Vec::new());
    }
    for field in ["query", "timezone", "isp", "org"] {
        if !v[field].is_string() {
            return Err("Invalid provider text field".into());
        }
    }
    let as_value = v["as"].as_str().ok_or("Invalid provider AS field")?;
    // JS substring(0, -1) is empty (source does NOT keep an AS without a space).
    let as_number = as_value.split_once(' ').map_or("", |(a, _)| a);
    let s = |k: &str| js_string(v.get(k));
    let mut location = format!("{}, {}, {}", s("country"), s("regionName"), s("city"));
    if !empty(v.get("district")) {
        location.push_str(&format!(", {}", s("district")));
    }
    if !empty(v.get("zip")) {
        location.push_str(&format!(", ZIP: {}", s("zip")));
    }
    let coords = if prefs.coordinates_format == "latLon" {
        format!("{} , {}", s("lat"), s("lon"))
    } else {
        format!("{} , {}", s("lon"), s("lat"))
    };
    Ok(vec![
        ("IP".into(), s("query")),
        ("Location".into(), location),
        ("GeoCoordinates".into(), coords),
        ("Timezone".into(), s("timezone")),
        ("AS".into(), as_number.into()),
        ("ISP".into(), s("isp")),
        ("Organization".into(), s("org")),
    ])
}
pub fn my_rows(
    v: &Value,
    prefs: &Preferences,
    local4: String,
    local6: String,
    public4: String,
    public6: String,
) -> Result<Rows, String> {
    let public4 = if public4.is_empty() {
        js_string(v.get("query"))
    } else {
        public4
    };
    let mut rows = vec![
        ("Local IPv4".into(), local4),
        ("Local IPv6".into(), local6),
        ("Public IPv4".into(), public4),
        ("Public IPv6".into(), public6),
    ];
    rows.extend(geo_rows(v, prefs)?.into_iter().skip(1));
    Ok(rows)
}
pub fn copy_all(rows: &Rows) -> String {
    let map: serde_json::Map<String, Value> = rows
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    serde_json::to_string_pretty(&map).expect("string map")
}
