<p align="center">
  <a href="https://surfacer.dev" target="_blank">
    <img src="https://raw.githubusercontent.com/Railly/crafter-station/main/public/logo.png" height="64" alt="Crafter Station">
  </a>
  <br />
  <h1 align="center">surfacer</h1>
</p>

<p align="center">
  Generate the interface instead of writing it.
</p>

<div align="center">

[![Release](https://img.shields.io/github/v/release/crafter-station/surfacer?display_name=tag&sort=semver&label=release)](https://github.com/crafter-station/surfacer/releases)
[![CI](https://github.com/crafter-station/surfacer/actions/workflows/ci.yml/badge.svg)](https://github.com/crafter-station/surfacer/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](./LICENSE)
[![Built with Crafter Station](https://img.shields.io/badge/built%20with-Crafter%20Station-orange)](https://crafterstation.com)
[![surfacer.dev](https://img.shields.io/badge/site-surfacer.dev-black)](https://surfacer.dev)

</div>

Describe a surface once, emit CLIs, agent tools, and native binaries that stay consistent because nothing hand-wrote them.

```bash
# Start from an IR describing the surface
surfacer lint ./news-ycombinator-com.surfacer.json

# Emit whatever interface you need from the same IR
surfacer install ./news-ycombinator-com.surfacer.json
surfacer emit ts-cli ./news-ycombinator-com.surfacer.json
surfacer emit just-bash ./news-ycombinator-com.surfacer.json

# Use it
news-ycombinator-com user id=Hunter17
```

## What it does

Software is increasingly operated by agents rather than people, and the interfaces they reach for were designed for humans. A CLI written by hand accumulates inconsistencies nobody notices until an agent hits them: `--json` on some commands and not others, a decorative header that breaks the parse, a subcommand missing from `--help` so the agent never learns it exists. Each one is a retry, a wasted token, or a wrong answer.

Those gaps are not carelessness. They are what happens when consistency depends on a person remembering, across years and contributors.

surfacer removes the remembering. It takes a declarative intermediate representation of a surface and compiles interfaces from it. Every command gets the same flag handling, the same JSON output, the same help, because one emitter wrote all of them. When the surface changes, you update the IR and re-emit, instead of rewriting the client.

surfacer does not map surfaces itself. That work needs judgment about terrain, about whether an official spec already exists, and about what counts as observed rather than assumed, so it belongs to the [`surface-recon`](https://github.com/crafter-station/skills/tree/main/skills/surface-recon) skill or to a person writing the descriptor by hand. Bring your own IR.

That matters most where no interface exists at all. Most of the useful internet has no documented API, and reaching it means either paying an LLM on every run, or hand-writing a client that breaks the next time the site moves. The second option is why unofficial clients die: yt-dlp maintains three release channels to keep up, and spotify-tui was abandoned when patching stopped scaling.

```
                                   ┌→ native CLI binary (via scriptc)
                                   ├→ CLI shim
                                   ├→ MCP server
IR (JSON) ─────────────────────────┼→ OpenAPI 3.1 document
                                   ├→ just-bash ExecutorConfig
                                   ├→ self-describing help
                                   └→ (your emitter here)
```

The IR is the artifact. It is a plain JSON file you can read, diff, edit, and commit, not an internal detail of a runtime you have to adopt. The IR is the source; the six interfaces are build artifacts, regenerated rather than maintained.

## Surfaces

HTTP endpoints and accessibility trees today. The IR models operations and transports rather than pages, so a new surface kind is a transport variant, not a rewrite.

## How it works

1. **Bring an IR**. The descriptor comes from the [`surface-recon`](https://github.com/crafter-station/skills/tree/main/skills/surface-recon) skill, which classifies the terrain, looks for an official spec before opening a browser, observes real traffic, and writes the `.surfacer.json`. Writing one by hand is supported and often faster: a target with a published OpenAPI spec needs a translation, not an investigation. `surfacer lint` is the gate either way, and it rejects an empty site name, an IR with no operations, an empty command path or description, an empty endpoint path, and duplicate command paths.

2. **Emit**. One IR, several targets: a CLI shim, a self-contained TypeScript program that [scriptc](https://scriptc.dev) compiles to a native binary needing no runtime, an MCP server any client can register, an OpenAPI 3.1 document for everything that already speaks OpenAPI, a [just-bash](https://github.com/vercel-labs/just-bash) ExecutorConfig, and `--help` derived from the same descriptor.

3. **Use**. The emitted interface fetches live data, extracts structured content, and renders it as a formatted list or JSON. Write operations are blocked by default, because the IR records that an operation writes and not whether the write is destructive.

4. **Re-emit**. This is the step the other three exist for. A target with no official API has no deprecation policy either, and the usual failure is not that the client was written badly, it is that the surface moved and nobody noticed until something returned the wrong answer quietly. `surfacer check` takes up to three endpoints from the IR as canaries, fetches their signatures, and compares them against a stored fingerprint. When a canary changes you update the IR and re-emit every target, which is cheaper than patching an integration you hand-wrote a year ago.

   Two limits worth knowing before relying on it. Drift only covers HTTP: an IR whose operations are not HTTP has nothing to fingerprint, and `check` says so and exits cleanly. And a canary is evidence, not proof, since three endpoints answering unchanged does not mean the response bodies kept their shape. Drift detected is a strong signal; drift not detected is a weak one. Reading `check` from a script, `fingerprintUpdated` is the field that matters: the baseline is rewritten on the run that finds drift, so a caller that polls and ignores that field sees the signal once and never again.

## Auth

Many surfaces worth reaching sit behind a login. The IR models how a surface authenticates, so the emitted interface inherits it instead of leaving each client to reinvent the flow.

Three shapes, because real portals use more than one:

- **OAuth2, headless.** The client mints its own token from a password or client-credentials grant. The ordinary case, and the one existing SDK generators already cover.
- **A static key or bearer** sent as a header on every request. The secret is never in the IR; the descriptor only names where the client reads it.
- **Browser-bootstrapped token.** Some portals mint a session token only inside their own browser login, with an audience a self-registered client can never request. surfacer captures that token once from the browser, then replays it headless until it expires. This is the mode no SDK generator models, and it is what a large share of government portals actually require.

The IR keeps acquisition and use separate: the browser step runs once, the headless calls run for the token's whole life. Auth attaches at the surface level as a default and can be overridden per operation, because one host often mixes a public lookup, an OAuth2 API, and a browser-only form.

## Commands

```
surfacer lint <ir-path> [--json]         Validate an IR file
surfacer install <ir-path> [--dest]      Install a site locally
surfacer check <site> [--json]           Detect drift against the recorded IR
surfacer shell [site]                    Interactive REPL over installed sites
surfacer emit cli <ir-path>              Generate a CLI shim binary
surfacer emit ts-cli <ir-path>           Generate a self-contained TypeScript CLI
surfacer emit mcp <ir-path>              Generate an MCP server
surfacer emit openapi <ir-path>          Generate an OpenAPI 3.1 document
surfacer emit just-bash <ir-path>        Generate a just-bash ExecutorConfig
surfacer schema                          Print this CLI's own surface as JSON
surfacer skills list                     List the manuals embedded in the binary
surfacer skills get core                 Print one of them
surfacer auth login <site>               Authenticate with a site
surfacer auth status <site>              Check auth state
surfacer auth logout <site>              Clear auth session
surfacer exec <site> <command>           Run a command (used by shims)
```

`--out-dir` on `emit` goes before the target: `surfacer emit --out-dir ./build ts-cli ir.json`.

`schema` is the entry point for an agent: it returns the commands, their arguments, and the exit codes as data instead of asking a reader to parse `--help` as prose. `skills` serves the agent-facing manual that ships inside the binary, so a caller does not need network access or a checkout to learn how the tool works.

## Installed site commands

```
<site> --help                 Show available commands
<site> <command>              Fetch and display structured content
<site> <command> --json       Machine-readable JSON output
<site> open [command] <index> Open item #N in browser
```

## What the generated interface guarantees

These hold for every emitted command, because one emitter produced all of them:

- `--json` exists on every command and prints the response body alone. No banner, no version line, no color codes on the machine channel. Without it, commands print a one-line JSON summary of status, URL, and size.
- `--help` lists every command and every parameter the IR records, with an example value where one was captured. A command that exists is a command help mentions.
- Parameters carry a name and an observed example rather than a declared type, because the IR is built from traffic and not from documentation. That is enough for help to name them instead of leaving an agent to guess from a URL.
- Writes are blocked by default and fail loudly, naming the constant to edit. The IR records that an operation writes, not whether the write is destructive, so the emitter refuses to guess.
- Exit codes are distinct: `0` success, `1` an unsuccessful response, `77` blocked by the trust gate, `127` unknown command.

### And surfacer follows them too

A compiler that emits agent-first CLIs while not being one is not a defensible position, so the same list runs against surfacer itself:

- `lint` and `check` print JSON on stdout whenever stdout is not a terminal, with no flag passed. `--json` forces it at a terminal.
- Human narration goes to stderr, so piping a command never mixes prose into the payload. A failed `lint` still returns a document naming every error; the exit code is what marks it as a failure.
- `surfacer schema` prints the command surface as JSON, so the surface is readable as data.
- Exit codes are three, and they say whether retrying is worth anything: `0` succeeded, `1` the request itself was wrong (a malformed IR, an unknown site) and will fail the same way on a retry, `2` the filesystem or network failed and a retry may work.

The rules are executable, not aspirational. [`crates/surfacer-app/tests/agent_first_rules.rs`](./crates/surfacer-app/tests/agent_first_rules.rs) runs them against the real binary through `CARGO_BIN_EXE_surfacer`, and [`crates/surfacer-emit-cli/tests/agent_first_rules.rs`](./crates/surfacer-emit-cli/tests/agent_first_rules.rs) runs the same list against emitter output. One list, two subjects; a rule added to either belongs in both. They exist because when they were first written, surfacer broke four of the six rules it was already enforcing on what it emitted.

Note the two exit code sets are different and both correct. `0/1/2` is surfacer's own, where the question is whether a retry helps. `0/1/77/127` above is the emitted CLI's, where a call can also be refused by the trust gate or name a command that does not exist.

## Requirements

Compiling an IR needs nothing beyond the binary. `lint`, `emit`, `install`, and `schema` work on a machine with no browser and no Node.

Per target:

- Rust toolchain, to compile the `cli` shim
- [scriptc](https://scriptc.dev), to compile the `ts-cli` output to a native binary
- [defuddle](https://github.com/anthropics/defuddle), for HTML content extraction when an installed site runs a command

Only for `surfacer auth login` against a surface that mints its token inside a browser session:

- [agent-browser](https://github.com/vercel-labs/agent-browser) for browser automation
- A Chromium browser with `--remote-debugging-port=9222`

## Quick start

Install surfacer and an existing IR for Hacker News in 4 lines:

```bash
curl -fsSL https://surfacer.dev/install.sh | sh
curl -O https://raw.githubusercontent.com/crafter-station/surfacer/main/examples/news-ycombinator-com.surfacer.json
surfacer install ./news-ycombinator-com.surfacer.json --dest ~/.cargo/bin
news-ycombinator-com news --json | jq '.items[].fields.title.value'
```

That's the full pipeline against Hacker News with zero LLM tokens at runtime. More IRs in [`examples/`](./examples) (SUNAT, more on the way).

The installer takes `SURFACER_VERSION` to pin a release and `SURFACER_INSTALL_DIR` to choose the destination (default `~/.local/bin`). Releases carry binaries for macOS on Apple Silicon and Intel, and Linux x86_64. To build from source instead:

```bash
cargo install --git https://github.com/crafter-station/surfacer surfacer
```

### Reach agents without installing anything

An MCP server, registered with any client:

```bash
surfacer emit mcp ./news-ycombinator-com.surfacer.json
npm install @modelcontextprotocol/server zod
claude mcp add hn -- node emit/mcp/news-ycombinator-com.mcp.js
```

Or an OpenAPI document, which every SDK generator and HTTP client already reads:

```bash
surfacer emit openapi ./news-ycombinator-com.surfacer.json
```

The spec says only what the IR records. Response schemas are absent because the IR carries observed bodies, not contracts, and inventing one would make the document look more authoritative than the evidence behind it.

### Emit a standalone binary

An IR can also become a self-contained CLI that needs no runtime at all: not even surfacer:

```bash
surfacer emit ts-cli ./news-ycombinator-com.surfacer.json
scriptc build emit/ts-cli/news-ycombinator-com.ts --dynamic -o hn
./hn user id=Hunter17
```

[`scriptc`](https://scriptc.dev) compiles the emitted TypeScript to a native binary. On the Hacker News descriptor, 94% of statements compile statically; `fetch` has no static lowering yet, so the network call runs in the embedded engine and `--dynamic` is required.

### Your own surface

Point `surface-recon` at the target and ask it for the IR, or write the descriptor by hand. Then the same three steps as any other IR:

```bash
surfacer lint ./your-site-example.surfacer.json
surfacer install ./your-site-example.surfacer.json
surfacer check your-site-example
```

If the surface needs a login the target mints only inside its own browser, that step needs a Chromium with debugging enabled:

```bash
# Start a browser with debugging
/Applications/Comet.app/Contents/MacOS/Comet --remote-debugging-port=9222 &
# or: google-chrome --remote-debugging-port=9222 &

surfacer auth login your-site-example
```

## Architecture

```
surfacer/
├── surfacer-ir          IR types (SiteDescriptor, extractors, registry)
├── surfacer-probe       agent-browser wrapper, HAR and token capture for auth
├── surfacer-classifier  11-feature heuristic backend archetype detection
├── surfacer-emit-cli    CLI shim + just-bash ExecutorConfig generation
├── surfacer-install     Local IR installer + registry
└── surfacer-app         CLI entry point, orchestration
```

## The thesis

Compiling agent behavior to a reusable artifact is an active research area, and running the model once at design time rather than on every execution is not a new idea. surfacer takes two positions that are less common.

**The IR is neutral.** Comparable systems emit a plan their own runtime consumes; adopting the tool means adopting the runtime. Here the descriptor is a plain JSON file, and every consumer is downstream of it, including consumers nobody has written yet. The site under `www/` is one: it renders the emitters without importing the CLI.

**The compiler does not run the model at all.** The published work on this pattern covers the first half of the pipeline: call an LLM once, freeze what it found into an IR, then execute deterministically from there. surfacer starts after that half is over. It has no recon step to defend, no prompt, no model of its own, which puts it beside that work rather than in competition with it. What the prior work leaves open is the half surfacer keeps: many targets from one neutral IR, and a way to notice when the surface underneath has moved. In a tool that also does the mapping, multi-target emission and drift detection are features. Here they are the whole product.

## A real target: SUNAT

Peru's tax portal is the case that shaped the auth model. Its monthly-declaration form looks like a server-rendered page and behaves like one: driving the DOM never works, because the fields stay disabled until a background call returns. Underneath sits a JSON API, and reaching it needs a session token the portal mints only during its own browser login.

That is the browser-bootstrapped mode end to end: open the browser once to capture the token, then read the API headless for its hour. A form that fought every DOM automation becomes a handful of authenticated GETs. The [`sunat-cli`](https://github.com/crafter-research/sunat-cli) client is the first surface driving surfacer's auth work, and its recon fed the IR schema directly.

The three auth states live together on one host: an OAuth2 default for SIRE, a browser-bootstrapped override for the F616 form, and an explicit public lookup for the padron. [`examples/sunat-declaraciones.surfacer.json`](./examples/sunat-declaraciones.surfacer.json) is a curated fixture, not recon output, that carries all three. Every auth field in it was verified live. `surfacer emit openapi` on it produces a spec that validates, with `oauth2` on the SIRE op, `security: []` on the padron op, and an `x-surfacer-auth` extension on F616 where OpenAPI has no vocabulary for the browser mode.

## Target surfaces

surfacer targets surfaces **without official CLIs or APIs**: government portals, internal systems, regional SaaS, legacy software. It does not compete with vendor CLIs like gh, stripe, or aws.

Prefer surfaces you already have the right to reach: your own accounts, your company's own systems, public records you are entitled to read. The generated interface inherits whatever permission you already had, and nothing more.

## Status

Early development. The pipeline works end-to-end for public HTML sites, from an IR through every emitter. The IR models three auth shapes including the browser-bootstrapped token described above; emitter support for them lands target by target. Extractors and field names come from whoever wrote the IR, since the compiler no longer infers them. Drift detection covers HTTP operations only.

## License

MIT
