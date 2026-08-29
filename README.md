# gerrit-sdk-rust-client

An example **consumer** of the generated Gerrit Rust SDK
([`davido/gerrit-sdk-rust`](https://github.com/davido/gerrit-sdk-rust)). It fetches
the SDK **from GitHub** as a cargo git dependency pinned to tag `v3.15.0-SNAPSHOT`
(mirroring the Gerrit version it was generated from) — no crates.io, no local
checkout, and **no hand-written request/response types**.

```
  gerrit6                 gerrit-sdk-rust              gerrit-sdk-rust-client
  (emit the spec)   -->   (generate the SDK)    -->    (this repo: consume it)
  OpenAPI 3.1 JSON        openapi-generator (rust)     cargo git-dep, live calls
```

Everything the demos send and receive is a type from the generated `gerrit_client`
crate, which is produced from Gerrit's statically generated OpenAPI 3.1 document.
This proves the end-to-end story behind Gerrit issue
[40011133](https://issues.gerritcodereview.com/issues/40011133).

## Dependency

```toml
[dependencies]
gerrit_client = { git = "https://github.com/davido/gerrit-sdk-rust.git", package = "gerrit_client", tag = "v3.15.0-SNAPSHOT" }
```

`Cargo.lock` pins the exact published commit. To target a newer Gerrit version,
point at a newer SDK tag.

## Two demos

One binary per REST operation:

| binary | operation | schema |
|---|---|---|
| `get-change-detail` | `GET /changes/{id}` (anonymous) | output (`ChangeInfo`) |
| `post-comment` | `POST …/revisions/current/review` (authenticated) | input (`ReviewInput`) |

### get-change-detail — GET, anonymous, output schema

Reads a change with detail and prints a colored, Web-UI-style summary — status
chip, owner/reviewers/CC, submit requirements, votes, and changed files — using
Gerrit's own palette. All of it comes from a **single GET**, widening the `o`
(ListChangesOption) array rather than making more calls.

```bash
cargo run --bin get-change-detail -- --change 622261
# other change / server:
cargo run --bin get-change-detail -- --url https://gerrit-review.googlesource.com --change 621763
```

Flags:

| flag | meaning | default |
|---|---|---|
| `--url <base>` | Gerrit base URL | `https://gerrit-review.googlesource.com` |
| `--change <id>` | numeric id or `project~branch~Change-Id` | `621763` |
| `--no-color` | disable ANSI color | — |

Color auto-enables on a TTY. `NO_COLOR=1` disables it; `CLICOLOR_FORCE=1` keeps it
through a pipe.

> **Tip:** `cargo run` prints two `Finished`/`Running` lines of its own. Add `-q`,
> or run the built binary directly (`./target/debug/get-change-detail …`).

### post-comment — POST, authenticated, input schema

Posts a change message via `ReviewInput`. Standalone Gerrit REST auth is **HTTP
basic** over the `/a/` path (Settings → HTTP Credentials → *Generate new password*
for `--token`; `--user` is your username).

```bash
# Mutating — run against your OWN / a local dev Gerrit, never a shared server.
cargo run --bin post-comment -- --url http://localhost:8080 \
            --user admin --token <http-password> \
            --change 1 --comment "Reviewed via the generated Rust SDK"
```

## License

Apache 2.0. See [LICENSE.txt](LICENSE.txt).
