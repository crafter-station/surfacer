# Auth in the emitters: slicing

```yaml
status: shaping
depends_on: docs/shaping-auth-ir.md (merged as AuthMode in surfacer-ir), the six emitters in surfacer-emit-cli
```

> The AuthMode slice is merged: the IR now carries auth and `resolve_auth()` picks the effective mode per operation. This doc slices the next step, teaching the six emitters to consume it. The what is already decided in `shaping-auth-ir.md` under "How each emitter consumes auth". This doc is about order, demo-ability, and one real unknown.

## Frame

### Problem

- The IR models auth but every emitter still writes `fetch(url)` with no credentials. A descriptor with auth produces a client that gets a 401 or a redirect.
- The six emitters do not consume auth uniformly: the shim can lean on the existing runtime, the headless targets cannot.
- One assumption in the IR doc is not yet true in code, and it gates half the work.

### Outcome

- Each emitter carries the auth the IR describes, or degrades honestly when its format cannot.
- Every slice ends in something demoable against a real descriptor (SUNAT for the browser mode, a public site for None).
- The write-gate stays orthogonal: auth never widens what kind of operation a client will run.

## The one real unknown (spike first)

**AE-SPIKE: `auth login` captures a session name, not a token.**

Verified in `crates/surfacer-app/src/cli.rs:1069`. Today `auth login`:
- opens the browser, waits for ENTER, writes `session-name` to the site dir
- stores the browser session *reference*, not any extracted token
- works for the **shim** because `surfacer exec` re-drives that live session via `fetch_authenticated_html`

The `BrowserBootstrappedToken` mode needs the opposite: a captured **token** (the SUNAT idCache: a JWT that rides a query param on one navigation request) written somewhere a fully headless client can read it after the browser is closed. That is exactly what `sunat-cli`'s `plataforma/session.ts` already does by hand.

So Mode B in the headless emitters (ts-cli, mcp, just-bash) is blocked until `auth login` learns to capture and persist a token per the IR's `TokenCapture`. This is the first slice.

| # | Question |
|---|----------|
| **AE-Q1** | Where does `auth login` read the network log / URL to apply a `TokenCapture::RequestQueryParam`? Does `surfacer_probe` expose captured requests the way `agent-browser network requests` does? |
| **AE-Q2** | What file does the captured token go in, and what does it carry (token, expiresAt, capturedAt)? `sunat-cli` uses `~/.sunat/plataforma-token.json`; the surfacer equivalent is the site dir. |
| **AE-Q3** | How does a headless client find its TTL and know to print the reauth hint vs fail hard? |

**Acceptance:** we can describe how `auth login` captures a token via each `TokenCapture` variant, where it persists, and how a headless emitter reads and expiry-checks it.

## Shape: follow the IR doc, sliced by consumer difficulty

The IR doc already fixed the mechanism per emitter. The shape here is only the **cut**: order the six emitters so each slice demos, cheapest and least-blocked first.

The natural axis is how much runtime each emitter needs:

| Emitter | Auth work | Blocked by AE-SPIKE? |
|---|---|---|
| shim | none (exec owns runtime auth) | no |
| help | render a lock glyph + hint from resolved auth | no |
| openapi | static securitySchemes + x-surfacer-auth fallback | no |
| ts-cli | resolveToken + header attach, three modes | Mode B only |
| mcp | same runtime as ts-cli | Mode B only |
| just-bash | shell equivalents of the three modes | Mode B only |

Three of the six (shim, help, openapi) need **no token capture at all**: they either defer to the runtime, describe the auth, or render it statically. They can ship before the spike resolves. That is the first cut.

## Slices

**AE-V1: token capture in `auth login`** (resolves AE-SPIKE)
- Extend `auth login` to apply the site's `TokenCapture` after the human logs in: read the matching request, pull the token, write `{token, expiresAt, capturedAt}` to the site dir.
- Port the proven logic from `sunat-cli/plataforma/session.ts` (RequestQueryParam on servletAcceso).
- Demo: `surfacer auth login sunat` then `cat ~/.surfacer/sites/sunat/token.json` shows a JWT with a real expiry.

**AE-V2: shim assertion + help markers**
- shim: no codegen change; add a test asserting an authed descriptor still emits a shim that shells to exec (guards the "no auth codegen" claim in the IR doc).
- help: lock glyph and `needs: surfacer auth login <site>` for any command whose resolved auth is not None.
- Demo: `surfacer emit help sunat` shows locks on the F616 commands, none on a public lookup.

**AE-V3: openapi securitySchemes**
- Emit `components.securitySchemes` for None / ApiKey / OAuth2, per-op `security`, and the `x-surfacer-auth` extension + description note for Mode B.
- Demo: `surfacer emit openapi sunat | ...` validates, SIRE ops carry `oauth2`, F616 ops carry the extension and no `security`.

**AE-V4: ts-cli three modes** (needs AE-V1)
- `authForOperation`, `resolveToken` (apiKey / oAuth2 / browserBootstrappedToken reading the AE-V1 file), header attach per `TokenUse`. Write-gate untouched.
- Demo: emit ts-cli for SUNAT, run a read op, it attaches the captured idCache and returns 200 headless.

**AE-V5: mcp three modes** (needs AE-V1)
- Same runtime record as ts-cli, adapted to the MCP server shape.
- Demo: register the emitted MCP server, an F616 read tool returns real data.

**AE-V6: just-bash three modes** (needs AE-V1)
- Shell equivalents: `curl -H`, token POST piped through `jq`, file read for Mode B with the reauth hint.
- Demo: the emitted bash config reads a SUNAT op headless.

### Slice ordering

AE-V1 unblocks V4/V5/V6. V2 and V3 are independent and can go in parallel or first, since they need no token capture. So the critical path is V1 then any of V4/V5/V6; V2 and V3 are free wins that ship whenever.

## What this slice does NOT do

- No LLM field naming, no new surface kinds. Auth only.
- No new auth modes beyond the four already in the IR.
- No OTP / multi-step browser acquisition (the IR doc parks this as a richer `BrowserAcquisition` for later).
- No credential storage redesign: secrets keep resolving from env / file / keychain as they do now. The IR names locations; it never holds a secret.
