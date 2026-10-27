use serde_json::{json, Value};
use tezbar_ip_geolocation::core::{self, Interface, Preferences, Rows};
fn prefs(v: &Value) -> Preferences {
    Preferences::parse(v).unwrap()
}
fn interfaces(v: &Value) -> Vec<Interface> {
    v.as_object()
        .unwrap()
        .iter()
        .flat_map(|(name, aliases)| {
            aliases.as_array().unwrap().iter().map(move |a| Interface {
                name: name.clone(),
                address: a["address"].as_str().unwrap().into(),
                family: if a["family"] == "IPv4" { 4 } else { 6 },
                internal: a["internal"].as_bool().unwrap(),
            })
        })
        .collect()
}
#[test]
fn full_actual_source_outputs() {
    let fixture: Value = serde_json::from_str(include_str!("../qa/oracle-results.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        match case["kind"].as_str().unwrap() {
            "query" => {
                let input = core::js_trim(case["input"].as_str().unwrap());
                let got = core::parse_query(input).unwrap();
                let expected = case["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c[0] == "geo")
                    .map(|c| {
                        c[1]["url"]
                            .as_str()
                            .unwrap()
                            .strip_prefix("http://ip-api.com/json/")
                            .unwrap()
                            .to_string()
                    });
                assert_eq!(got, expected, "input {:?}", input);
                if expected.is_some() {
                    assert_eq!(
                        serde_json::to_value(
                            core::geo_rows(&default_geo(), &prefs(&case["preferences"])).unwrap()
                        )
                        .unwrap(),
                        case["rows"]
                    );
                }
            }
            "format" => assert_eq!(
                serde_json::to_value(
                    core::geo_rows(&case["geo"], &prefs(&case["preferences"])).unwrap()
                )
                .unwrap(),
                case["rows"]
            ),
            "my" => {
                let p = prefs(&case["preferences"]);
                let i = interfaces(&case["interfaces"]);
                let rows = core::my_rows(
                    &case["geo"],
                    &p,
                    core::local(&i, 4),
                    if p.show_ipv6 {
                        core::local(&i, 6)
                    } else {
                        String::new()
                    },
                    "198.51.100.4".into(),
                    if p.show_ipv6 {
                        "2001:db8::4".into()
                    } else {
                        String::new()
                    },
                )
                .unwrap();
                assert_eq!(serde_json::to_value(rows).unwrap(), case["rows"]);
            }
            "copyinterfaces" => {
                let mut i = interfaces(&case["interfaces"]);
                if let Some(order) = case["interfaceOrder"].as_array() {
                    i.sort_by_key(|i| {
                        order
                            .iter()
                            .position(|v| v.as_str() == Some(&i.name))
                            .unwrap()
                    });
                }
                let family = if case["args"]["ipVersion"] == "IPv6" {
                    6
                } else {
                    4
                };
                let text = core::local(&i, family);
                let actual = case["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|c| c[0] == "copy");
                assert_eq!(
                    actual.map(|c| c[1].as_str().unwrap()),
                    if text.is_empty() {
                        None
                    } else {
                        Some(text.as_str())
                    }
                );
            }
            "copy" => assert!(case["calls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c[0] == "hide")),
            "public" | "persistent-args" | "background" => {}
            _ => panic!("unknown case"),
        }
        if let Some(rows) = case.get("rows") {
            let rows: Rows = serde_json::from_value(rows.clone()).unwrap();
            fn walk(v: &Value, found: &mut Vec<Value>) {
                if v["type"] == "Action.CopyToClipboard" && v["props"]["title"] == "Copy All Info" {
                    found.push(v["props"]["content"].clone());
                }
                match v {
                    Value::Object(o) => {
                        for v in o.values() {
                            walk(v, found)
                        }
                    }
                    Value::Array(a) => {
                        for v in a {
                            walk(v, found)
                        }
                    }
                    _ => {}
                }
            }
            let mut copies = Vec::new();
            walk(&case["tree"], &mut copies);
            for copy in copies {
                assert_eq!(core::copy_all(&rows), copy.as_str().unwrap());
            }
        }
    }
}
fn default_geo() -> Value {
    json!({"status":"success","query":"203.0.113.1","country":"Test","regionName":"Region","city":"City","district":"","zip":"","lat":0,"lon":-2,"timezone":"Etc/UTC","as":"AS0 Test","isp":"Fixture ISP","org":"Fixture Org"})
}
#[test]
fn source_query_encoding() {
    let s = tezbar_ip_geolocation::services::source_geo_request("x.test?x=&lang=ru/#@secret", "en");
    assert!(s.contains("x.test%3Fx%3D%26lang%3Dru%2F%23%40secret?lang=en&fields=585727"));
}

#[tokio::test]
async fn actual_locked_public_ip_fallback_outputs_and_parameters() {
    use std::{collections::VecDeque, sync::Mutex};
    use tezbar_extension_sdk::BoxFuture;
    use tezbar_ip_geolocation::services::{public_ip_from, PublicRequest, PublicTransport};
    struct Transport {
        responses: Mutex<VecDeque<Option<String>>>,
        calls: Mutex<Vec<Value>>,
    }
    impl PublicTransport for Transport {
        fn request(&self, r: PublicRequest) -> BoxFuture<'_, Result<String, String>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(json!(["https",r.url,{"dnsLookupIpVersion":r.family,"retry":{"limit":r.retry_limit},"timeout":{"request":r.timeout_ms}}]));
                self.responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .flatten()
                    .ok_or("synthetic failure".into())
            })
        }
    }
    let fixture: Value = serde_json::from_str(include_str!("../qa/oracle-results.json")).unwrap();
    for case in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "public")
    {
        let t = Transport {
            responses: Mutex::new(serde_json::from_value(case["responses"].clone()).unwrap()),
            calls: Mutex::default(),
        };
        let value = public_ip_from(&t, 4).await.ok();
        let actual = case["calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c[0] == "copy")
            .map(|c| c[1].as_str().unwrap().to_string());
        assert_eq!(value, actual);
        assert_eq!(
            *t.calls.lock().unwrap(),
            case["calls"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c[0] == "https")
                .cloned()
                .collect::<Vec<_>>()
        );
    }
}
