use super::*;
use crate::{
    core::{self, Preferences},
    services::{Production, Services},
    Adapter,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use tezbar_extension_sdk::{Extension, ExtensionPaths, TestContext};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[derive(Clone, Default)]
struct TestClock(Arc<AtomicU64>);
impl Clock for TestClock {
    fn ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    fn wall(&self) -> std::time::SystemTime {
        std::time::UNIX_EPOCH + Duration::from_secs(1_798_761_600)
    }
}
fn service(address: std::net::SocketAddr) -> (GeoService, TestClock, Arc<AtomicUsize>) {
    let clock = TestClock::default();
    let calls = Arc::new(AtomicUsize::new(0));
    (
        GeoService {
            transport: Box::new(FixedTransport {
                client: OnceLock::from(Ok(client(Duration::from_millis(100)).unwrap())),
                loopback: Some(address),
                calls: calls.clone(),
            }),
            clock: Box::new(clock.clone()),
            cooldown: tokio::sync::Mutex::new(0),
            operation_limit: OPERATION_LIMIT,
        },
        clock,
        calls,
    )
}
async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut b = [0; 1024];
        let n = stream.read(&mut b).await.unwrap();
        assert!(n > 0 && bytes.len() < 16384);
        bytes.extend_from_slice(&b[..n]);
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}
fn response(status: u16, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut b = format!(
        "HTTP/1.1 {status} Synthetic\r\nConnection: close\r\nContent-Length: {}\r\n{headers}\r\n",
        body.len()
    )
    .into_bytes();
    b.extend_from_slice(body);
    b
}
fn success() -> Value {
    json!({"status":"success","query":"203.0.113.1","country":"Test","regionName":"Region","city":"City","district":"","zip":"","lat":0,"lon":-2,"timezone":"Etc/UTC","as":"AS0 Test","isp":"Fixture ISP","org":"Fixture Org"})
}
async fn fixture(raw: Vec<u8>, delay: bool) -> Result<Value, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (geo, _, _) = service(listener.local_addr().unwrap());
    let server = async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        if delay {
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        let _ = stream.write_all(&raw).await;
    };
    let (_, result) = tokio::join!(server, geo.lookup("example.test", "en"));
    result
}
#[test]
fn fixed_production_url_and_policy() {
    assert_eq!(REQUEST_LIMIT, Duration::from_secs(5));
    assert_eq!(OPERATION_LIMIT, Duration::from_secs(10));
    for q in [
        "203.0.113.1",
        "example.test",
        "2001:db8::1",
        "",
        "x.test?x=&lang=ru/#@TOKEN\r\nAuthorization: secret",
        "//evil.test/@x.test",
        "ü.test",
        "%2f..%2f",
    ] {
        let u = request_url(q, "pt-BR").unwrap();
        assert_eq!(u.as_str(), source_geo_request(q, "pt-BR"));
        assert_eq!(u.host_str(), Some("ip-api.com"));
        assert_eq!(
            u.path(),
            format!("/json/{}", tezbar_extension_sdk::encode_component(q))
        );
        assert_eq!(u.query(), Some("lang=pt-BR&fields=585727"));
    }
    for q in [".", ".."] {
        assert!(request_url(q, "en").is_err());
    }
    assert!(request_url("example.test", "en&x=1").is_err());
    assert!(request_url(&"a".repeat(2049), "en").is_err());
    // The same builder used in production really allows the fixed HTTP protocol.
    assert!(client(REQUEST_LIMIT)
        .unwrap()
        .get(request_url("", "en").unwrap())
        .build()
        .is_ok());
}
#[tokio::test]
async fn production_entrypoint_real_http_requests_and_source_outputs() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (geo, _, calls) = service(listener.local_addr().unwrap());
    let production = Production { geo };
    let queries = [
        "203.0.113.1",
        "example.test",
        "2001:db8::1",
        "",
        "x.test?x=&lang=ru/#@TOKEN\r\nAuthorization: secret",
    ];
    let server = async {
        for q in queries {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            assert!(request.starts_with(&format!(
                "GET /json/{}?lang=ru&fields=585727 HTTP/1.1\r\n",
                tezbar_extension_sdk::encode_component(q)
            )));
            let lower = request.to_ascii_lowercase();
            assert!(lower.contains("\r\nhost: ip-api.com\r\n"));
            for h in [
                "\r\nauthorization:",
                "\r\nproxy-authorization:",
                "\r\ncookie:",
            ] {
                assert!(!lower.contains(h));
            }
            stream
                .write_all(&response(
                    200,
                    "Set-Cookie: TOKEN=secret\r\n",
                    &serde_json::to_vec(&success()).unwrap(),
                ))
                .await
                .unwrap();
        }
    };
    let run = async {
        for q in queries {
            assert_eq!(production.geolocation(q, "ru").await.unwrap(), success());
        }
    };
    tokio::join!(server, run);
    assert_eq!(calls.load(Ordering::SeqCst), queries.len());
    // Actual production parser must accept every successful/scalar formatting oracle.
    let oracles: Value = serde_json::from_str(include_str!("../qa/oracle-results.json")).unwrap();
    for case in oracles["cases"].as_array().unwrap() {
        if case["kind"] == "format" {
            let decoded = fixture(
                response(200, "", &serde_json::to_vec(&case["geo"]).unwrap()),
                false,
            )
            .await
            .unwrap();
            let rows = core::geo_rows(&decoded, &Preferences::parse(&case["preferences"]).unwrap())
                .unwrap();
            assert_eq!(serde_json::to_value(rows).unwrap(), case["rows"]);
        }
    }
}
#[tokio::test]
async fn production_adapter_domain_http_and_no_failed_cache_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (geo, _, _) = service(listener.local_addr().unwrap());
    let adapter = Adapter::new(Production { geo });
    let dir = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/.work")).unwrap();
    let t = TestContext::new(
        ExtensionPaths {
            package_dir: dir.path().into(),
            data_dir: dir.path().join("data"),
            temp_dir: dir.path().join("tmp"),
        },
        &[],
    );
    let server = async {
        for body in [
            serde_json::to_vec(&success()).unwrap(),
            br#"{"status":"fail","message":"private range TOKEN","query":"example.test"}"#.to_vec(),
            b"invalid TOKEN".to_vec(),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(read_request(&mut stream)
                .await
                .starts_with("GET /json/example.test?lang=en&fields=585727 HTTP/1.1"));
            stream.write_all(&response(200, "", &body)).await.unwrap();
        }
    };
    let run = async {
        let mut state = json!({});
        let mut first_saved = Value::Null;
        for i in 0..3 {
            let v = t.run(adapter.handle(&t.context,"query-ip-geolocation","extension:run-command", &json!({"sessionId": format!("q{i}"),"argumentValues":{"ipAddress":" example.test "},"preferences":{}}), &mut state)).await.unwrap();
            let saved = crate::store::read(&t.context).await.unwrap();
            assert_eq!(saved["nativeV2"]["cache"].as_array().unwrap().len(), 1);
            if i == 0 {
                first_saved = saved.clone();
                assert_eq!(v["root"]["children"][0]["props"]["subtitle"], "203.0.113.1");
            }
            assert_eq!(saved, first_saved);
            if i == 1 {
                assert!(state["rows"].as_array().unwrap().is_empty());
            }
            if i == 2 {
                assert!(v.to_string().contains("cached result"));
            }
            assert!(!v.to_string().contains("TOKEN"));
        }
        assert!(t.calls().is_empty());
    };
    tokio::join!(server, run);
}
#[tokio::test]
async fn decoder_bounds_failure_status_no_redirect_no_reflection() {
    for status in [201, 301, 302, 307, 308, 401, 429, 500, 599] {
        let e = fixture(
            response(
                status,
                "Location: http://127.0.0.1:1/TOKEN\r\nSet-Cookie: TOKEN=1\r\n",
                b"TOKEN",
            ),
            false,
        )
        .await
        .unwrap_err();
        assert_eq!(
            e,
            if status == 429 {
                GEO_COOLDOWN
            } else {
                "Geolocation service returned an unexpected status"
            }
        );
        assert!(!e.contains("TOKEN") && !e.contains("127.0.0.1"));
    }
    let fail = json!({"status":"fail","message":"private range TOKEN","query":"192.0.2.1"});
    assert_eq!(
        fixture(
            response(200, "", &serde_json::to_vec(&fail).unwrap()),
            false
        )
        .await
        .unwrap(),
        fail
    );
    for body in [
        b"nonJSON TOKEN".to_vec(),
        b"{\"status\":".to_vec(),
        vec![255],
        b"null".to_vec(),
        b"{}".to_vec(),
        b"[]".to_vec(),
        b"{\"status\":true}".to_vec(),
        b"{\"status\":\"unexpected\"}".to_vec(),
        b"{\"status\":\"success\",\"query\":123}".to_vec(),
        b"{\"status\":\"fail\",\"message\":123}".to_vec(),
    ] {
        assert_eq!(
            fixture(response(200, "", &body), false).await.unwrap_err(),
            BAD_RESPONSE
        );
    }
    assert!(fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 999999\r\n\r\n".to_vec(),
        false
    )
    .await
    .is_err());
    assert!(fixture(
        b"HTTP/1.1 200 OK\r\nContent-Length: 200\r\n\r\n{}".to_vec(),
        false
    )
    .await
    .is_err());
    let body = vec![b'x'; MAX_BODY + 1];
    let mut chunked = format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
        body.len()
    )
    .into_bytes();
    chunked.extend(body);
    chunked.extend(b"\r\n0\r\n\r\n");
    assert!(fixture(chunked, false).await.is_err());
    assert!(fixture(response(200, "", b"{}"), true).await.is_err());
    for (key, bad) in [
        ("query", json!(false)),
        ("timezone", json!([])),
        ("as", json!(null)),
        ("lat", json!({})),
        ("country", json!([])),
    ] {
        let mut v = success();
        v[key] = bad;
        assert_eq!(
            fixture(response(200, "", &serde_json::to_vec(&v).unwrap()), false)
                .await
                .unwrap_err(),
            BAD_RESPONSE
        );
    }
    for extra in [
        json!("x".repeat(8193)),
        json!(vec![0; 257]),
        serde_json::from_str::<Value>(&format!("{}0{}", "[".repeat(17), "]".repeat(17))).unwrap(),
        json!({"k".repeat(129):0}),
    ] {
        let mut v = success();
        v["extra"] = extra;
        assert_eq!(
            fixture(response(200, "", &serde_json::to_vec(&v).unwrap()), false)
                .await
                .unwrap_err(),
            BAD_RESPONSE
        );
    }
}
#[tokio::test]
async fn cooldown_manual_retry_elapsed_and_bounded_headers() {
    for (status, headers, delay) in [
        (200, "X-Rl: 0\r\nX-Ttl: 60\r\n", 60000),
        (429, "Retry-After: 2\r\nX-Ttl: 3\r\n", 3000),
        (429, "Retry-After: Fri, 01 Jan 2027 00:00:03 GMT\r\n", 3000),
        (429, "Retry-After: Thu, 01 Jan 2026 00:00:03 GMT\r\n", 0),
        (
            429,
            "Retry-After: Fri, 01 Jan 2027 01:00:03 GMT\r\n",
            MAX_COOLDOWN_MS,
        ),
        (429, "X-Ttl: 9999999999\r\n", MAX_COOLDOWN_MS),
        (429, "Retry-After: invalid TOKEN\r\nX-Ttl: -1\r\n", 0),
        (429, "X-Ttl: 1\r\nX-Ttl: 2\r\n", 0),
        (200, "X-Rl: 1\r\nX-Ttl: 60\r\n", 0),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (geo, clock, calls) = service(listener.local_addr().unwrap());
        let server = async {
            for (s, h) in [(status, headers), (200, "")] {
                let (mut stream, _) = listener.accept().await.unwrap();
                read_request(&mut stream).await;
                stream
                    .write_all(&response(s, h, &serde_json::to_vec(&success()).unwrap()))
                    .await
                    .unwrap();
            }
        };
        let run = async {
            let first = geo.lookup("example.test", "en").await;
            assert_eq!(first.is_ok(), status == 200);
            if delay > 0 {
                for at in [0, delay - 1] {
                    clock.0.store(at, Ordering::SeqCst);
                    assert_eq!(
                        geo.lookup("other.test", "en").await.unwrap_err(),
                        GEO_COOLDOWN
                    );
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                }
                clock.0.store(delay, Ordering::SeqCst);
            }
            assert!(geo.lookup("example.test", "en").await.is_ok());
        };
        tokio::join!(server, run);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
#[tokio::test]
async fn dropping_http_future_cancels_and_releases_serialization() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (geo, _, calls) = service(listener.local_addr().unwrap());
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let server = async {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        ready_tx.send(()).unwrap();
        let mut b = [0; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), stream.read(&mut b))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(&response(200, "", &serde_json::to_vec(&success()).unwrap()))
            .await
            .unwrap();
    };
    let run = async {
        let mut first = Box::pin(geo.lookup("example.test", "en"));
        tokio::select! { _ = ready_rx => {}, _ = &mut first => panic!("request should be pending") }
        drop(first);
        assert!(geo.lookup("example.test", "en").await.is_ok());
    };
    tokio::join!(server, run);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn whole_operation_deadline_and_cooldown_before_partial_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (mut geo, clock, calls) = service(listener.local_addr().unwrap());
    geo.operation_limit = Duration::from_millis(30);
    let server = async {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nX-Rl: 0\r\nX-Ttl: 60\r\nContent-Length: 999\r\n\r\n{")
            .await
            .unwrap();
        let mut b = [0; 1];
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), stream.read(&mut b))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        stream
            .write_all(&response(200, "", &serde_json::to_vec(&success()).unwrap()))
            .await
            .unwrap();
    };
    let run = async {
        assert_eq!(
            geo.lookup("example.test", "en").await.unwrap_err(),
            "Geolocation operation timed out"
        );
        assert_eq!(
            geo.lookup("example.test", "en").await.unwrap_err(),
            GEO_COOLDOWN
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        clock.0.store(60000, Ordering::SeqCst);
        assert!(geo.lookup("example.test", "en").await.is_ok());
    };
    tokio::join!(server, run);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    // Saturating deadline arithmetic cannot wrap into an unbounded cooldown.
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("x-ttl", "9999999999".parse().unwrap());
    assert_eq!(seconds(&headers, "x-ttl"), Some(MAX_COOLDOWN_MS));
    assert_eq!(
        (u64::MAX - 1).saturating_add(seconds(&headers, "x-ttl").unwrap()),
        u64::MAX
    );
}

// Synthetic producers for the other two boundaries; geolocation stays Production.
struct MyServices(Production);
impl Services for MyServices {
    fn interfaces(&self) -> Result<Vec<core::Interface>, String> {
        Ok(vec![core::Interface {
            name: "fixture0".into(),
            address: "192.0.2.7".into(),
            family: 4,
            internal: false,
        }])
    }
    fn public_ip(&self, _: u8) -> BoxFuture<'_, Result<String, String>> {
        Box::pin(async { Ok("198.51.100.4".into()) })
    }
    fn geolocation<'a>(&'a self, q: &'a str, l: &'a str) -> BoxFuture<'a, Result<Value, String>> {
        self.0.geolocation(q, l)
    }
    fn now_ms(&self) -> u64 {
        1000
    }
}
#[tokio::test]
async fn my_source_fail_and_transport_failure_never_replace_fresh_cache() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (geo, _, _) = service(listener.local_addr().unwrap());
    let adapter = Adapter::new(MyServices(Production { geo }));
    let dir = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/.work")).unwrap();
    let t = TestContext::new(
        ExtensionPaths {
            package_dir: dir.path().into(),
            data_dir: dir.path().join("data"),
            temp_dir: dir.path().join("tmp"),
        },
        &[],
    );
    let server = async {
        for body in [
            serde_json::to_vec(&success()).unwrap(),
            br#"{"status":"fail","message":"TOKEN"}"#.to_vec(),
            b"invalid TOKEN".to_vec(),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(read_request(&mut stream)
                .await
                .starts_with("GET /json/198.51.100.4?lang=en&fields=585727 HTTP/1.1"));
            stream.write_all(&response(200, "", &body)).await.unwrap();
        }
    };
    let run = async {
        let mut state = json!({});
        let mut first_saved = Value::Null;
        for i in 0..3 {
            let v = t
                .run(adapter.handle(
                    &t.context,
                    "my-ip-geolocation",
                    "extension:run-command",
                    &json!({"sessionId": format!("my{i}"),"argumentValues":{},"preferences":{}}),
                    &mut state,
                ))
                .await
                .unwrap();
            let saved = crate::store::read(&t.context).await.unwrap();
            if i == 0 {
                first_saved = saved.clone();
                assert_eq!(state["rows"].as_array().unwrap().len(), 10);
            } else {
                assert_eq!(state["rows"].as_array().unwrap().len(), 4);
                assert_eq!(state["rows"][0], json!(["Local IPv4", "192.0.2.7"]));
                assert_eq!(state["rows"][2], json!(["Public IPv4", "198.51.100.4"]));
                assert_eq!(state["retry"], true);
            }
            assert_eq!(saved, first_saved);
            assert!(!v.to_string().contains("TOKEN"));
        }
        assert!(t.calls().is_empty());
    };
    tokio::join!(server, run);
}

#[tokio::test]
async fn whole_deadline_includes_waiting_for_other_lookup() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (mut geo, _, calls) = service(listener.local_addr().unwrap());
    geo.operation_limit = Duration::from_millis(30);
    // Hold serialization with a pending real HTTP request. A queued manual call
    // still has the whole-operation deadline, and never starts its own request.
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let server = async {
        let (mut stream, _) = listener.accept().await.unwrap();
        read_request(&mut stream).await;
        ready_tx.send(()).unwrap();
        let mut b = [0; 1];
        assert_eq!(stream.read(&mut b).await.unwrap(), 0);
    };
    let run = async {
        let mut first = Box::pin(geo.lookup("example.test", "en"));
        tokio::select! { _ = ready_rx => {}, _ = &mut first => panic!("must be pending") }
        // Do not poll first again: it keeps the lock until explicitly dropped.
        assert_eq!(
            geo.lookup("other.test", "en").await.unwrap_err(),
            "Geolocation operation timed out"
        );
        drop(first);
    };
    tokio::join!(server, run);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
