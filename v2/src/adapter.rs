use crate::{
    core::{self, Preferences, Rows},
    services::{Production, Services, GEO_COOLDOWN},
    store,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tezbar_extension_sdk::{
    BoxFuture, CommandConcurrency, CoreResult, Extension, ExtensionContext,
};

const COMMANDS: &[&str] = &["query-ip-geolocation", "my-ip-geolocation", "copy-ip"];
#[derive(Clone)]
enum Effect {
    Copy(String),
    HideAfterCopy,
    Settings,
    Fetch,
}
#[derive(Default)]
struct Authority {
    actions: HashMap<String, Effect>,
    pending: Option<Effect>,
    revision: u64,
    owner_nonce: u128,
}
pub struct Adapter {
    services: Arc<dyn Services>,
    authority: Arc<Mutex<HashMap<String, Authority>>>,
}
#[derive(Clone)]
struct OpenGuard {
    owner: Arc<Mutex<HashMap<String, Authority>>>,
    id: String,
    armed: bool,
    owner_nonce: u128,
}
impl Drop for OpenGuard {
    fn drop(&mut self) {
        if self.armed {
            if let Ok(mut map) = self.owner.lock() {
                if map
                    .get(&self.id)
                    .is_some_and(|a| a.owner_nonce == self.owner_nonce)
                {
                    map.remove(&self.id);
                }
            }
        }
    }
}
impl Adapter {
    pub fn production() -> Self {
        Self::new(Production::default())
    }
    pub fn new(services: impl Services) -> Self {
        Self {
            services: Arc::new(services),
            authority: Arc::default(),
        }
    }
    fn reset(&self, id: &str) -> Result<(), String> {
        let mut map = self
            .authority
            .lock()
            .map_err(|_| "Action ownership unavailable")?;
        if !map.contains_key(id) && map.len() >= 128 {
            return Err("Session capacity reached".into());
        }
        let a = map.entry(id.into()).or_insert_with(|| Authority {
            owner_nonce: uuid::Uuid::new_v4().as_u128(),
            ..Authority::default()
        });
        a.actions.clear();
        a.pending = None;
        a.revision = a.revision.checked_add(1).ok_or("Revision exhausted")?;
        Ok(())
    }
    fn consume(&self, id: &str, action: &str) -> Result<Effect, String> {
        let mut map = self
            .authority
            .lock()
            .map_err(|_| "Action ownership unavailable")?;
        let a = map.get_mut(id).ok_or("Session no longer owns actions")?;
        let effect = a
            .actions
            .remove(action)
            .ok_or("Action expired; refresh the view")?;
        a.actions.clear();
        a.pending = Some(effect.clone());
        Ok(effect)
    }
    fn add_action(
        &self,
        id: &str,
        title: &str,
        effect: Effect,
        actions: &mut Vec<Value>,
    ) -> Result<String, String> {
        let mut map = self
            .authority
            .lock()
            .map_err(|_| "Action ownership unavailable")?;
        let a = map.get_mut(id).ok_or("Session no longer owns actions")?;
        if a.actions.len() >= 64 {
            return Err("Too many actions".into());
        }
        let action_id = format!("{}:{}", a.revision, uuid::Uuid::new_v4());
        let kind = if matches!(effect, Effect::Copy(_)) {
            "copy"
        } else {
            "action"
        };
        a.actions.insert(action_id.clone(), effect);
        actions.push(json!({"id":action_id,"title":title,"kind":kind}));
        Ok(action_id)
    }
    fn render(
        &self,
        id: &str,
        command: &str,
        state: &Value,
        message: Option<&str>,
        recovery: Option<Effect>,
    ) -> CoreResult {
        let rows: Rows = serde_json::from_value(state["rows"].clone()).unwrap_or_default();
        let query = state["query"].as_str().unwrap_or("");
        let mut actions = Vec::new();
        let mut children = Vec::new();
        if let Some(effect) = recovery {
            // Keep the checkpoint even if this recovery render is discarded/canceled.
            self.authority
                .lock()
                .map_err(|_| "Action ownership unavailable")?
                .get_mut(id)
                .ok_or("Session missing")?
                .pending = Some(effect.clone());
            let title = if matches!(effect, Effect::Copy(_)) {
                "Retry Exact Copy"
            } else if matches!(effect, Effect::HideAfterCopy) {
                "Retry Close Window"
            } else {
                "Retry"
            };
            self.add_action(id, title, effect, &mut actions)?;
        }
        let all = core::copy_all(&rows);
        for (index, (key, value)) in rows.iter().enumerate() {
            if command == "my-ip-geolocation" && value.is_empty() {
                continue;
            }
            let copy = self.add_action(
                id,
                &format!("Copy {key}"),
                Effect::Copy(value.clone()),
                &mut actions,
            )?;
            let copy_all =
                self.add_action(id, "Copy All Info", Effect::Copy(all.clone()), &mut actions)?;
            // Text is rendered as React labels, never interpreted as Markdown or remote images.
            children.push(json!({"type":"List.Item","props":{"id":format!("row-{index}"),"title":key,"subtitle":value,"actionIds":[copy,copy_all],"icon":row_icon(command,index)}}));
        }
        if state["retry"].as_bool() == Some(true) {
            self.add_action(id, "Retry Lookup", Effect::Fetch, &mut actions)?;
        }
        if rows.is_empty() {
            children.push(json!({"type":"List.EmptyView","props":{"title":if command=="query-ip-geolocation"{if query.is_empty(){"IP Geolocation"}else{"Invalid Query"}}else{"No Geolocation Info"},"icon":"Icon.AirplaneTakeoff"}}));
        }
        // The selected host exposes one generic settings surface; both source entries stay named.
        if command == "my-ip-geolocation" || rows.is_empty() {
            self.add_action(id, "Configure Command", Effect::Settings, &mut actions)?;
        }
        self.add_action(id, "Configure Extension", Effect::Settings, &mut actions)?;
        let mut view = tezbar_extension_sdk::view(
            if command == "query-ip-geolocation" {
                "Query IP Geolocation"
            } else if command == "my-ip-geolocation" {
                "My IP Geolocation"
            } else {
                "Copy IP"
            },
            json!({"type":"List","props":{"searchText":query,"searchBarPlaceholder":if command=="query-ip-geolocation"{"Query geolocation of IP or domain"}else{"My IP Geolocation"},"__hasServerSearch":command=="query-ip-geolocation"},"children":children}),
            actions,
        );
        view["refreshMs"] = Value::Null;
        if let Some(m) = message {
            view["message"] = json!(m);
        }
        Ok(view)
    }
    async fn fetch(
        &self,
        ctx: &ExtensionContext,
        command: &str,
        state: &mut Value,
        prefs: &Preferences,
    ) -> CoreResult {
        let id = state["id"].as_str().ok_or("Missing session")?.to_owned();
        self.reset(&id)?;
        let query = state["query"].as_str().unwrap_or("").to_owned();
        let key = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&json!([command, query, prefs])).unwrap())
        );
        let saved = store::read(ctx).await?;
        let now = self.services.now_ms();
        let cached = saved["nativeV2"]["cache"]
            .as_array()
            .and_then(|cache| {
                cache.iter().rev().find(|e| {
                    e["key"] == key
                        && e["at"]
                            .as_u64()
                            .is_some_and(|at| at <= now && now - at <= 300_000)
                })
            })
            .map(|e| e["rows"].clone());
        let mut source_failed = false;
        let result: Result<Rows, String> = if command == "query-ip-geolocation" {
            match core::parse_query(&query)? {
                None => Ok(Vec::new()),
                Some(query) => match self.services.geolocation(&query, &prefs.language).await {
                    Ok(v) => {
                        source_failed = v["status"] != "success";
                        core::geo_rows(&v, prefs)
                    }
                    Err(e) => Err(e),
                },
            }
        } else {
            let interfaces = match self.services.interfaces() {
                Ok(v) => v,
                Err(_) => {
                    state["rows"] = json!([]);
                    state["retry"] = json!(true);
                    return self.render(
                        &id,
                        command,
                        state,
                        Some("Cannot read OS interfaces. Select Retry Lookup."),
                        None,
                    );
                }
            };
            let local4 = core::local(&interfaces, 4);
            let local6 = if prefs.show_ipv6 {
                core::local(&interfaces, 6)
            } else {
                String::new()
            };
            let public4 = self.services.public_ip(4).await.unwrap_or_default();
            let public6 = if prefs.show_ipv6 {
                self.services.public_ip(6).await.unwrap_or_default()
            } else {
                String::new()
            };
            match self.services.geolocation(&public4, &prefs.language).await {
                Ok(v) => {
                    source_failed = v["status"] != "success";
                    core::my_rows(&v, prefs, local4, local6, public4, public6)
                }
                Err(e) => {
                    // Source fail response still exposes addresses and its literal "undefined" fallback.
                    state["rows"] = serde_json::to_value(core::my_rows(
                        &json!({"status":"fail"}),
                        prefs,
                        local4,
                        local6,
                        public4,
                        public6,
                    )?)
                    .unwrap();
                    state["retry"] = json!(true);
                    return self.render(&id, command, state, Some(safe_lookup_error(&e)), None);
                }
            }
        };
        match result {
            Ok(rows) => {
                if rows.len() > 10 || rows.iter().any(|(k, v)| k.len() > 128 || v.len() > 8192) {
                    state["rows"] = json!([]);
                    state["retry"] = json!(true);
                    return self.render(
                        &id,
                        command,
                        state,
                        Some("Provider result exceeds display bounds. Select Retry Lookup."),
                        None,
                    );
                }
                state["rows"] = serde_json::to_value(&rows).unwrap();
                state["retry"] = json!(
                    source_failed
                        || (command == "query-ip-geolocation"
                            && rows.is_empty()
                            && core::parse_query(&query)?.is_some())
                );
                if !source_failed {
                    store::update(ctx, None, Some(json!({"key":key,"at":now,"rows":rows}))).await?;
                }
                self.render(
                    &id,
                    command,
                    state,
                    if source_failed {
                        Some("Geolocation service returned no result. Select Retry Lookup.")
                    } else {
                        None
                    },
                    None,
                )
            }
            Err(e) => {
                state["rows"] = cached.unwrap_or(json!([]));
                state["retry"] = json!(true);
                let notice = if state["rows"].as_array().is_some_and(|a| !a.is_empty()) {
                    if e == GEO_COOLDOWN {
                        "Geolocation service is cooling down. Showing a cached result up to five minutes old. Select Retry Lookup after the server cooldown."
                    } else {
                        "Lookup failed. Showing a cached result up to five minutes old. Select Retry Lookup."
                    }
                } else {
                    safe_lookup_error(&e)
                };
                self.render(&id, command, state, Some(notice), None)
            }
        }
    }
    async fn copy_launch(
        &self,
        ctx: &ExtensionContext,
        payload: &Value,
        state: &mut Value,
    ) -> CoreResult {
        let id = state["id"].as_str().ok_or("Missing session")?.to_owned();
        let old = store::read(ctx).await?;
        let arguments = &payload["argumentValues"];
        let mut args = json!({});
        for (arg, key) in [("ipType", "ipType"), ("ipVersion", "IpVersion")] {
            let value = arguments.get(arg).cloned().unwrap_or_else(|| {
                old["nativeV2"]["args"][key]
                    .as_str()
                    .map_or(json!(""), |s| json!(s))
            });
            if value.as_str().is_none_or(|s| s.len() > 256) {
                return Err("Invalid Copy IP argument".into());
            }
            args[key] = value;
        }
        // Source caches both argument values BEFORE hide/lookup/copy.
        store::update(ctx, Some(args.clone()), None).await?;
        let public = args["ipType"] == "Public";
        let family = if args["IpVersion"] == "IPv6" { 6 } else { 4 };
        let user = payload["launchContext"]["launchType"].as_str() != Some("background");
        if user
            && host(ctx, "host.window.visibility", json!({"visible":false}))
                .await
                .is_err()
        {
            return self.render(
                &id,
                "copy-ip",
                state,
                Some("Could not hide the window. Reopen Copy IP to try again."),
                None,
            );
        }
        let ip = if public {
            self.services.public_ip(family).await.unwrap_or_default()
        } else {
            match self.services.interfaces() {
                Ok(i) => core::local(&i, family),
                Err(_) => String::new(),
            }
        };
        if !user {
            return Ok(tezbar_extension_sdk::no_view(
                "Background metadata refresh is not scheduled by this event-driven port",
            ));
        }
        if ip.is_empty() {
            state["retry"] = json!(true);
            let _ = host(ctx, "host.window.visibility", json!({"visible":true})).await;
            return self.render(
                &id,
                "copy-ip",
                state,
                Some("Failed to get IP address. Select Retry Lookup."),
                None,
            );
        }
        // Capture exact bytes before the irreversible call; never re-lookup after an unknown ACK.
        {
            let mut a = self
                .authority
                .lock()
                .map_err(|_| "Action ownership unavailable")?;
            a.get_mut(&id).ok_or("Session missing")?.pending = Some(Effect::Copy(ip.clone()));
        }
        if host(ctx, "host.clipboard.write", json!({"text":ip}))
            .await
            .is_err()
        {
            let _ = host(ctx, "host.window.visibility", json!({"visible":true})).await;
            return self.render(&id,"copy-ip",state,Some("Copy outcome is unknown. Retry Exact Copy sends the same bytes only when selected."),Some(Effect::Copy(ip)));
        }
        self.authority
            .lock()
            .map_err(|_| "Action ownership unavailable")?
            .remove(&id);
        Ok(tezbar_extension_sdk::no_view(&format!(
            "🅿️ {} IPv{} {}",
            if public { "Public" } else { "Local" },
            family,
            ip
        )))
    }
}
fn row_icon(command: &str, index: usize) -> &'static str {
    const QUERY: [&str; 7] = [
        "Icon.Globe",
        "Icon.Map",
        "Icon.Pin",
        "Icon.Clock",
        "Icon.EditShape",
        "Icon.Network",
        "Icon.Building",
    ];
    const MY: [&str; 10] = [
        "Icon.Info",
        "Icon.Cd",
        "Icon.Globe",
        "Icon.BullsEye",
        "Icon.Map",
        "Icon.Pin",
        "Icon.Clock",
        "Icon.EditShape",
        "Icon.Network",
        "Icon.Building",
    ];
    if command == "my-ip-geolocation" {
        MY.get(index).copied().unwrap_or("Icon.Info")
    } else {
        QUERY.get(index).copied().unwrap_or("Icon.Info")
    }
}
fn safe_lookup_error(error: &str) -> &'static str {
    if error == GEO_COOLDOWN {
        GEO_COOLDOWN
    } else {
        "Lookup failed. Select Retry Lookup to try again."
    }
}
pub async fn host(ctx: &ExtensionContext, method: &str, params: Value) -> CoreResult {
    let value = tokio::time::timeout(Duration::from_secs(10), ctx.host(method, params))
        .await
        .map_err(|_| "Host action outcome is unknown")?
        .map_err(|_| "Host action outcome is unknown")?;
    validate_host_result(method, value)
}
pub fn validate_host_result(method: &str, value: Value) -> CoreResult {
    if method == "host.window.visibility" {
        if value.is_null() {
            return Ok(value);
        }
        return Err("Invalid visibility response".into());
    }
    if ["host.clipboard.read", "host.selection.read"].contains(&method) {
        if value.is_string() {
            return Ok(value);
        }
        return Err("Invalid host text response".into());
    }
    if ![
        "host.clipboard.write",
        "host.clipboard.pasteText",
        "host.shell.open",
        "host.settings.open",
    ]
    .contains(&method)
    {
        return Err("Unimplemented host contract".into());
    }
    if let Some(o) = value.as_object() {
        if o.get("ok") == Some(&json!(true))
            && o.keys().all(|k| ["ok", "message"].contains(&k.as_str()))
            && o.get("message").is_none_or(Value::is_string)
        {
            return Ok(value);
        }
    }
    Err("Invalid host acknowledgement; outcome is unknown".into())
}
impl Extension for Adapter {
    fn id(&self) -> &'static str {
        "com.tezbar.native.ip-geolocation"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn commands(&self) -> &'static [&'static str] {
        COMMANDS
    }
    fn concurrency(&self, _: &str) -> CommandConcurrency {
        CommandConcurrency::Serialize
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        payload: &'a Value,
        state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            let id = payload["sessionId"].as_str().ok_or("Missing sessionId")?;
            if operation == "extension:dispose-session" {
                self.authority
                    .lock()
                    .map_err(|_| "Action ownership unavailable")?
                    .remove(id);
                return Ok(json!({}));
            }
            if !COMMANDS.contains(&command) {
                return Err("Unknown command".into());
            }
            let prefs = Preferences::parse(&payload["preferences"])?;
            let opening = operation == "extension:run-command";
            let mut guard = OpenGuard {
                owner: self.authority.clone(),
                id: id.into(),
                armed: false,
                owner_nonce: 0,
            };
            if opening {
                self.reset(id)?;
                guard.owner_nonce = self
                    .authority
                    .lock()
                    .map_err(|_| "Action ownership unavailable")?
                    .get(id)
                    .ok_or("Session missing")?
                    .owner_nonce;
                guard.armed = true;
                // One non-polling SDK-owned drop guard closes the commit/cancel race.
                // It captures no context/scope, creates no ownership cycle, and is
                // dropped even if SDK cancellation happens after this handler returns.
                let scope_guard = guard.clone();
                ctx.spawn(async move {
                    let _scope_guard = scope_guard;
                    std::future::pending::<()>().await;
                })?;
                let q = payload["argumentValues"]["ipAddress"]
                    .as_str()
                    .map(core::js_trim)
                    .unwrap_or("");
                *state = json!({"id":id,"query":q,"rows":[],"retry":false});
                let result = if command == "copy-ip" {
                    self.copy_launch(ctx, payload, state).await
                } else {
                    self.fetch(ctx, command, state, &prefs).await
                };
                if result.is_ok() {
                    guard.armed = false;
                }
                return result;
            }
            match operation {
                "extension:search-text-changed" => {
                    if command != "query-ip-geolocation" {
                        return self.render(id, command, state, None, None);
                    }
                    let text = payload["searchText"]
                        .as_str()
                        .ok_or("Invalid search text")?;
                    state["query"] = json!(text);
                    self.fetch(ctx, command, state, &prefs).await
                }
                "extension:refresh-session" => {
                    let pending = self
                        .authority
                        .lock()
                        .map_err(|_| "Action ownership unavailable")?
                        .get(id)
                        .and_then(|a| a.pending.clone());
                    if let Some(effect) = pending {
                        self.reset(id)?;
                        self.render(
                            id,
                            command,
                            state,
                            Some("Previous action outcome is unknown; nothing was replayed."),
                            Some(effect),
                        )
                    } else {
                        Ok(tezbar_extension_sdk::unchanged())
                    }
                }
                "extension:invoke-action" => {
                    let effect =
                        self.consume(id, payload["actionId"].as_str().ok_or("Missing action ID")?)?;
                    if matches!(effect, Effect::Fetch) {
                        if command == "copy-ip" {
                            self.reset(id)?;
                            return self.copy_launch(ctx, payload, state).await;
                        }
                        return self.fetch(ctx, command, state, &prefs).await;
                    }
                    let result = match &effect {
                        Effect::Copy(text) => {
                            host(ctx, "host.clipboard.write", json!({"text":text})).await
                        }
                        Effect::HideAfterCopy => {
                            host(ctx, "host.window.visibility", json!({"visible":false})).await
                        }
                        Effect::Settings => {
                            host(
                                ctx,
                                "host.settings.open",
                                json!({"surface":"settings-extensions"}),
                            )
                            .await
                        }
                        Effect::Fetch => unreachable!(),
                    };
                    if result.is_err() {
                        self.reset(id)?;
                        return self.render(
                            id,
                            command,
                            state,
                            Some("Action outcome is unknown. No automatic replay."),
                            Some(effect),
                        );
                    }
                    if matches!(effect, Effect::Copy(_)) {
                        // Clipboard ACK is known. A canceled/failed close may retry ONLY close.
                        {
                            let mut map = self
                                .authority
                                .lock()
                                .map_err(|_| "Action ownership unavailable")?;
                            map.get_mut(id).ok_or("Session missing")?.pending =
                                Some(Effect::HideAfterCopy);
                        }
                        if host(ctx, "host.window.visibility", json!({"visible":false}))
                            .await
                            .is_err()
                        {
                            self.reset(id)?;
                            return self.render(
                                id,
                                command,
                                state,
                                Some("Copied. Window close outcome is unknown."),
                                Some(Effect::HideAfterCopy),
                            );
                        }
                        self.reset(id)?;
                        return Ok(tezbar_extension_sdk::no_view("Copied"));
                    }
                    self.reset(id)?;
                    if matches!(effect, Effect::HideAfterCopy) {
                        Ok(tezbar_extension_sdk::no_view("Copied"))
                    } else {
                        self.render(
                            id,
                            command,
                            state,
                            Some("Preferences apply after reopening the command."),
                            None,
                        )
                    }
                }
                "extension:load-more" => Ok(tezbar_extension_sdk::unchanged()),
                _ => Err("Unsupported operation".into()),
            }
        })
    }
}
