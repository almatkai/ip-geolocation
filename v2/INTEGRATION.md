# Integration and parent review

Canonical ID: `com.tezbar.native.ip-geolocation`; exact replacements confirmed by
own manifest/catalog: `com.tezbar.ip-geolocation`, `raycast.ip-geolocation`.
Source `extension.json` is schema 3, Rust process, protocol 1, with empty artifacts
and `refreshMs:null`. Both native platform verification entries are `not-run`.
It is development metadata, not an installable public catalog release.

Read-only host pin: `ea100cee3f69711de12485ba061950c5fc65258f`.
All host fixtures came from `git show <pin>:<path>`, never current app files.
SDK: HTTPS Git revision `e955911f0bf5b558f9d3eb5183df0adc95c22af2`, crate 0.1.1,
MSRV 1.85. Cargo.lock has the exact Git revision; no local SDK/app dependency.
Frozen SDK API/PROTOCOL/HOST_INTEGRATION files live in `qa/sdk/`.

The only used host methods are:

| Method | Params | Accepted result |
| --- | --- | --- |
| `host.clipboard.write` | `{text: <exact bytes as UTF-8 string>}` | Closed object `ok:true`, optional string `message` |
| `host.window.visibility` | `{visible:false}` or recovery `{visible:true}` | **JSON null**, as returned at this host pin |
| `host.settings.open` | `{surface:"settings-extensions"}` | Closed object `ok:true`, optional string `message` |

Null, booleans, extra keys, contradictory errors and non-string messages are
rejected for object ACK methods. Read-text contracts are tested separately as
strings; they are not invoked or granted by this port. Visibility is never passed
through an object-ACK validator. PasteText is unused; no duplicate paste effects
are introduced. Host RPC errors are replaced by fixed safe messages.

SDK stages/commits successful current state, but network, host effects and stores
do not roll back. Adapter action authority is separate: bounded per-session
records, monotonic revisions and fresh UUID action IDs, consumed before awaiting.
Starting a lookup clears previous actions. A-B-A never restores old IDs. Refresh
is unchanged unless there is an interrupted effect to recover. Failed lookups
expose manual Retry Lookup. Effects never replay automatically.

Unknown clipboard outcome offers Retry Exact Copy, which reuses captured bytes
without another lookup. Once clipboard success is acknowledged, an interrupted
close offers close-only recovery. Session disposal clears volatile authority;
no clipboard journal is persisted. **Cancellation of an automatic no-view open
may lose its volatile checkpoint after the clipboard side effect.** Its outcome
is unknown, with no automatic retry. An explicit new launch is a new command.
The SDK failed-open guard and adapter drop guard are exercised by 140 canceled
opens, including 70 canceled clipboard writes with late acknowledgements.

No detached extension task/thread, timer, background poll or helper exists.
Each open registers one SDK-owned, non-polling drop guard. It captures no SDK
context/scope, creates no ownership cycle and removes only its nonce-owned
authority record on scope/operation cancellation, including the post-return
commit race. It requests no background ownership and ends with its session.
Reqwest/Tokio's ordinary bounded request machinery performs fixed-service DNS;
no custom resolver worker is started. Dropped network requests and reverse
waiters are canceled by their futures/SDK. Filesystem I/O is bounded, serialized
and atomic beneath SDK `data_dir`; symlink components, non-objects, wrong schemas,
oversized stores and malformed cache records fail explicitly. Unix writes use
private directory/temp-file modes and O_NOFOLLOW on the store. The SDK-granted
private root is trusted; this is not a defense against a hostile process racing
renames throughout an entire user-owned directory tree.

## Unapplied generic host prerequisites

`docs/host-prerequisites.patch` is generated from the exact host fixtures:

1. Registry asset lookup uses verified active-version resources while retaining
   canonical package paths for preferences/uninstall. Existing preference/retention
   code stays byte-identical apart from initializing the new resource-only field.
2. Legacy aliases route through exact verified replacement ownership and covered
   Rust commands/platforms. Ambiguous owners, lookalikes, uncovered commands and
   unverified packages cannot authorize execution. Verification stays in the host.
3. A no-view action result disposes the session and calls onBack; this covers
   successful list copy and exact-copy recovery. The extension does not patch
   visibility ACKs.

Actual extracted Rust functions and renderer callbacks have executable before/
after fixtures. Unused Detail/Form/dropdown changes are absent. The host patch
has not been applied or validated with a full app build/live desktop. Parent must
review it against the same pin before integration; no app worktree was changed.

## Required release work

The source branch preserves the original free-provider HTTP contract (see
`docs/provider-evidence.md`); no provider approval gate remains. Review/apply only
the necessary host patch. Run clean native macOS/Windows OS interface,
clipboard, settings, cancellation and public TLS success tests. Build/verify both
release targets and authenticate target archives before creating real artifact
URLs/catalog pins. Publication, push, PR, installation and deletion were not
performed. The source branch and all edits remain available for parent review.
