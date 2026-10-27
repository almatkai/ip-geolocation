use serde_json::{json, Value};
mod alias {
    use serde_json::Value;
    include!("../qa/host-alias.rs");
}
#[test]
fn exact_verified_alias_owner_command_platform() {
    let package = json!({"id":"com.tezbar.native.ip-geolocation","replaces":["com.tezbar.ip-geolocation","raycast.ip-geolocation"],"commands":[{"name":"copy-ip","runtime":"rust","platforms":["macOS","Windows"]}]});
    for id in ["com.tezbar.ip-geolocation", "raycast.ip-geolocation"] {
        assert_eq!(
            alias::verified_alias_from(id, "copy-ip", "macOS", std::slice::from_ref(&package))
                .unwrap(),
            Some("com.tezbar.native.ip-geolocation".into())
        );
    }
    for id in [
        "ip-geolocation",
        "evil.raycast.ip-geolocation",
        "Raycast.ip-geolocation",
        "raycast.ip-geolocation-extra",
    ] {
        assert!(
            alias::verified_alias_from(id, "copy-ip", "macOS", std::slice::from_ref(&package))
                .unwrap()
                .is_none()
        );
    }
    assert!(alias::verified_alias_from(
        "raycast.ip-geolocation",
        "unknown",
        "macOS",
        std::slice::from_ref(&package)
    )
    .unwrap()
    .is_none());
    assert!(alias::verified_alias_from(
        "raycast.ip-geolocation",
        "copy-ip",
        "Linux",
        std::slice::from_ref(&package)
    )
    .unwrap()
    .is_none());
    assert!(
        alias::verified_alias_from("raycast.ip-geolocation", "copy-ip", "macOS", &[])
            .unwrap()
            .is_none()
    );
    let mut second = package.clone();
    second["id"] = json!("com.tezbar.native.other");
    second["commands"] = json!([]);
    assert!(alias::verified_alias_from(
        "raycast.ip-geolocation",
        "copy-ip",
        "macOS",
        &[package, second]
    )
    .is_err());
}
use std::path::{Component, Path, PathBuf};
struct Installed {
    id: String,
    slug: String,
    path: PathBuf,
    resource_path: PathBuf,
    manifest: Value,
}
mod before {
    use super::*;
    include!("../qa/host-icon-before.rs");
}
mod after {
    use super::*;
    include!("../qa/host-icon-after.rs");
}
#[test]
fn active_resources_and_canonical_preferences_are_distinct() {
    let d = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR").to_string() + "/.work").unwrap();
    let canonical = d.path().join("extensions/com.tezbar.native.ip-geolocation");
    let active = canonical.join("versions/fixture/resources");
    std::fs::create_dir_all(&active).unwrap();
    std::fs::write(active.join("extension-icon.png"), b"synthetic bytes").unwrap();
    let e = Installed {
        id: "com.tezbar.native.ip-geolocation".into(),
        slug: "ip-geolocation".into(),
        path: canonical.clone(),
        resource_path: active.parent().unwrap().into(),
        manifest: json!({"icon":"resources/extension-icon.png"}),
    };
    assert!(before::call(&e).is_none());
    assert_eq!(
        after::call(&e).unwrap(),
        active.join("extension-icon.png").to_string_lossy()
    );
    assert_eq!(e.path, canonical);
}
