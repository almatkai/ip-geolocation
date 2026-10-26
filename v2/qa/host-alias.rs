pub(super) fn verified_alias_from(requested: &str, command: &str, target: &str, manifests: &[Value]) -> Result<Option<String>, String> {
    let owners: Vec<_> = manifests.iter().filter(|m| m["replaces"].as_array().is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(requested)))).collect();
    if owners.len() > 1 { return Err("Ambiguous verified replacement owners".into()); }
    let Some(owner) = owners.first() else { return Ok(None) };
    let root_compatible = owner["platforms"].as_array().map_or(target == "macOS", |platforms| platforms.iter().any(|p| p == target));
    let compatible = root_compatible && owner["commands"].as_array().is_some_and(|commands| commands.iter().any(|c| c["name"] == command && c["runtime"] == "rust" && c["platforms"].as_array().is_some_and(|p| p.iter().any(|p| p == target))));
    Ok(if compatible { owner["id"].as_str().map(str::to_owned) } else { None })
}
