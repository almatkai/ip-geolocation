#!/usr/bin/env python3
"""Generate unapplied generic patch from immutable extracted host files only."""
from pathlib import Path
import difflib
BASE=Path(__file__).resolve().parents[1]
BEFORE=BASE/'qa/host';AFTER=BASE/'qa/host-after'
changes={}
p='src-tauri/src/native_core/extensions/registry.rs';s=(BEFORE/p).read_text()
s=s.replace('    path: PathBuf,\n    manifest:', '    path: PathBuf,\n    resource_path: PathBuf,\n    manifest:')
s=s.replace('            let mut manifest = if native {','            let mut resource_path = path.clone();\n            let mut manifest = if native {').replace('Ok(package) => package.manifest,','Ok(package) => { resource_path = package.dir; package.manifest },')
s=s.replace('                path,\n                manifest,','                path,\n                resource_path,\n                manifest,').replace('        path,\n        manifest: Value::Null,','        resource_path: path.clone(),\n        path,\n        manifest: Value::Null,')
s=s.replace('extension.path.join(relative)','extension.resource_path.join(relative)').replace('extension.path.join("assets").join(relative)','extension.resource_path.join("assets").join(relative)')
s+='''
// Input manifests here have ALREADY passed process_install::resolve verification.
pub(super) fn verified_alias_from(requested: &str, command: &str, target: &str, manifests: &[Value]) -> Result<Option<String>, String> {
    let owners: Vec<_> = manifests.iter().filter(|m| m["replaces"].as_array().is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(requested)))).collect();
    if owners.len() > 1 { return Err("Ambiguous verified replacement owners".into()); }
    let Some(owner) = owners.first() else { return Ok(None) };
    let root_compatible = owner["platforms"].as_array().map_or(target == "macOS", |platforms| platforms.iter().any(|p| p == target));
    let compatible = root_compatible && owner["commands"].as_array().is_some_and(|commands| commands.iter().any(|c| c["name"] == command && c["runtime"] == "rust" && c["platforms"].as_array().is_some_and(|p| p.iter().any(|p| p == target))));
    Ok(if compatible { owner["id"].as_str().map(str::to_owned) } else { None })
}
pub(super) fn verified_alias(ctx: &CoreContext, requested: &str, command: &str) -> Result<Option<String>, String> {
    let mut manifests = Vec::new(); let mut seen = HashSet::new(); let mut count = 0;
    for root in managed_roots(ctx) {
        let entries = match std::fs::read_dir(root) { Ok(e) => e, Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue, Err(_) => return Err("Cannot inspect installed packages".into()) };
        for entry in entries {
            count += 1; if count > 4096 { return Err("Installed package scan exceeds capacity".into()); }
            let entry = entry.map_err(|_| "Cannot inspect installed package")?;
            let name = entry.file_name(); let Some(id) = name.to_str() else { continue };
            if !id.starts_with("com.tezbar.native.") || !seen.insert(id.to_string()) { continue; }
            if let Ok(package) = super::process_install::resolve(ctx, id) { manifests.push(package.manifest); }
            if manifests.len() > 128 { return Err("Verified package scan exceeds capacity".into()); }
        }
    }
    verified_alias_from(requested, command, platform(), &manifests)
}
'''
changes[p]=s
p='src-tauri/src/native_core/extensions/native.rs';s=(BEFORE/p).read_text();old='''            if requested.starts_with("com.tezbar.native.") || v2_runtime::slug(requested).is_some()
            {
                self.launch(ctx, files, body).await
            } else {
                return None;
            }''';new='''            let canonical = if requested.starts_with("com.tezbar.native.") { Some(requested.to_string()) } else {
                match registry::verified_alias(ctx, requested, body["commandName"].as_str().unwrap_or("")) {
                    Ok(id) => id,
                    Err(message) => return Some(Ok(json!({"ok":false,"message":message}))),
                }
            };
            if let Some(canonical) = canonical {
                let mut normalized = body.clone(); normalized["extensionId"] = json!(canonical);
                self.launch(ctx, files, &normalized).await
            } else { return None; }'''
assert old in s;s=s.replace(old,new);changes[p]=s
p='src/renderer/ExtensionRuntimeView.tsx';s=(BEFORE/p).read_text();old='''              if (result.mode === 'no-view') {
                setState((prev) => ({ ...prev, message: undefined }))
                return
              }''';new='''              if (result.mode === 'no-view') {
                currentSession.current = null
                await window.tezbar.extensionDisposeSession({ sessionId: state.sessionId })
                onBack()
                return
              }'''
assert old in s;s=s.replace(old,new);changes[p]=s
patch=[]
for p,s in changes.items():
 d=AFTER/p;d.parent.mkdir(parents=True,exist_ok=True);d.write_text(s)
 patch.extend(difflib.unified_diff((BEFORE/p).read_text().splitlines(True),s.splitlines(True),fromfile='a/'+p,tofile='b/'+p))
(BASE/'docs/host-prerequisites.patch').write_text(''.join(patch))
print('Unapplied patch generated from frozen host; app untouched')
