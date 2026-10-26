# QA evidence — feature branch, 2026-10-26

No extension installation, publication, commit, push, PR, release, app build,
profile read, real clipboard/settings/window operation, OS interface query,
public-IP request, product API call, local scanning or real domain lookup was run.
Only dependency/public primary-source downloads and owned synthetic loopback HTTP/TLS
fixtures used network I/O. All generated HOME/TMP/caches/stores stay under v2/.

## Passed

- Stable rustc/cargo **1.98.0** and **1.85.0**: fmt check, locked all-target test
  suite (**21 tests per toolchain**, including 9 production geo-HTTP tests), strict all-target Clippy `-D warnings`.
  The production core and adapter are the tested library, not a metadata self-test.
- **50 actual-source oracle scenarios**: pinned TSX + real locked public-ip 7.0.1,
  is-ip/ip-regex and supporting locked library code inside an import/effect-closed
  VM. OS interfaces, HTTP, preferences, clock, clipboard/HUD/metadata/settings and
  source Cache are injected. Full output trees, actions, copy-all JSON, calls and
  parameters are in `qa/oracle-results.json`. Source `useCachedPromise` is mocked
  at its async hook boundary to execute the real command's promise; this does not
  pretend to run Raycast's GUI/React scheduler. Locked caching sources are also
  extracted for inspection, and native cache adaptation is tested independently.
- IPv4/IPv6/zone IDs, invalid numeric queries, initial/search trim, URL stripping,
  malformed query delimiters, IDNA delegation, non-JS whitespace, AS substring,
  false/zero/null formatting, JS numbers, all source defaults and coordinates,
  interface loopback/internal flags, private/link-local aliases, missing versions,
  multiple aliases and numeric interface-name enumeration. No source date fields
  exist. Raw declared text fields with malformed non-string values fail safely.
- Locked public-IP HTTPS fallback order, family, 5000 ms timeout/zero retry
  parameters, invalid responses and all-fallback failure compared to Rust's actual
  injected transport loop. No UDP/DNS fallback is invoked by onlyHttps flow.
- Atomic private-store corruption/oversize/symlink/schema/unknown-key checks;
  Args persistence retains `IpVersion` capitalization. Results are bounded to
  32 entries/256 KiB; 5-minute expiry boundary, future-clock rejection, profile
  separation and relevant preference separation are tested. No migration/history.
- Actual production geo HTTP client and decoder, with a `#[cfg(test)]`-only
  loopback routing seam after fixed-origin URL validation. Calls through
  `Production::geolocation` cover IPv4/domain/IPv6/empty self lookup and hostile
  path/query/header delimiters. Captured GET/Host/lang/fields match; no auth,
  proxy authorization or cookies are sent, including after Set-Cookie. The real
  adapter's domain path and My IP use that transport; My IP's other producers
  are synthetic, so no real interface or public-IP discovery occurs.
- Source success/scalar-format oracles run through the actual HTTP parser;
  source fail responses retain source rows. Malformed/non-JSON/partial/invalid
  UTF-8, unsupported status, scalar/field-type errors, depth/node/string/key
  bounds, declared/streamed body overflow, request and whole-operation deadlines
  all fail with fixed messages. Non-followed redirect is verified by the original
  status-specific error; URL/body/secret markers are not reflected in errors.
- Cooldown tests inject monotonic and wall clocks: X-Rl zero/X-Ttl, 429 numeric
  and HTTP-date Retry-After, greater TTL, expired dates, one-hour clamp, malformed
  and duplicate headers. No second request occurs during a known cooldown; a
  fresh manual call after expiry succeeds. Partial body/timeout cannot erase a
  learned cooldown. Dropped futures close their owned loopback socket and release
  serialization; queue wait is included in the operation deadline. No product
  task/timer/poll/replay is added. Geo tests also pass with all proxy environment
  variables pointed at an unusable owned-loopback port and NO_PROXY empty.
- Query/My IP source-fail and transport-error paths never overwrite the previous
  successful snapshot or timestamp. Displayed stale query rows explicitly say
  they are cached and remain subject to the original five-minute bound.
- Actual TLS decoder using a private synthetic fixture key/certificate and injected
  certificate-validation clock, mocked service DNS, owned loopback socket:
  201/301/401/429/500/599, non-followed redirect, invalid UTF-8, chunked/content-length
  overflow, deadline, trusted synthetic TLS success and unknown certificate failure.
  Error strings do not reflect URL/body/token markers.
- Actual schema-3 JSON schema with **13 negative families**; source artifacts stay
  empty and native verification states stay not-run. Draft2020-12 plus URI format
  implementation dependencies are isolated/pinned under qa/requirements.txt.
- Real JSONL pipes to `qa/fixture.rs` (same production adapter/SDK): success for
  all three commands, input changes, reader-order latest query, A-B-A old-copy
  rejection, stale/discarded IDs, unknown ACK, one-use exact-byte retry, cancel
  plus late ACK, refresh recovery, **140 canceled opens** (70 no-view clipboard
  writes and 70 pending geolocation opens), 128-session capacity, profile/Args
  isolation, no-view disposal, explicit dispose and EOF while lookup is pending.
- Extracted actual host Rust asset/alias functions and actual TSX no-view callback
  before/after patch. Exact clipboard/window/settings router/platform result
  contracts are read from pinned source. Canonical preference/retention functions
  are preserved apart from initializing the new resource-path field.
- V1 typecheck and existing smoke test in an exact disposable root-lock copy.
  Plain npm ci rejected stale peer metadata; `npm ci --legacy-peer-deps
  --ignore-scripts` succeeded without changing that lock. No root package or lock
  was edited.
- Fresh isolated CargoHome **and target directory for each build**, HTTPS SDK Git
  fetch and native ARM64 release build after the final Rust changes.
  No sibling dependency is used. Ignored dev ZIP has a complete closed inventory,
  normalized ZIP member timestamps, exact executable and archive hashes, original
  PNGs and authentic notices. Source/lock/build-directory hashes/provenance and
  extracted executable hash are recorded. Extracted **actual production executable** smoke uses
  invalid input (`localhost`) with zero network/host effects. Success is exercised
  separately through the same adapter's synthetic executable, not claimed for the
  actual live provider/OS environment. The dedicated production HTTP transport is
  additionally tested directly and through the adapter on owned loopback.
- Root HEAD remains source pin; index empty; final visible changes are root README
  plus new v2/. All other V1 bytes and all eight PNGs match immutable source hashes.

`docs/package-report.json`, `docs/provenance.json`, `docs/upstream-assets.json` and
`INDEX.sha256` are review evidence. Logs live in ignored `.work/` and are not required
fixtures. The ZIP is also ignored and can be reproduced by scripts/build-package.sh.

## Not run / integration limits

- Live production geolocation/provider acceptance is **not run**. The original
  fixed HTTP contract is now implemented and tested locally; the former disabled
  transport/HTTPS-policy blocker is removed. Plaintext query/response transport
  has no TLS confidentiality or authenticity. No provider/key substitution was
  introduced. This is source-branch work, not a service/product release.
- Native macOS interface enumeration, actual clipboard/window/settings, native
  Windows GUI/API and real public HTTPS success are **not run**.
- Full Windows GNU check blocked in ring: `x86_64-w64-mingw32-gcc` absent.
  Full MSVC check blocked by missing target core/std and Windows CRT/SDK headers
  (`assert.h`). No toolchain or target was installed. A separate Windows GNU
  metadata typecheck of the native interface module passed; it is not a full
  product build or runtime acceptance.
- No full pinned host build or live renderer acceptance. The host patch is
  unapplied; the executed callback/function fixtures are narrower evidence.
- Source hourly command subtitle/metadata refresh has no generic host equivalent;
  it is not scheduled. Both settings actions share the generic settings surface.
  Native result caching is a bounded synchronous adaptation of source SWR.
- Canceling/discarding a no-view open can lose its volatile copy checkpoint after
  an OS side effect. No automatic replay or persistent clipboard journal exists.
- Missing dependency license texts are explicitly recorded (18 records, some are
  metadata-only or duplicate Python directory records); no license grant is made
  up. Authentic text files from 326 dependency records are preserved separately.

## XERJ

Initial checkout index/search used namespace `ax-1b88ba5347e7`, derived from the
canonical root SHA-256 prefix. State is local to `.git/xerj-state`; no shared/global
namespace or schema was changed. Initial coverage: 15/15 source code files,
21 indexed files, generation 1, exit 3 because binary/junk assets were skipped.
A single final update is logged in `.work/xerj-final.log`. Any schema/429/RSS
failure is final incomplete search coverage, not permission to bypass the
namespace, restart the global server or mutate a schema. Exact-source/Git/rg and
byte comparisons are authoritative for validation.

Final XERJ result: **failed/incomplete**, exit 1 after scanning 581 files (11 MiB).
The frozen dataset rejected new `v2/Cargo.toml` as family `docs` with fields
`end_line/start_line/text`: unsupported dataset/schema evolution. The request to
use a new prefix was deliberately not followed. No final V2 coverage is claimed;
no 429/RSS error was observed in this run. No retry/schema/server change occurred.

Round 2 retains the frozen/incomplete final V2 index result above. The initial
search in this correction used the same `ax-1b88ba5347e7` prefix; it returned V1
passages and did not establish V2 coverage. No new prefix, index/schema change,
server restart or attempted reindex was used in this correction. Full current
files, exact `rg`, Git byte checks and INDEX are the review evidence.
