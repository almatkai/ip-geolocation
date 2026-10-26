# Original provider contract and transport evidence

Official primary documentation: https://ip-api.com/docs/api:json, observed
2026-10-26 by the parent reviewer without a product API request. The free base
is `http://ip-api.com/json/{query}`; query accepts IPv4, IPv6, a domain, or an
empty self-lookup. `lang` and `fields` are GET parameters; no API key is required.
The free endpoint has no SSL support. The separately paid service's TLS contract
is not the original source contract. The free endpoint is documented for
non-commercial use; this is provider evidence, not a license grant or a legal
conclusion. This work prepares a source branch, not a service/product release.

V1 `src/utils/constants.ts` fixes `http://ip-api.com/json/` and fields `585727`.
The port now uses that original HTTP service in production; the earlier generic
HTTPS-only restriction and disabled-geolocation claim have been removed. A
separate geo client does not weaken public-IP TLS. Plaintext HTTP exposes the
lookup query and location response to network observers and permits response
modification; it provides no TLS confidentiality or provider authenticity.

Production constructs the complete URL with one component-encoded query path
and an allowlisted source language. It validates the parsed URL against the
constructed URL, `http`, exact `ip-api.com`, default port 80, no credentials and
no fragment before sending. Dot-only inputs whose path would be normalized by
the URL parser fail locally. There is no environment/preference endpoint override,
proxy, redirect, auth, cookie store or extra domain probe. Only the fixed service
host is resolved by the HTTP client; lookup domains are resolved by the provider.
A failed public IPv4 lookup may still pass an empty query, as in the source.

The dedicated reqwest client has 5-second connect/request limits. The entire
geolocation operation, including serialization wait and streaming/parse, has a
10-second limit. The streamed body is capped at 65,536 bytes; JSON is restricted
to depth 16, 256 nodes, 8,192-byte strings and 128-byte keys. Success requires the
source text field types; source scalar formatting quirks and finite numbers are
retained. Source `status:fail` stays a fail response with empty geolocation rows
(and the source address rows for My IP). Bodies/messages/URLs are never reflected
in error messages. A failed lookup cannot refresh the successful result cache.

The documented limit is 45 requests/minute per public IP. `X-Rl` is remaining
capacity and `X-Ttl` is seconds to reset; 429 indicates throttling. On `X-Rl: 0`
with a valid `X-Ttl`, or 429 with `Retry-After`/`X-Ttl`, production records a
monotonic deadline before reading the body. Numeric Retry-After and HTTP-date
forms are supported; date conversion uses a bounded wall-clock delta and then a
monotonic deadline. The greater valid delay wins, capped at one hour. Duplicate,
negative, overlong or malformed delay headers are ignored. A 429 without a usable
delay reports a manual-retry error without inventing a permanent ban. A known
cooldown prevents any subsequent geo send until expiry. No timer, automatic
replay, polling or product sleep is scheduled. Dropping a future drops the HTTP
request and serialization guard; an already learned cooldown remains in memory.
Cooldown is per process and does not claim coordination across other clients.

The locked `public-ip@7.0.1` `onlyHttps:true` contract remains independent:
`https://icanhazip.com/` then `https://api.ipify.org/` for IPv4, or
`https://api6.ipify.org/` for IPv6. Each attempt has 5000 ms timeout, zero retries
and the requested address family. Proxy use and redirects are disabled. WebPKI
certificate verification remains enabled. Bodies are capped at 1024 bytes and
validated against the requested IP family. No auth is sent.

QA uses the actual production geo transport/client factory/decoder. A private
`#[cfg(test)]` seam routes the already validated fixed-origin request to owned
loopback, preserving the original Host and encoded path. Production compilation
has no routing field or override. Tests call `Production::geolocation` directly
and through the real adapter; synthetic interfaces/public-IP producers are used
only where My IP needs those separate boundaries. Existing real SDK JSONL pipe
checks exercise all three commands and authoritative cancel/stale-action fences.
No live ip-api, public-IP, lookup DNS or native interface call was performed.
