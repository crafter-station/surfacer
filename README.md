# surfacer

Keep integrations against systems with no API, without rewriting them every time they change.

```bash
# Map a surface once
surfacer recon https://news.ycombinator.com --auto --yes

# Emit whatever interface you need from the same IR
surfacer install ./surfacer-recon-hn/news-ycombinator-com.surfacer.json
surfacer emit ts-cli ./news-ycombinator-com.surfacer.json
surfacer emit just-bash ./news-ycombinator-com.surfacer.json

# Use it
news-ycombinator-com user id=Hunter17
```

## What it does

Most of the useful internet has no documented API. Reaching it means either paying an LLM on every run, or hand-writing a client that breaks the next time the site changes. The second option is why unofficial clients die: yt-dlp maintains three release channels to keep up, and spotify-tui was abandoned when patching stopped scaling.

surfacer maps a surface once into a declarative intermediate representation, then emits interfaces from it. When the surface changes, you re-run recon instead of rewriting the client. Nothing calls an LLM at runtime.

```
                                   ┌→ native CLI binary (via scriptc)
                                   ├→ CLI shim
surface → recon → IR (JSON) ───────┼→ just-bash ExecutorConfig
                                   ├→ self-describing help
                                   └→ (your emitter here)
```

The IR is the artifact. It is a plain JSON file you can read, diff, edit, and commit, not an internal detail of a runtime you have to adopt.

## Surfaces

HTTP endpoints and accessibility trees today. The IR models operations and transports rather than pages, so a new surface kind is a transport variant, not a rewrite.

## How it works

1. **Recon**. `surfacer recon <url> --auto` opens a browser, autonomously navigates the site, captures HTTP traffic, classifies the backend archetype, detects repeating content patterns, and emits a declarative IR.

2. **Emit**. One IR, several targets: a CLI shim, a self-contained TypeScript program that [scriptc](https://scriptc.dev) compiles to a native binary needing no runtime, a [just-bash](https://github.com/vercel-labs/just-bash) ExecutorConfig for agents, and `--help` derived from the same descriptor.

3. **Use**. The emitted interface fetches live data, extracts structured content, and renders it as a formatted list or JSON. Write operations are blocked by default: recon cannot tell a harmless write from a destructive one.

4. **Re-run**. `surfacer check` fingerprints canary endpoints to detect drift. When the surface moves, recon again and re-emit every target.

## Commands

```
surfacer recon <url> [--auto] [--yes]    Map a surface into an IR
surfacer install <ir-path>               Install a site locally
surfacer check <site>                    Detect drift against the recorded IR
surfacer shell [site]                    Interactive REPL over installed sites
surfacer emit cli <ir-path>              Generate a CLI shim binary
surfacer emit ts-cli <ir-path>           Generate a self-contained TypeScript CLI
surfacer emit just-bash <ir-path>        Generate a just-bash ExecutorConfig
surfacer lint <ir-path>                  Validate an IR file
surfacer auth login <site>               Authenticate with a site
surfacer auth status <site>              Check auth state
surfacer auth logout <site>              Clear auth session
surfacer exec <site> <command>           Run a command (used by shims)
```

## Installed site commands

```
<site> --help                 Show available commands
<site> <command>              Fetch and display structured content
<site> <command> --json       Machine-readable JSON output
<site> open [command] <index> Open item #N in browser
```

## Requirements

- [agent-browser](https://github.com/vercel-labs/agent-browser) for browser automation
- [defuddle](https://github.com/anthropics/defuddle) for HTML content extraction
- Rust toolchain (for shim compilation)
- A Chromium browser with `--remote-debugging-port=9222`

## Quick start

Skip recon. Install surfacer and a pre-generated IR for Hacker News in 4 lines:

```bash
curl -fsSL https://raw.githubusercontent.com/crafter-station/surfacer/main/install.sh | sh
curl -O https://raw.githubusercontent.com/crafter-station/surfacer/main/examples/news-ycombinator-com.surfacer.json
surfacer install ./news-ycombinator-com.surfacer.json --dest ~/.cargo/bin
news-ycombinator-com news --json | jq '.items[].fields.title.value'
```

That's the full pipeline against Hacker News with zero LLM tokens at runtime. More IRs in [`examples/`](./examples) (SUNAT, more on the way).

The installer takes `SURFACER_VERSION` to pin a release and `SURFACER_INSTALL_DIR` to choose the destination (default `~/.local/bin`). To build from source instead:

```bash
cargo install --git https://github.com/crafter-station/surfacer surfacer
```

### Emit a standalone binary

An IR can also become a self-contained CLI that needs no runtime at all: not even surfacer:

```bash
surfacer emit ts-cli ./news-ycombinator-com.surfacer.json
scriptc build emit/ts-cli/news-ycombinator-com.ts --dynamic -o hn
./hn user id=Hunter17
```

[`scriptc`](https://scriptc.dev) compiles the emitted TypeScript to a native binary. On the Hacker News descriptor, 94% of statements compile statically; `fetch` has no static lowering yet, so the network call runs in the embedded engine and `--dynamic` is required.

To recon your own site, you also need a Chromium with debugging enabled:

```bash
# Start a browser with debugging
/Applications/Comet.app/Contents/MacOS/Comet --remote-debugging-port=9222 &
# or: google-chrome --remote-debugging-port=9222 &

surfacer recon https://your-site.example --auto --yes
surfacer install ./surfacer-recon-your-site-example/your-site-example.surfacer.json
```

## Architecture

```
surfacer/
├── surfacer-ir          IR types (SiteDescriptor, extractors, registry)
├── surfacer-probe       agent-browser wrapper, auto-recon, HAR capture
├── surfacer-classifier  11-feature heuristic backend archetype detection
├── surfacer-emit-cli    CLI shim + just-bash ExecutorConfig generation
├── surfacer-install     Local IR installer + registry
└── surfacer-app         CLI entry point, orchestration
```

## The thesis

Most AI agent runtimes re-run an LLM every time an agent interacts with a website (Browser Use, Stagehand, Operator). surfacer takes the opposite approach: reverse-engineer the site once, emit a deterministic interface, use it forever. One LLM pass during recon, zero tokens at runtime.

## Target sites

surfacer targets sites **without official CLIs or APIs**: government portals, banks, regional SaaS, legacy systems. It does not compete with vendor CLIs (gh, stripe, vercel, aws).

## Status

Early development. The pipeline works end-to-end for public HTML sites. Auth support exists but is minimal. Extractors auto-detect repeating patterns but field naming uses heuristics (LLM naming coming soon).

## License

MIT
