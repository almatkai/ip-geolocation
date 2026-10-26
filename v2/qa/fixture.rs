//! Synthetic executable uses the SAME adapter/SDK. Never ships in the dev ZIP.
use serde_json::{json, Value};
use tezbar_extension_sdk::BoxFuture;
use tezbar_ip_geolocation::{core::Interface, services::Services, Adapter};
struct Fixture;
impl Services for Fixture {
    fn interfaces(&self) -> Result<Vec<Interface>, String> {
        Ok(vec![
            Interface {
                name: "synthetic0".into(),
                address: "192.0.2.7".into(),
                family: 4,
                internal: false,
            },
            Interface {
                name: "synthetic0".into(),
                address: "2001:db8::7".into(),
                family: 6,
                internal: false,
            },
        ])
    }
    fn public_ip(&self, f: u8) -> BoxFuture<'_, Result<String, String>> {
        Box::pin(async move {
            Ok(if f == 4 {
                "198.51.100.4"
            } else {
                "2001:db8::4"
            }
            .into())
        })
    }
    fn geolocation<'a>(&'a self, q: &'a str, _: &'a str) -> BoxFuture<'a, Result<Value, String>> {
        Box::pin(async move {
            if q == "slow.example" {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            if q == "fail.example" {
                return Err("synthetic failure".into());
            }
            Ok(
                json!({"status":"success","query":q,"country":"Test","regionName":"Region","city":"City","district":"","zip":"","lat":0,"lon":-2,"timezone":"Etc/UTC","as":"AS0 Test","isp":"Fixture ISP","org":"Fixture Org"}),
            )
        })
    }
    fn now_ms(&self) -> u64 {
        1000
    }
}
#[tokio::main(worker_threads = 2)]
async fn main() {
    if tezbar_extension_sdk::serve(Adapter::new(Fixture))
        .await
        .is_err()
    {
        std::process::exit(1);
    }
}
