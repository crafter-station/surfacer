# webctl

Turn any website into an installable CLI.

```bash
# Discover a site autonomously
webctl recon https://news.ycombinator.com --auto --yes

# Install it
webctl install ./webctl-recon-hn/news-ycombinator-com.webctl.json

# Use it
news-ycombinator-com news
news-ycombinator-com news --json
news-ycombinator-com open 1
```

## What it does

webctl reverse-engineers websites into declarative intermediate representations (IRs) that become installable CLIs. One recon pass, zero LLM tokens at runtime.

```
Website → webctl recon → IR (JSON) → webctl install → CLI binary
                                   → webctl emit just-bash → ExecutorConfig
```

The same IR feeds multiple runtimes: standalone CLI shims, [just-bash](https://github.com/vercel-labs/just-bash) ExecutorConfig, and more.

## How it works

1. **Recon**. `webctl recon <url> --auto` opens a browser, autonomously navigates the site, captures HTTP traffic, classifies the backend archetype, detects repeating content patterns, and emits a declarative IR.

2. **Install**. `webctl install <ir>` generates a thin CLI shim (~300KB) and places it in your PATH.

3. **Use**. The installed CLI fetches live data from the site, extracts structured content via CSS selectors, and renders it as a formatted list or JSON.

## Commands

```
webctl recon <url> [--auto] [--yes]    Reverse-engineer a website
webctl install <ir-path>               Install a site locally
webctl emit cli <ir-path>              Generate a CLI shim binary
webctl emit just-bash <ir-path>        Generate a just-bash ExecutorConfig
webctl lint <ir-path>                  Validate an IR file
webctl auth login <site>               Authenticate with a site
webctl auth status <site>              Check auth state
webctl auth logout <site>              Clear auth session
webctl exec <site> <command>           Run a command (used by shims)
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

Skip recon. Install webctl and a pre-generated IR for Hacker News in 4 lines:

```bash
curl -fsSL https://raw.githubusercontent.com/crafter-station/webctl/main/install.sh | sh
curl -O https://raw.githubusercontent.com/crafter-station/webctl/main/examples/news-ycombinator-com.webctl.json
webctl install ./news-ycombinator-com.webctl.json --dest ~/.cargo/bin
news-ycombinator-com news --json | jq '.items[].fields.title.value'
```

That's the full pipeline against Hacker News with zero LLM tokens at runtime. More IRs in [`examples/`](./examples) (SUNAT, more on the way).

The installer takes `WEBCTL_VERSION` to pin a release and `WEBCTL_INSTALL_DIR` to choose the destination (default `~/.local/bin`). To build from source instead:

```bash
cargo install --git https://github.com/crafter-station/webctl webctl
```

### Emit a standalone binary

An IR can also become a self-contained CLI that needs no runtime at all — not even webctl:

```bash
webctl emit ts-cli ./news-ycombinator-com.webctl.json
scriptc build emit/ts-cli/news-ycombinator-com.ts --dynamic -o hn
./hn user id=Hunter17
```

[`scriptc`](https://scriptc.dev) compiles the emitted TypeScript to a native binary. On the Hacker News descriptor, 94% of statements compile statically; `fetch` has no static lowering yet, so the network call runs in the embedded engine and `--dynamic` is required.

To recon your own site, you also need a Chromium with debugging enabled:

```bash
# Start a browser with debugging
/Applications/Comet.app/Contents/MacOS/Comet --remote-debugging-port=9222 &
# or: google-chrome --remote-debugging-port=9222 &

webctl recon https://your-site.example --auto --yes
webctl install ./webctl-recon-your-site-example/your-site-example.webctl.json
```

## Architecture

```
webctl/
├── webctl-ir          IR types (SiteDescriptor, extractors, registry)
├── webctl-probe       agent-browser wrapper, auto-recon, HAR capture
├── webctl-classifier  11-feature heuristic backend archetype detection
├── webctl-emit-cli    CLI shim + just-bash ExecutorConfig generation
├── webctl-install     Local IR installer + registry
└── webctl-app         CLI entry point, orchestration
```

## The thesis

Most AI agent runtimes re-run an LLM every time an agent interacts with a website (Browser Use, Stagehand, Operator). webctl takes the opposite approach: reverse-engineer the site once, emit a deterministic interface, use it forever. One LLM pass during recon, zero tokens at runtime.

## Target sites

webctl targets sites **without official CLIs or APIs**: government portals, banks, regional SaaS, legacy systems. It does not compete with vendor CLIs (gh, stripe, vercel, aws).

## Status

Early development. The pipeline works end-to-end for public HTML sites. Auth support exists but is minimal. Extractors auto-detect repeating patterns but field naming uses heuristics (LLM naming coming soon).

## License

MIT
