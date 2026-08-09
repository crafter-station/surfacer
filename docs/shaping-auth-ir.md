---
type: shaping
project: surfacer
feature: auth
created: 2026-08-09
status: shaping-active
depends_on: surfacer-ir SiteDescriptor, surfacer-ir HttpSurface, surfacer-emit-cli (openapi, ts-cli, mcp, just-bash, shim, help)
---

# Auth in the IR: Shaping

> The IR describes a service surface and six emitters turn it into clients.
> Today those clients cannot authenticate: the IR models endpoints, params,
> content types, and accessibility trees, but nothing about how a caller
> proves who it is. Every emitted client can only reach the unauthenticated
> subset of a service. This doc adds auth to the IR so the emitters can carry
> it out.

## Problem

`surfacer-app` already knows how to authenticate at runtime. `execute.rs`
has `fetch_authenticated_html(url, session_name)`, which drives a named
`agent-browser` session, and the CLI exposes `auth login | status | logout`
per site. But that knowledge lives entirely in the app and is keyed by an
opaque `session-name` file on disk. The IR is blind to it.

Two consequences:

1. The emitters (`ts-cli`, `mcp`, `openapi`, `just-bash`, `shim`, `help`)
   receive a `SiteDescriptor` with no auth field, so every generated client
   issues bare `fetch(url)` calls. Against any login-walled operation they
   get a redirect or a 401. The `ts-cli` and `mcp` emitters literally write
   `await fetch(url)` with no header path.
2. There is no vocabulary to say *how* an operation authenticates. The one
   runtime mode that exists (`agent-browser` session) is hardcoded and cannot
   distinguish an OAuth2 client-credentials token from a browser-captured
   short-lived JWT.

The forcing case is SUNAT, the Peruvian tax portal, already the fixture
across the IR tests. Recon there found **two different auth modes on the
same host**, which kills any design that models auth once per site.

### Mode A: OAuth2 headless (the ordinary case)

A registered client fetches a token by `password` grant:

```
POST api-seguridad.sunat.gob.pe/v1/clientessol/{clientId}/oauth2/token/
grant_type=password&username=...&password=...&scope=...
```

Token lives ~1h, refreshes itself, and covers SIRE, GRE, CPE, buzon. This is
the shape Stainless, Speakeasy, and Fern already generate. Nothing novel.

### Mode B: browser-bootstrapped token (the differentiator)

The F616 monthly declaration form sends a header `IdCache` whose value is a
JWT. That JWT is minted by the portal's **internal** client during the
browser login (grant `authorization_code`, `aud=e-plataformaunica`, life
3600s). A self-registered client can never obtain that `aud`. But the token
captured from the browser network log works headless for its full hour,
verified with the browser fully closed.

The acquisition shape:

```
open browser once -> user logs in
  -> capture idCache= query param from the servletAcceso request
  -> close browser
  -> replay headless with header IdCache: <jwt> until TTL expires
```

No SDK generator on the market models "acquire this token via a browser once
per hour, then call headless." Roughly half of LATAM government portals work
exactly this way. Modeling it is the reason this feature exists.

## Outcome

After this slice, a `SiteDescriptor` can carry auth at the surface level and
optionally override it per operation. Given the SUNAT IR:

- The `openapi` emitter writes a `components.securitySchemes` block for Mode A
  (`oauth2`) and marks each operation's `security`. For Mode B it writes an
  `x-surfacer-auth` extension and omits it from `security`, because OpenAPI
  has no scheme for a browser-bootstrapped token (see D3).
- The `ts-cli` and `mcp` emitters generate an `acquireToken()` path and attach
  the right header before `fetch`, or, for Mode B, refuse to run headless
  until a captured token is present and print the bootstrap instruction.
- The `help` emitter shows which commands need auth and how to acquire it.
- `surfacer auth login <site>` (already present) becomes the Mode B bootstrap
  step, now described by the IR instead of hardcoded.

Nothing about the emitted **unauthenticated** clients changes: an IR with no
auth block emits exactly what it emits today.

## Requirements (R)

| ID | Requirement | Status |
|----|-------------|--------|
| AU-R0 | The IR can describe an auth mode, defaulting to none so existing descriptors deserialize unchanged | Must-have |
| AU-R1 | Auth can be declared once for the whole HTTP surface and overridden per operation | Must-have |
| AU-R2 | The model separates **acquisition** (how a token/credential is obtained) from **use** (how it is attached to a request) | Must-have |
| AU-R3 | OAuth2 client-credentials / password grant is modeled with token URL, grant, and scopes | Must-have |
| AU-R4 | Browser-bootstrapped token is modeled: a one-time browser acquisition that yields a value replayed headless with a TTL | Must-have |
| AU-R5 | Static API key and static bearer token are modeled for the trivial header cases | Must-have |
| AU-R6 | TTL and re-acquisition are expressed in the IR, so a client knows a token expires and how to renew it | Must-have |
| AU-R7 | Where a mode maps to OpenAPI `securitySchemes`, the openapi emitter emits it; where it does not (Mode B), it degrades to an `x-surfacer-auth` extension and says so | Must-have |
| AU-R8 | Secrets (passwords, client secrets, captured tokens) are never written into the IR; the IR names *where* a value comes from, not the value | Must-have |
| AU-R9 | `surfacer lint` rejects an auth block that references a source it cannot resolve (e.g. an operation override with no surface default and no self-contained acquisition) | Nice-to-have |
| AU-R10 | The existing `auth login/status/logout` runtime is reused as the Mode B bootstrap, not replaced | Must-have |

## IR Schema Extension

Style follows the rest of `surfacer-ir`: `#[serde(rename_all = "camelCase")]`,
internally-tagged enums, `#[serde(default)]` on every added field so older
descriptors still parse, secrets referenced by name not value.

New file: `crates/surfacer-ir/src/auth.rs`, re-exported from `lib.rs`.

### Where auth attaches

`HttpSurface` gets a surface-level default; `OperationDescriptor` gets an
optional override. Both default to `None`.

```rust
// http.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpSurface {
    pub endpoints: Vec<HttpEndpoint>,
    /// Auth applied to every operation on this surface unless the operation
    /// overrides it. `None` means the surface was reached unauthenticated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthMode>,
}
```

```rust
// descriptor.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDescriptor {
    pub command_path: Vec<String>,
    pub summary: String,
    pub description: String,
    pub operation_kind: OperationKind,
    pub transport: OperationTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor: Option<Extractor>,
    /// Overrides the surface-level auth for this operation. `None` means
    /// "inherit the surface default". To express "this one operation is
    /// public on an otherwise-authenticated surface", set
    /// `Some(AuthMode::None)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthMode>,
}
```

Resolution rule (implemented as a helper, not a struct field):

```rust
// auth.rs
/// The auth an operation actually uses: its own override, else the surface
/// default, else None.
pub fn resolve_auth<'a>(
    op: &'a OperationDescriptor,
    surface: Option<&'a HttpSurface>,
) -> Option<&'a AuthMode> {
    op.auth
        .as_ref()
        .or_else(|| surface.and_then(|s| s.auth.as_ref()))
}
```

The `Option<AuthMode>` override with an explicit `AuthMode::None` variant is
what makes SUNAT expressible: the surface default is Mode A (OAuth2), and the
F616 operation overrides to Mode B, while a public `ficha-ruc` lookup
overrides to `AuthMode::None`.

### The AuthMode enum

```rust
// auth.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AuthMode {
    /// No authentication. Explicit so an operation can opt out of a
    /// surface-level default.
    None,

    /// A static secret sent on every request as a header. Covers API keys and
    /// long-lived bearer tokens that the caller already holds. The value is
    /// never in the IR; `secret_ref` names where the client reads it.
    ApiKey(ApiKeyAuth),

    /// OAuth2 where the client acquires a token itself, headless, with no
    /// browser. Password and client-credentials grants. The ordinary case.
    OAuth2(OAuth2Auth),

    /// A token that only the target's own browser session can mint, captured
    /// once and replayed headless until it expires. Acquisition and use are
    /// modeled separately because they happen in different processes at
    /// different times. This is the mode no SDK generator models.
    BrowserBootstrappedToken(BrowserBootstrappedTokenAuth),
}
```

### ApiKey and static bearer

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyAuth {
    /// Where the value is attached. Header is the common case; query covers
    /// legacy `?apikey=` endpoints observed during recon.
    pub location: CredentialLocation,
    /// The header name or query param name, e.g. "Authorization",
    /// "X-Api-Key", "IdCache".
    pub name: String,
    /// A prefix prepended to the value, e.g. "Bearer ". Empty for a bare key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_prefix: Option<String>,
    /// Names the environment variable or credential file the client reads the
    /// secret from. Never the secret itself (AU-R8).
    pub secret_ref: SecretRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialLocation {
    Header,
    Query,
}

/// Names a secret's source without carrying its value. The client resolves it
/// at runtime; the IR stays safe to commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "from")]
pub enum SecretRef {
    /// Read from an environment variable, e.g. "SUNAT_API_KEY".
    Env { var: String },
    /// Read from a file under the site's config dir, e.g. ".sunat/token".
    File { path: String },
    /// Produced at runtime by an acquisition step (OAuth2 token, captured
    /// browser token). Not read from anywhere static.
    Acquired,
}
```

### OAuth2

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Auth {
    pub grant: OAuth2Grant,
    /// The token endpoint observed during recon.
    pub token_url: String,
    /// Scopes seen on the token request, if any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// How the resulting access token is attached to subsequent requests.
    /// Defaults to `Authorization: Bearer <token>` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_use: Option<TokenUse>,
    /// Where the grant's own inputs (username/password, client id/secret)
    /// come from. The IR names the sources; the client resolves them.
    pub credentials: SecretRef,
    /// Token lifetime and how to renew. Present when recon observed an
    /// `expires_in` or an equivalent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<TokenTtl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuth2Grant {
    Password,
    ClientCredentials,
    /// Only meaningful inside BrowserBootstrappedToken acquisition; kept here
    /// for completeness of the observed grants.
    AuthorizationCode,
}
```

### Browser-bootstrapped token: acquisition vs use split

This is the heart of the design. Acquisition happens once, in a browser, by
a human (or `surfacer auth login`). Use happens many times, headless, by the
emitted client. They are two different `struct`s wired together by the header
name and the TTL.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBootstrappedTokenAuth {
    /// How the token is obtained. Runs in a browser, once per TTL window.
    pub acquire: BrowserAcquisition,
    /// How the captured token is attached to headless requests afterward.
    pub use_: TokenUse,
    /// How long the captured token stays valid and when to re-acquire.
    pub ttl: TokenTtl,
}

/// The one-time, browser-side half. Describes what to open and where the
/// token surfaces so the acquisition step can capture it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAcquisition {
    /// URL to open for the human to log in. The portal's own login flow.
    pub login_url: String,
    /// How the token becomes observable once login completes.
    pub capture: TokenCapture,
    /// The named agent-browser session this reuses, tying acquisition to the
    /// existing `surfacer auth login <site>` runtime (AU-R10). `None` means a
    /// fresh session named after the site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<String>,
}

/// Where the token appears during the browser login, so recon and the
/// acquisition step know what to watch for.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "source")]
pub enum TokenCapture {
    /// A query parameter on a specific request, e.g. `idCache=` on the
    /// servletAcceso request. This is the SUNAT F616 case.
    RequestQueryParam {
        /// A substring the request URL must contain to be the right one.
        url_contains: String,
        /// The query parameter carrying the token, e.g. "idCache".
        param: String,
    },
    /// A response header on a matched request.
    ResponseHeader {
        url_contains: String,
        header: String,
    },
    /// A cookie set during login.
    Cookie { name: String },
    /// A value read from browser storage after login, e.g.
    /// localStorage["access_token"].
    Storage {
        store: BrowserStore,
        key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowserStore {
    Local,
    Session,
}

/// How an acquired token is attached to a headless request. Shared by OAuth2
/// and BrowserBootstrappedToken because "use" is the same problem for both:
/// a header (or query param), a name, an optional prefix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUse {
    pub location: CredentialLocation,
    /// Header or query param name, e.g. "Authorization", "IdCache".
    pub name: String,
    /// e.g. "Bearer ". `None` for a bare token like SUNAT's `IdCache`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_prefix: Option<String>,
}

/// Token lifetime and renewal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenTtl {
    /// Observed lifetime in seconds. SUNAT's IdCache and OAuth2 token are both
    /// 3600.
    pub seconds: u64,
    /// What the client does when the token is past its TTL.
    pub on_expiry: RenewalStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenewalStrategy {
    /// Re-run the acquisition automatically. Valid for OAuth2 (headless) but
    /// NOT for BrowserBootstrappedToken, which needs a human.
    Reacquire,
    /// Stop and tell the human to re-run `surfacer auth login <site>`. The
    /// only honest option for a browser-bootstrapped token.
    PromptReauth,
}
```

> Note the trailing underscore on `use_` and `Self::use_`: `use` is a Rust
> keyword. With `rename_all = "camelCase"` and no field rename the JSON key
> becomes `use`. Add `#[serde(rename = "use")]` if the raw JSON key must be
> exactly `use`; otherwise serde emits `use` from `use_` only after the
> keyword strip, so verify the serialized key in a roundtrip test and add the
> explicit rename if it does not land on `use`.

### JSON example: the full SUNAT surface

```json
{
  "http": {
    "auth": {
      "kind": "oAuth2",
      "grant": "password",
      "tokenUrl": "https://api-seguridad.sunat.gob.pe/v1/clientessol/{clientId}/oauth2/token/",
      "scopes": ["sire", "gre", "cpe"],
      "credentials": { "from": "env", "var": "SUNAT_SOL_CREDENTIALS" },
      "ttl": { "seconds": 3600, "onExpiry": "reacquire" }
    },
    "endpoints": [ "..." ]
  },
  "operations": [
    {
      "commandPath": ["f616", "declarar"],
      "summary": "Presenta la declaracion mensual F616",
      "description": "...",
      "operationKind": "write",
      "transport": { "kind": "http", "endpointIndex": 4 },
      "auth": {
        "kind": "browserBootstrappedToken",
        "acquire": {
          "loginUrl": "https://e-menu.sunat.gob.pe/cl-ti-itmenu/AutenticaMenuInternet.htm",
          "capture": {
            "source": "requestQueryParam",
            "urlContains": "servletAcceso",
            "param": "idCache"
          },
          "sessionRef": "sunat"
        },
        "use": { "location": "header", "name": "IdCache" },
        "ttl": { "seconds": 3600, "onExpiry": "promptReauth" }
      }
    },
    {
      "commandPath": ["ficha-ruc"],
      "summary": "Consulta ficha RUC",
      "description": "...",
      "operationKind": "read",
      "transport": { "kind": "http", "endpointIndex": 1 },
      "auth": { "kind": "none" }
    }
  ]
}
```

This one IR expresses all three states on the same host: OAuth2 default,
Mode B override for F616, public override for `ficha-ruc`. That is the exact
shape no single-mode design can carry, and it is the justification for
per-surface-default-plus-per-operation-override (AU-R1).

## How each emitter consumes auth

Every emitter already resolves the endpoint per operation. Each now also
calls `resolve_auth(op, descriptor.http.as_ref())` and branches.

### openapi

Maps to `components.securitySchemes` where a scheme exists, degrades where it
does not.

- `AuthMode::None` -> operation has no `security`, or `security: []` to mark
  it explicitly public against an authenticated default.
- `AuthMode::ApiKey` -> `type: apiKey` scheme (`in: header|query`, `name`).
- `AuthMode::OAuth2` -> `type: oauth2` scheme with a `flows` block
  (`password` or `clientCredentials`, `tokenUrl`, `scopes`).
- `AuthMode::BrowserBootstrappedToken` -> **no OpenAPI equivalent**. OpenAPI
  3.1 has no security scheme for "acquire via browser once, replay headless."
  The emitter writes the operation with an `x-surfacer-auth` extension
  carrying the full `BrowserBootstrappedTokenAuth` JSON, omits it from
  `security`, and adds a one-line `description` note that the operation needs
  `surfacer auth login <site>`. This is stated openly rather than faked, in
  the same spirit as the existing "response schemas are absent because recon
  captured bodies, not contracts" comment in `openapi.rs`.

Add a `components.securitySchemes` map built from the distinct schemes across
the surface, and a `security` array per operation referencing them by name.

### ts-cli and mcp

These carry the descriptor inline and issue `fetch` directly, so they gain a
real auth path. Today both write `const res = await fetch(url)`. After:

- Emit an `authForOperation(name)` lookup returning a small runtime record
  (`{ location, name, valuePrefix, acquire }`) built from the resolved
  `AuthMode`, alongside the existing `OPERATIONS` array.
- Emit a `resolveToken(auth)` helper:
  - `apiKey` -> read `secret_ref` (env var or file) and return the value.
  - `oAuth2` -> POST the token URL with the grant, cache the token with its
    TTL, refresh on `reacquire`.
  - `browserBootstrappedToken` -> read the captured token from the site
    config dir (written by `surfacer auth login`). If absent or past TTL with
    `onExpiry: promptReauth`, print
    `run \`surfacer auth login <site>\` then retry` and exit non-zero. Never
    attempt the browser flow inside a headless client.
- Before `fetch`, attach the token per `TokenUse` (`headers[name] = prefix +
  token`, or append a query param).

The existing `ALLOWED_KINDS` write-gate stays; auth is orthogonal to it. A
Mode B write is still blocked by kind until a human widens it.

### just-bash

The bash target hands operations to one runtime as shell functions. It gains
the same three cases expressed in shell: `curl -H "$name: $prefix$token"`,
with the token sourced from an env var (`apiKey`), a `curl` token POST piped
through `jq` (`oAuth2`), or a file written by `auth login` (Mode B). Mode B
functions print the `auth login` hint and return non-zero when the file is
missing or stale.

### shim

The shim shells out to `surfacer exec`, which already owns
`fetch_authenticated_html` and the session runtime. So the shim needs **no
auth codegen**: `surfacer exec` resolves auth from the installed IR at
runtime. This is the cleanest consumer and the reason `auth login/status/
logout` stays in the app (AU-R10) rather than moving into generated clients.

### help

Adds an `auth` column or a per-command marker: a lock glyph and a short hint
(`needs: surfacer auth login sunat`) for any command whose resolved auth is
not `None`. Public commands render as they do today.

## TTL and refresh

`TokenTtl { seconds, on_expiry }` is the whole model. `seconds` is the
observed lifetime (3600 for both SUNAT modes). `on_expiry` is a two-value
enum, and the split is load-bearing:

- `Reacquire` is only valid for `OAuth2` and `ApiKey`-with-`Acquired`-source,
  where renewal is a headless call the client can make alone.
- `PromptReauth` is the only honest value for `BrowserBootstrappedToken`,
  because re-minting the token requires a human at a browser. A client that
  tried to `Reacquire` a Mode B token headless would spin forever. `lint`
  should reject `BrowserBootstrappedToken` with `on_expiry: Reacquire`
  (AU-R9).

The client caches `(token, acquired_at)` and treats the token as expired at
`acquired_at + seconds`. No refresh-token modeling in this slice (see below).

## What NOT to model yet (scope creep)

Explicitly out of this slice:

- **Refresh tokens.** OAuth2 `refresh_token` grant is a separate acquisition;
  for now `Reacquire` re-runs the original grant. Add a `RefreshToken` variant
  only when a real target needs it.
- **mTLS / client certificates.** Some government portals require them; no
  current fixture does, so no cert model.
- **Multi-step / MFA login flows** beyond "open URL, log in, capture token."
  If a target needs OTP entry mid-flow, that is a richer `BrowserAcquisition`
  and waits for a target that forces it.
- **Auth per parameter or per response** (e.g. a token that scopes which
  fields return). Out of scope; auth attaches at operation/surface only.
- **Secret storage.** The IR names sources (`SecretRef`); it does not manage a
  keychain, vault, or credential rotation. Where the secret actually lives is
  the client's problem.
- **Automatic recon of auth.** This doc is the IR shape only. Teaching
  `surfacer recon` to *detect* OAuth2 vs Mode B and populate these structs is
  a separate slice; until then the fields are populated by hand or by the
  existing `auth login` capture.
- **CSRF tokens and per-request nonces.** These are request-shaping, not
  identity, and belong nearer the endpoint model if ever.

## Implementation plan

### Phase 1: IR types (small)
- New `crates/surfacer-ir/src/auth.rs` with `AuthMode` and its structs, re-exported from `lib.rs`.
- Add `auth: Option<AuthMode>` to `HttpSurface` and `OperationDescriptor` (both `#[serde(default)]`).
- `resolve_auth` helper.
- Roundtrip tests, including the SUNAT three-state fixture. Verify the `use`/`use_` serialized key.

### Phase 2: lint (small)
- `IrLintError::AuthReacquireNeedsHeadless` for Mode B + `Reacquire`.
- `IrLintError::AuthOverrideUnresolved` where an operation override references `SecretRef::Acquired` with no acquisition in scope.

### Phase 3: emitters (medium)
- openapi: `securitySchemes` + per-op `security`, `x-surfacer-auth` fallback for Mode B.
- ts-cli, mcp: `resolveToken` + header attach; Mode B prompts `auth login`.
- just-bash: shell equivalents.
- help: auth marker column.
- shim: no change (exec owns runtime auth); add a test asserting that.

### Phase 4: runtime wiring (medium, separate concern)
- `surfacer exec` reads resolved auth from the IR and drives the right path, reusing `fetch_authenticated_html` and the session store for Mode B.
- `surfacer auth login` writes the captured token where `resolveToken` expects it, described by `BrowserAcquisition`.

## Decisions

**D1: auth on the surface, the operation, or both?**

Both, with per-operation override winning. SUNAT proves a single host runs
multiple modes, so surface-only fails. Operation-only would force repeating
the OAuth2 block on every SIRE/GRE/CPE command, which drifts. Surface default
+ optional override is the minimum that expresses SUNAT without repetition.
An explicit `AuthMode::None` variant lets an operation opt out of an
authenticated default (the `ficha-ruc` case). **Chosen: both, override wins.**

**D2: one flat `AuthMode` or acquisition/use split everywhere?**

Split only inside `BrowserBootstrappedToken`, shared `TokenUse` elsewhere.
For `ApiKey` and OAuth2, "use" is a header attach and "acquisition" is either
trivial (read a secret) or a single call, so a flat struct reads better. Mode
B is the only mode where acquisition and use happen in different processes at
different times, so only it carries the explicit `acquire` / `use_` split.
Factoring `TokenUse` out keeps the header-attach logic identical across
OAuth2 and Mode B. **Chosen: split only where the timeline actually splits.**

**D3: how does OpenAPI carry Mode B?**

It cannot, honestly. OpenAPI's `securitySchemes` has `apiKey`, `http`,
`oauth2`, `openIdConnect`, `mutualTLS`, none of which mean "browser-minted,
replayed headless." Options were: (a) lie and emit `apiKey` (a downstream SDK
would then think it can pass any string), (b) omit the operation entirely,
(c) emit it with an `x-surfacer-auth` extension and no `security`, plus a
description note. **Chosen: (c).** It matches the file's existing stance of
saying only what was observed and never inventing authority the evidence does
not support. This is also the single most defensible-vs-arguable call in the
doc: (b) would keep the spec strictly valid but hides a real operation, and
some would argue Mode B should not touch OpenAPI at all. I keep it visible-
but-flagged because a spec that silently drops half a government portal's
operations is worse than one that documents the operation and states plainly
that its auth is out of OpenAPI's vocabulary.

**D4: secrets in the IR?**

Never. `SecretRef` names an env var, a file, or an acquisition step. The IR
stays committable and shareable; resolving the actual value is the client's
job at runtime. This mirrors the repo rule that recon records what was
observed, not secrets it captured.
