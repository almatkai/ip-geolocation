use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tezbar_extension_sdk::{BoxFuture, Extension, ExtensionPaths, TestContext};
use tezbar_ip_geolocation::{
    adapter::validate_host_result, core::Interface, services::Services, Adapter,
};
#[derive(Clone, Default)]
struct Mock {
    calls: Arc<Mutex<Vec<String>>>,
}
impl Services for Mock {
    fn interfaces(&self) -> Result<Vec<Interface>, String> {
        self.calls.lock().unwrap().push("interfaces".into());
        Ok(vec![Interface {
            name: "fake".into(),
            address: "192.0.2.7".into(),
            family: 4,
            internal: false,
        }])
    }
    fn public_ip(&self, f: u8) -> BoxFuture<'_, Result<String, String>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(format!("public{f}"));
            Ok(if f == 4 {
                "198.51.100.4"
            } else {
                "2001:db8::4"
            }
            .into())
        })
    }
    fn geolocation<'a>(&'a self, q: &'a str, l: &'a str) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(format!("geo:{q}:{l}"));
            Ok(
                json!({"status":"success","query":q,"country":"Test","regionName":"R","city":"C","district":"","zip":"","lat":0,"lon":1,"timezone":"UTC","as":"AS1 X","isp":"I","org":"O"}),
            )
        })
    }
    fn now_ms(&self) -> u64 {
        1000
    }
}
fn context(path: &std::path::Path) -> TestContext {
    TestContext::new(
        ExtensionPaths {
            package_dir: path.into(),
            data_dir: path.join("data"),
            temp_dir: path.join("tmp"),
        },
        &["clipboard.write", "window.visibility", "settings.open"],
    )
}
#[tokio::test]
async fn automatic_copy_exact_order_and_closed_ack() {
    let d = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR").to_string() + "/.work").unwrap();
    let t = context(d.path());
    let a = Adapter::new(Mock::default());
    let mut state = json!({});
    t.reply(Ok(Value::Null));
    t.reply(Ok(json!({"ok":true})));
    let v = t
        .run(a.handle(
            &t.context,
            "copy-ip",
            "extension:run-command",
            &json!({"sessionId":"x","argumentValues":{},"preferences":{}}),
            &mut state,
        ))
        .await
        .unwrap();
    assert_eq!(v["mode"], "no-view");
    assert_eq!(
        t.calls()
            .iter()
            .map(|c| c["method"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        ["host.window.visibility", "host.clipboard.write"]
    );
    assert_eq!(t.calls()[1]["params"], json!({"text":"192.0.2.7"}));
}
#[tokio::test]
async fn exact_retry_no_second_lookup_and_old_ids_consumed() {
    let d = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR").to_string() + "/.work").unwrap();
    let t = context(d.path());
    let m = Mock::default();
    let a = Adapter::new(m.clone());
    let mut state = json!({});
    t.reply(Ok(Value::Null));
    t.reply(Ok(json!(true)));
    t.reply(Ok(Value::Null));
    let v = t
        .run(a.handle(
            &t.context,
            "copy-ip",
            "extension:run-command",
            &json!({"sessionId":"x","argumentValues":{},"preferences":{}}),
            &mut state,
        ))
        .await
        .unwrap();
    let action = v["actions"][0]["id"].as_str().unwrap().to_string();
    t.reply(Ok(json!({"ok":true})));
    t.reply(Ok(Value::Null));
    let request = json!({"sessionId":"x","actionId":action,"preferences":{}});
    t.run(a.handle(
        &t.context,
        "copy-ip",
        "extension:invoke-action",
        &request,
        &mut state,
    ))
    .await
    .unwrap();
    assert!(t
        .run(a.handle(
            &t.context,
            "copy-ip",
            "extension:invoke-action",
            &request,
            &mut state
        ))
        .await
        .is_err());
    assert_eq!(*m.calls.lock().unwrap(), ["interfaces"]);
    let writes = t
        .calls()
        .into_iter()
        .filter(|c| c["method"] == "host.clipboard.write")
        .map(|c| c["params"].clone())
        .collect::<Vec<_>>();
    assert_eq!(writes, vec![json!({"text":"192.0.2.7"}); 2]);
}
#[test]
fn method_specific_result_shapes() {
    for method in [
        "host.clipboard.write",
        "host.clipboard.pasteText",
        "host.shell.open",
        "host.settings.open",
    ] {
        for good in [json!({"ok":true}), json!({"ok":true,"message":"done"})] {
            assert!(validate_host_result(method, good).is_ok());
        }
        for bad in [
            Value::Null,
            json!(true),
            json!({"ok":true,"message":null}),
            json!({"ok":true,"error":"bad"}),
            json!({"ok":false}),
            json!({"ok":true,"extra":1}),
            json!({}),
        ] {
            assert!(validate_host_result(method, bad).is_err());
        }
    }
    assert!(validate_host_result("host.window.visibility", Value::Null).is_ok());
    assert!(validate_host_result("host.window.visibility", json!({"ok":true})).is_err());
    for method in ["host.clipboard.read", "host.selection.read"] {
        assert!(validate_host_result(method, json!("text")).is_ok());
        assert!(validate_host_result(method, json!({"text":"text"})).is_err());
    }
}
#[tokio::test]
async fn store_corruption_symlink_bounds_unknown_keys() {
    let d = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR").to_string() + "/.work").unwrap();
    let t = context(d.path());
    let p = t.context.paths.data_dir.join("ip-geolocation");
    std::fs::create_dir_all(&p).unwrap();
    let f = p.join("storage.json");
    std::fs::write(&f, b"{\"unrelated\":{\"keep\":true}}").unwrap();
    tezbar_ip_geolocation::store::update(&t.context, Some(json!({"ipType":"Public"})), None)
        .await
        .unwrap();
    assert_eq!(
        tezbar_ip_geolocation::store::read(&t.context)
            .await
            .unwrap()["unrelated"],
        json!({"keep":true})
    );
    for bytes in [
        b"[]".to_vec(),
        b"bad".to_vec(),
        vec![b' '; 262145],
        b"{\"nativeV2\":{\"schema\":2}}".to_vec(),
    ] {
        std::fs::write(&f, bytes).unwrap();
        assert!(tezbar_ip_geolocation::store::read(&t.context)
            .await
            .is_err());
    }
    #[cfg(unix)]
    {
        std::fs::remove_file(&f).unwrap();
        std::os::unix::fs::symlink(d.path().join("outside"), &f).unwrap();
        assert!(tezbar_ip_geolocation::store::read(&t.context)
            .await
            .is_err());
    }
}

#[tokio::test]
async fn relevant_preferences_isolate_cache_entries_and_sdk_private_paths() {
    let d = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR").to_string() + "/.work").unwrap();
    let t = context(d.path());
    let m = Mock::default();
    let a = Adapter::new(m.clone());
    for (id, language, format) in [
        ("en", "en", "latLon"),
        ("ru", "ru", "latLon"),
        ("coords", "en", "lonLat"),
    ] {
        let mut state = json!({});
        let result=t.run(a.handle(&t.context,"query-ip-geolocation","extension:run-command",&json!({"sessionId":id,"argumentValues":{"ipAddress":"x.test"},"preferences":{"language":language,"coordinatesFormat":format}}),&mut state)).await.unwrap();
        assert_eq!(result["mode"], "view");
    }
    let saved = tezbar_ip_geolocation::store::read(&t.context)
        .await
        .unwrap();
    assert_eq!(saved["nativeV2"]["cache"].as_array().unwrap().len(), 3);
    assert_eq!(
        m.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_str() == "geo:x.test:ru")
            .count(),
        1
    );
    let other = context(&d.path().join("other"));
    assert_eq!(
        tezbar_ip_geolocation::store::read(&other.context)
            .await
            .unwrap(),
        json!({})
    );
}

#[tokio::test]
async fn result_cache_expiry_and_capacity_are_bounded() {
    struct Failed {
        now: u64,
    }
    impl Services for Failed {
        fn interfaces(&self) -> Result<Vec<Interface>, String> {
            panic!("No interface query expected")
        }
        fn public_ip(&self, _: u8) -> BoxFuture<'_, Result<String, String>> {
            panic!("No public lookup expected")
        }
        fn geolocation<'a>(
            &'a self,
            _: &'a str,
            _: &'a str,
        ) -> BoxFuture<'a, Result<Value, String>> {
            Box::pin(async { Err("synthetic failure".into()) })
        }
        fn now_ms(&self) -> u64 {
            self.now
        }
    }
    let d = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR").to_string() + "/.work").unwrap();
    let t = context(d.path());
    let a = Adapter::new(Mock::default());
    let mut state = json!({});
    t.run(a.handle(
        &t.context,
        "query-ip-geolocation",
        "extension:run-command",
        &json!({"sessionId":"seed","argumentValues":{"ipAddress":"x.test"},"preferences":{}}),
        &mut state,
    ))
    .await
    .unwrap();
    for (now, cached) in [(301000, true), (301001, false), (999, false)] {
        let a = Adapter::new(Failed { now });
        let mut state = json!({});
        let v=t.run(a.handle(&t.context,"query-ip-geolocation","extension:run-command",&json!({"sessionId":"fail","argumentValues":{"ipAddress":"x.test"},"preferences":{}}),&mut state)).await.unwrap();
        assert_eq!(v["root"]["children"][0]["type"] == "List.Item", cached);
        assert!(v["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["title"] == "Retry Lookup"));
    }
    for n in 0..34 {
        tezbar_ip_geolocation::store::update(
            &t.context,
            None,
            Some(json!({"key":format!("{n:064x}"),"at":1000,"rows":[]})),
        )
        .await
        .unwrap();
    }
    let store = tezbar_ip_geolocation::store::read(&t.context)
        .await
        .unwrap();
    assert_eq!(store["nativeV2"]["cache"].as_array().unwrap().len(), 32);
    assert_eq!(store["nativeV2"]["cache"][0]["key"], format!("{:064x}", 2));
}
