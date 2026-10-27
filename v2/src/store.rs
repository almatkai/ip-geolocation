//! Only source Args + bounded cached rows, in the SDK private data path.
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Component, Path},
    sync::Mutex,
};
use tezbar_extension_sdk::{CoreResult, ExtensionContext};
const MAX_BYTES: u64 = 262_144;
static CHECK_LOCK: Mutex<()> = Mutex::new(());
pub fn validate_path(data: &Path) -> Result<(), String> {
    if !data.is_absolute() {
        return Err("Store path must be absolute".into());
    }
    let mut path = std::path::PathBuf::new();
    for c in data.components().chain([
        Component::Normal(std::ffi::OsStr::new("ip-geolocation")),
        Component::Normal(std::ffi::OsStr::new("storage.json")),
    ]) {
        if matches!(c, Component::ParentDir) {
            return Err("Invalid store path".into());
        }
        path.push(c.as_os_str());
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_symlink() => return Err("Symlink store path refused".into()),
            Ok(m)
                if path.file_name() == Some(std::ffi::OsStr::new("storage.json"))
                    && (!m.is_file() || m.len() > MAX_BYTES) =>
            {
                return Err("Invalid or oversized store".into())
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Cannot inspect store".into()),
        }
    }
    Ok(())
}
fn read_bounded(ctx: &ExtensionContext) -> CoreResult {
    validate_path(&ctx.paths.data_dir)?;
    let path = ctx.paths.data_dir.join("ip-geolocation/storage.json");
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = match options.open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(_) => return Err("Cannot read private store".into()),
    };
    if !file
        .metadata()
        .map_err(|_| "Cannot inspect private store")?
        .is_file()
    {
        return Err("Invalid private store file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read private store")?;
    if bytes.len() > MAX_BYTES as usize {
        return Err("Private store exceeds capacity".into());
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| "Corrupt private store")?;
    if !v.is_object() {
        return Err("Private store must be an object".into());
    }
    if v.get("nativeV2").is_some() {
        validate(&v["nativeV2"])?;
    }
    Ok(v)
}
pub async fn read(ctx: &ExtensionContext) -> CoreResult {
    let _guard = CHECK_LOCK.lock().map_err(|_| "Store lock unavailable")?;
    read_bounded(ctx)
}
fn validate(v: &Value) -> Result<(), String> {
    if !v.is_object()
        || v["schema"] != 1
        || !v["args"].is_object()
        || !v["cache"].is_array()
        || v["cache"].as_array().is_some_and(|a| a.len() > 32)
    {
        return Err("Invalid private store schema".into());
    }
    for k in ["ipType", "IpVersion"] {
        if v["args"]
            .get(k)
            .is_some_and(|v| !v.is_string() || v.as_str().is_some_and(|s| s.len() > 256))
        {
            return Err("Invalid stored argument".into());
        }
    }
    for e in v["cache"].as_array().unwrap() {
        if e["key"].as_str().is_none_or(|s| s.len() != 64)
            || e["at"].as_u64().is_none()
            || serde_json::from_value::<crate::core::Rows>(e["rows"].clone()).is_err()
            || e["rows"].as_array().is_none_or(|a| a.len() > 10)
        {
            return Err("Invalid cached result".into());
        }
    }
    Ok(())
}
pub async fn update(
    ctx: &ExtensionContext,
    args: Option<Value>,
    entry: Option<Value>,
) -> CoreResult {
    let _guard = CHECK_LOCK.lock().map_err(|_| "Store lock unavailable")?;
    let mut value = read_bounded(ctx)?;
    let v = &mut value;
    {
        if v.get("nativeV2").is_none() {
            v["nativeV2"] = json!({"schema":1,"args":{},"cache":[]});
        }
        validate(&v["nativeV2"])?;
        if let Some(args) = args {
            for (k, a) in args.as_object().ok_or("Invalid arguments")? {
                v["nativeV2"]["args"][k] = a.clone();
            }
        }
        if let Some(entry) = entry {
            let cache = v["nativeV2"]["cache"].as_array_mut().unwrap();
            cache.retain(|e| e["key"] != entry["key"]);
            cache.push(entry);
            if cache.len() > 32 {
                cache.remove(0);
            }
        }
        validate(&v["nativeV2"])?;
        if serde_json::to_vec(v)
            .map_err(|_| "Invalid private store")?
            .len()
            > MAX_BYTES as usize
        {
            return Err("Private store capacity exceeded".into());
        }
    }
    let parent = ctx.paths.data_dir.join("ip-geolocation");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&parent)
        .map_err(|_| "Cannot create private store directory")?;
    validate_path(&ctx.paths.data_dir)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(&parent).map_err(|_| "Cannot stage private store")?;
    let bytes = serde_json::to_vec(&value).map_err(|_| "Invalid private store")?;
    temporary
        .write_all(&bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| "Cannot write private store")?;
    temporary
        .persist(parent.join("storage.json"))
        .map_err(|_| "Cannot replace private store")?;
    #[cfg(unix)]
    std::fs::File::open(&parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync private store")?;
    Ok(value)
}
