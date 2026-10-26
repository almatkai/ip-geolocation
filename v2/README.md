# IP Geolocation — standalone Rust V2

Feature-branch implementation for parent review. It is **not installed or released**.
The original V1 files remain byte-identical, apart from the short link in the root README.

The executable implements all three declared command flows with independent
`tezbar-extension-sdk` 0.1.1 over process protocol 1 / manifest schema 3.
Production uses Rust only. No app, Tauri, sibling SDK path, JS runtime, helpers,
background lease or polling is needed. A single SDK-owned non-polling drop
guard per session cleans volatile action authority on cancellation.

Production geolocation uses the original fixed `http://ip-api.com/json/` free
service, with source `lang` and `fields=585727`. This transport is plaintext:
queries and returned location data have no TLS confidentiality or authenticity.
It sends no authentication or cookies, disables proxies and redirects, and
cannot be redirected by a query or a product endpoint setting. Public-IP services
keep their separate verified HTTPS client and original fallback order. No new
provider, key or preference was added. See [provider evidence](docs/provider-evidence.md).

The real production HTTP client/decoder and `Production::geolocation` entrypoint
are exercised against owned loopback fixtures, including a domain lookup through
the adapter. QA makes no live provider/public-IP/DNS/interface query. Server
cooldown blocks further sends until its deadline; only a new manual Retry Lookup
can try again. There are no background retries, sleeps or polls in the product.

| Command | Source behavior retained | Review boundaries |
| --- | --- | --- |
| `query-ip-geolocation` | Exact trim/prevalidation, URL stripping, provider-side domain resolution, field ordering, labels and copy-all JSON | Fixed original HTTP transport; live provider acceptance not run |
| `my-ip-geolocation` | First eligible local alias per version; public IPv4, optional IPv6; same geolocation formatting and hidden-empty-row behavior | Original HTTP transport; native OS calls compile but were never executed in QA |
| `copy-ip` | Args cache keys `ipType` / `IpVersion`, defaults, automatic no-view launch, hide → lookup → copy order, exact-byte recovery | HUD is the no-view message; hourly metadata/subtitle update has no selected-host generic API and is not scheduled |

Preferences are the original `language`, `coordinatesFormat` and command-local
`showIPv6`, with original defaults. They are immutable SDK-session snapshots;
close and reopen after changing them. Both Configure actions open the host's
single `settings-extensions` surface. This command has no source selection read,
clipboard read, paste, shell action, flags, history, scan or API-key preference.

Source cache behavior is adapted to event-driven views: await a fresh lookup;
on lookup failure, only a same-input/same-preferences result up to five minutes old
may be shown, explicitly marked as cached, with manual Retry Lookup. Source fail
responses and transport failures never refresh successful-cache timestamps. The private store is limited to 256 KiB
and 32 cached snapshots. It preserves unrelated JSON keys. There is no V1-store
migration grant. Existing V1 cache files are never inspected.

Read [QA.md](QA.md), [INTEGRATION.md](INTEGRATION.md),
[exact pins and hashes](docs/provenance.json), and the
[development package inventory](docs/package-report.json).
`docs/host-prerequisites.patch` is separate and **unapplied**.

From `v2/`, `bash scripts/validate.sh` runs the offline/injected checks after the
fixture dependencies are prepared by `scripts/prepare.sh`. That preparation only
downloads pinned public source/dependencies into ignored directories inside this
clone. `bash scripts/build-package.sh` rebuilds from a fresh isolated CargoHome and
target directory, packages the production binary and checks the extracted invalid-input
smoke. `scripts/package.py` accepts that build directory through `CARGO_TARGET_DIR`. ZIPs, all Cargo/npm/Python caches, HOME, temporary stores and extracted
executables stay ignored under `v2/`. No artifact URL/catalog pin is fabricated.
