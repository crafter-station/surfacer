# Spike: token capture in `auth login` (resolves AE-SPIKE)

```yaml
status: spike resolved
resolves: AE-SPIKE in docs/shaping-auth-emitters.md
reference: crafter-research/sunat-cli/packages/cli/src/plataforma/session.ts
scope: read-only recon; no code changed
```

Today `auth login` persists the browser session NAME, not a token. Verified at
`crates/surfacer-app/src/cli.rs:1078` (`session_name = "surfacer-{site}"`) and
`crates/surfacer-app/src/cli.rs:1102-1105` (writes that string to
`<site_dir>/session-name`). The shim path re-drives that live session via
`fetch_authenticated_html` (`crates/surfacer-app/src/execute.rs:136`). Mode B
(`BrowserBootstrappedToken`) needs the opposite: a captured token written where a
fully headless client reads it after the browser closes. `sunat-cli` already does
this by hand; the three questions below map its proven logic onto surfacer's code.

---

## AE-Q1: network capture, reading a `TokenCapture::RequestQueryParam`

**Answer: use the HAR the probe already records. The HAR `request.url` field keeps
the full query string, so the idCache rides in it verbatim. No probe change needed
for the RequestQueryParam variant.**

Evidence:

- The probe already has a start/stop HAR pair driving `agent-browser network har`:
  `crates/surfacer-probe/src/agent_browser.rs:91-103` (start),
  `crates/surfacer-probe/src/agent_browser.rs:105-126` (stop, writes to
  `paths::har_path` = `<output_dir>/capture.har`, see
  `crates/surfacer-probe/src/paths.rs:7-9`).
- The HAR parser deserializes each request URL raw:
  `crates/surfacer-probe/src/har.rs:30-39` (`HarRequest { method, url, headers,
  post_data }`, `url` is `#[serde(default)] pub url: String`). Query strings are
  NOT stripped.
- Ground truth from the checked-in fixture `fixtures/sunat/capture.har`: request
  URLs retain their query params. Observed entries carry
  `AutenticaMenuInternet.htm?state=rO0ABX...`, `MenuInternet.htm?_=1775876327302`,
  and `oauth2/authen?redirect_uri=...`. So a substring match on `url_contains`
  plus a query-param pull on `param` works directly against `HarRequest.url`.
- The classifier already consumes `HarRequest.url` this way:
  `crates/surfacer-classifier/src/classify.rs:23` (`parse_har`) feeding
  `extract_features(&har, ...)`.

**The reference (sunat-cli):** it does NOT parse a HAR. It calls
`agent-browser --session sunat network requests` and regexes the raw text output:
`.../plataforma/session.ts:56-71` spawns `network requests`, then
`match(/idCache=(ey[A-Za-z0-9._-]+)/)` at `session.ts:64`. That is the "more
direct than HAR" path your question asks about, and `agent-browser network
requests --filter <pattern> --json` exists (confirmed via `agent-browser network
--help`: `requests [--filter <pattern>] [--type ...] [--method ...] --json`).

**Recommendation for AE-V1:** stay on the HAR. surfacer already starts/stops HAR,
already parses it into typed structs, and already reuses that parse in two crates.
Adding a `network requests` text-scrape path would be a second, untyped capture
surface for one variant. Extract the param from `HarRequest.url` with the `url`
crate (already a workspace dep, `Cargo.toml` workspace.dependencies `url = "2"`).

**What to add (surfacer_ir or surfacer_probe):** a small pure function that takes
the parsed `HarLog` and a `&TokenCapture` and returns `Option<String>`:

- `RequestQueryParam { url_contains, param }` -> find first `entry.request.url`
  containing `url_contains`, parse it with `url::Url`, return the `param` query
  value.
- `ResponseHeader { url_contains, header }` -> match `entry.request.url`, then scan
  `entry.response.headers` (`har.rs:46`, already typed as `Vec<HarHeader>`) for
  `header`.
- `Cookie { name }` / `Storage { store, key }` -> NOT in the HAR. See Unknowns.

This function has no natural home yet. `TokenCapture` lives in `surfacer-ir`
(`crates/surfacer-ir/src/auth.rs:130-155`); `HarLog` lives in `surfacer-probe`
(`har.rs`). `surfacer-probe` depends on nothing that would pull in `surfacer-ir`
today, so the cheapest placement is a new function in `surfacer-app` (which already
depends on both crates, `crates/surfacer-app/Cargo.toml` lists `surfacer-ir` and
`surfacer-probe`) or a new `surfacer-probe` fn that takes primitives
(`url_contains`, `param`) instead of the IR enum to avoid a new crate edge.

---

## AE-Q2: persistence, where the captured token goes

**Answer: write `<site_dir>/token.json` next to the existing `session-name`, with
`{ token, expiresAt, capturedAt }`. `expiresAt` comes from a manual base64 decode
of the JWT payload's `exp` claim, no JWT library. The cache struct should live in
`surfacer-app`, not `surfacer-ir`.**

Where:

- `surfacer_ir::site_dir(&home, site)` = `~/.surfacer/sites/<site>/`. Verified at
  `crates/surfacer-ir/src/paths.rs:31-33` (`surfacer_home` -> `.surfacer` at
  `paths.rs:4,8-10`; then `sites/<site_name>`). `auth login` already creates this
  dir and writes `session-name` there (`cli.rs:1102-1105`).
- The demo in the shaping doc already assumes this path:
  `docs/shaping-auth-emitters.md:67` -> `cat ~/.surfacer/sites/sunat/token.json`.
  So the target file is `<site_dir>/token.json`.

What it carries (mirror sunat-cli's shape):

- sunat-cli `CachedToken` = `{ idCache: string; expiresAt: number; capturedAt:
  number }` at `.../plataforma/session.ts:27-32`, written to
  `~/.sunat/plataforma-token.json` (`session.ts:22-23,82-85`).
- surfacer equivalent: `{ token, expiresAt, capturedAt }`. Use the generic name
  `token` (not `idCache`) so the file is not SUNAT-specific; the `TokenUse.name`
  in the IR (`auth.rs:169-176`, e.g. `"IdCache"`) tells a client which header to
  put it in, so the cache file does not need to encode the header name.

Where the struct lives:

- Put the `TokenCache` struct in **surfacer-app**, alongside the `auth_login`
  writer and next to the future headless reader path. Justification by analogy:
  `surfacer-ir` holds the *descriptor* model that ships in `ir.json` (AuthMode,
  TokenTtl, TokenCapture, all `Serialize/Deserialize` in `auth.rs`). The token
  cache is *runtime state*, not descriptor. Runtime state today already lives
  outside `surfacer-ir` and is written ad hoc by the app: `session-name` is a bare
  string file written in `cli.rs`, `fingerprint.json` is built in `cli.rs:1148`,
  and other per-site state files are handled in `cli.rs:1247,1255`. None of those
  have an IR struct. `token.json` is the same category: per-site captured state,
  owned by the app, never part of the committed IR. (Caveat: the *emitted* headless
  client is a separate TS program that also has to read this file (see AE-Q3), so
  the JSON key names are a cross-language contract even though the Rust struct is
  app-local.)

JWT decode for `expiresAt`:

- There is NO JWT or base64 dependency in the repo. Verified:
  `rg base64|jsonwebtoken|jwt` over `*.rs`/`*.toml` returns nothing, and
  `Cargo.lock` has no `base64` crate (not even transitive).
- sunat-cli decodes by hand, no library: `decodeExp` at
  `.../plataforma/session.ts:39-46` splits on `.`, pads to a multiple of 4,
  `Buffer.from(padded, "base64url")`, `JSON.parse`, reads `claims.exp`.
- Rust port options: (a) add the `base64` crate (workspace dep, ~1 line) and
  `serde_json` (already a dep) to decode the middle segment; or (b) hand-roll a
  tiny base64url decoder to avoid any new dep, matching sunat-cli's zero-dep
  stance. Given the middle JWT segment is always base64url without padding,
  option (a) with `base64` + `URL_SAFE_NO_PAD` is the least error-prone. Decision
  is an owner call, not a spike blocker.

---

## AE-Q3: headless read and expiry

**Answer: the emitted ts-cli reads the same `~/.surfacer/sites/<SITE>/token.json`,
computing the path from `$HOME` + a hardcoded `.surfacer/sites/<SITE>` at runtime
(it has no access to the Rust `site_dir` helper). On expiry with
`onExpiry: promptReauth` it prints the `surfacer auth login <site>` hint and exits
nonzero, exactly like sunat-cli's `requireIdCache`.**

The IR fields (verified in `crates/surfacer-ir/src/auth.rs`):

- `TokenTtl { seconds: u64, on_expiry: RenewalStrategy }` at `auth.rs:179-187`.
- `RenewalStrategy::{ Reacquire, PromptReauth }` at `auth.rs:189-198`. Doc comment
  is explicit: `Reacquire` is valid for OAuth2 (headless) but NOT for
  BrowserBootstrappedToken, which needs a human; `PromptReauth` is "the only honest
  option for a browser-bootstrapped token" (`auth.rs:190-197`).
- `TokenUse { location, name, value_prefix }` at `auth.rs:167-176` tells the client
  where to attach it: `location: Header` + `name: "IdCache"` + no prefix for SUNAT.
- `BrowserBootstrappedTokenAuth { acquire, use_, ttl }` at `auth.rs:100-110`. The
  `#[serde(rename = "use")]` on `use_` (`auth.rs:106`) means the IR JSON key is
  `use`, confirmed by the test at `auth.rs:365-392`.

How the emitted client finds the path:

- The emitted ts-cli today is fully self-contained and does a bare `fetch(url)`
  with NO auth: `crates/surfacer-emit-cli/src/ts_cli.rs:202`. It inlines only
  `SITE`, `DISPLAY`, `BASE` (`ts_cli.rs:80-82`); it has no config-dir notion.
- No emitter consumes auth yet: `rg resolve_auth|AuthMode|TokenCapture` over
  `crates/surfacer-emit-cli/src/` returns nothing; the emitters that mention
  `auth` only set `auth: None` in tests (`help.rs:234,245,271`).
- The Rust `site_dir` helper (`surfacer-ir/src/paths.rs:31`) is NOT available to
  the emitted TS program (it ships as a standalone binary via `scriptc`, no
  surfacer install required, per the header comment at `ts_cli.rs:54-63`). So the
  emitter must bake the path into the generated TS: read `process.env.HOME` and
  join `.surfacer/sites/<SITE>/token.json`. `SITE` is already a compile-time
  literal in the template (`ts_cli.rs:80`), so the emitter can hardcode the exact
  path string. This is a NEW cross-language contract: the Rust writer
  (`.surfacer/sites/<site>/token.json`) and the TS reader must agree on the path
  and the JSON keys.

Expiry behavior to mirror:

- sunat-cli `requireIdCache` (`.../plataforma/session.ts:106-114`): if no cache OR
  `expiresAt <= now + 60`, throw with the message "No fresh Nueva Plataforma
  token. Run a command that opens the browser to capture one (it lasts 1 hour)."
- `hasFreshToken` (`session.ts:88-92`) uses the same 60s skew.
- surfacer emitted client, for `onExpiry: promptReauth`: on missing/expired token,
  print `needs: surfacer auth login <site>` (the same hint string the help emitter
  will render per `docs/shaping-auth-emitters.md:71`) and exit nonzero. For
  `onExpiry: reacquire` (OAuth2 only, never Mode B), the client   endpoint instead, out of scope for AE-V1.

---

## Steps to implement AE-V1

1. **Add a token-cache type (surfacer-app).** In `surfacer-app`, define
   `TokenCache { token: String, expires_at: u64, captured_at: u64 }` with
   `Serialize/Deserialize` (serde already a dep). Serialize keys as camelCase
   (`expiresAt`, `capturedAt`) to match the sunat-cli shape and the TS reader in
   AE-V4. Do NOT put it in `surfacer-ir` (it is runtime state, not descriptor;
   analogy: `session-name`, `fingerprint.json`).

2. **Add a capture-extraction function.** A pure fn over the parsed `HarLog`.
   Cheapest placement without a new crate edge: `surfacer-probe`, taking primitives
   (`url_contains: &str`, `param: &str`) so it does not need to import
   `surfacer-ir`. Body: iterate `har.log.entries`, find first
   `entry.request.url` (`har.rs:34`) containing `url_contains`, `url::Url::parse`
   it, return the `param` query value. Also handle `ResponseHeader` by scanning
   `entry.response.headers` (`har.rs:46`).

3. **Add a JWT `exp` decoder.** Either add `base64` to workspace deps and decode
   the JWT middle segment with `URL_SAFE_NO_PAD` + `serde_json` to read `exp`, or
   hand-roll base64url like sunat-cli's `decodeExp`
   (`.../plataforma/session.ts:39-46`). Owner decides dep vs hand-roll.

4. **Rewrite `auth_login` to capture (cli.rs).** After `wait_for_enter`
   (`cli.rs:1100`), branch on the site's resolved auth. If
   `BrowserBootstrappedToken`, read its `acquire.capture` (`auth.rs:120`):
   - Start/stop or reuse the HAR. Today `auth login` builds a `ProbeSession` via
     `connect_session` (`cli.rs:1094`) but never calls `start_har_capture`. Add a
     `start_har_capture` right after connect and a `stop_har_capture` after ENTER
     (both exist: `agent_browser.rs:91,105`) so the login navigation is recorded.
   - Parse the HAR (`surfacer_probe::har::parse_har`, `har.rs:78`), run the
     extraction fn from step 2 with `url_contains`/`param` from the IR.
   - Decode `exp` (step 3) into `expires_at`, build `TokenCache`, write it to
     `site_dir.join("token.json")` (site_dir already computed at `cli.rs:1102`).
   - Keep writing `session-name` too (the shim still uses it); token.json is
     additive.

5. **Handle the "no token in log" case honestly.** Mirror
   `captureIdCacheFromSession` (`session.ts:64-70`): if the extraction fn returns
   `None`, error with a message telling the human to navigate into the authed form
   first (the token only appears once the form iframe loads).

6. **Demo (from the shaping doc, `shaping-auth-emitters.md:67`):**
   `surfacer auth login sunat` then `cat ~/.surfacer/sites/sunat/token.json` shows
   a JWT `token` with a real `expiresAt`.

The biggest single step is #4: teaching `auth login` to record the HAR during the
login navigation and pull the param out of it. Everything else (cache struct, exp
decode, error path) is small and has a proven reference in sunat-cli.

---

## Unknowns that remain

1. **The checked-in fixture cannot prove the idCache path end to end.**
   `fixtures/sunat/capture.har` has ZERO `idCache=` and ZERO `servletAcceso`
   occurrences (verified by grep). It was captured during unauthenticated recon, so
   the servletAcceso navigation that carries the idCache is not in it. The claim
   "the idCache rides in `HarRequest.url`" is INFERRED from (a) sunat-cli capturing
   it from the same `network requests`/CDP stream and (b) the fixture proving other
   query params survive in `HarRequest.url`. It is not OBSERVED against a real
   idCache in a surfacer HAR. Resolve by capturing one authed HAR
   (`surfacer auth login sunat`, navigate into an F616 form, `network har stop`) and
   confirming `idCache=ey...` appears in an entry's `request.url`. This is the one
   live check AE-V1 should run before trusting the HAR path.

2. **Cookie and Storage captures are not in the HAR.**
   `TokenCapture::Cookie` and `TokenCapture::Storage` (`auth.rs:149-155`) cannot be
   read from `HarLog` (the HAR parser models request/response headers and bodies,
   not `document.cookie` or `localStorage`). Those variants would need
   `agent-browser eval` (cookie: `document.cookie`; storage:
   `localStorage.getItem(key)` / `sessionStorage.getItem(key)`), a different capture
   surface. Out of scope for AE-V1 (SUNAT is RequestQueryParam), but the extraction
   fn should return a clear "unsupported capture variant" error for them rather than
   silently failing.

3. **`network requests --content` window.** The HAR records only what happened
   between `har start` and `har stop`. `auth login` today never starts a HAR
   (`cli.rs:1094-1100` connects and waits but does not call `start_har_capture`), so
   step #4 must add it BEFORE the human navigates. If the idCache navigation happens
   before `har start`, it is lost. Confirm the ordering during AE-V1: start HAR at
   connect, keep it running across the whole login, stop after ENTER.

4. **base64 dep vs hand-roll** is an owner decision, not a spike blocker. Noted in
   AE-Q2 / step #3.

## Observed 2026-08-09 (was inferred)

The AE-Q1 inference is now confirmed against a live authenticated SUNAT session. The idCache rides the URL of GET requests, exactly where the HAR stores it in `request.url`:

```
GET https://e-menu.sunat.gob.pe/cl-ti-itmenu2/MenuInternetPlataforma.htm?...&idCache=ey...
GET https://e-plataformaunica.sunat.gob.pe/servletAcceso?...&idCache=ey...
```

So `TokenCapture::RequestQueryParam { url_contains: "servletAcceso", param: "idCache" }` extracts it. The one caveat the spike flagged holds: `auth login` does not start HAR capture during the login navigation today, so AE-V1's core step is wiring `start_har_capture` in before the navigation and `stop_har_capture` after ENTER, then parsing the saved HAR. The capture-into-HAR path itself is proven.
