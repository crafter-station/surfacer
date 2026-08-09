# surfacer

Generate the interface instead of writing it. Map a surface once, emit CLIs, agent tools, and native binaries that stay consistent because nothing hand-wrote them.

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

Software is increasingly operated by agents rather than people, and the interfaces they reach for were designed for humans. A CLI written by hand accumulates inconsistencies nobody notices until an agent hits them: `--json` on some commands and not others, a decorative header that breaks the parse, a subcommand missing from `--help` so the agent never learns it exists. Each one is a retry, a wasted token, or a wrong answer.

Those gaps are not carelessness. They are what happens when consistency depends on a person remembering, across years and contributors.

surfacer removes the remembering. It maps a surface once into a declarative intermediate representation, then generates interfaces from it. Every command gets the same flag handling, the same JSON output, the same help, because one emitter wrote all of them. When the surface changes, you re-run recon instead of rewriting the client.

That matters most where no interface exists at all. Most of the useful internet has no documented API, and reaching it means either paying an LLM on every run, or hand-writing a client that breaks the next time the site moves. The second option is why unofficial clients die: yt-dlp maintains three release channels to keep up, and spotify-tui was abandoned when patching stopped scaling.

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

## What the generated interface guarantees

These hold for every emitted command, because one emitter produced all of them:

- `--json` exists on every command and prints the response body alone. No banner, no version line, no color codes on the machine channel. Without it, commands print a one-line JSON summary of status, URL, and size.
- `--help` lists every command and every parameter recon observed, with an example value where one was seen. A command that exists is a command help mentions.
- Parameters carry a name and an observed example rather than a declared type, since recon reads traffic and not documentation. That is enough for help to name them instead of leaving an agent to guess from a URL.
- Writes are blocked by default and fail loudly, naming the constant to edit. Recon cannot tell a harmless write from a destructive one, so it refuses to guess.
- Exit codes are distinct: `0` success, `1` an unsuccessful response, `77` blocked by the trust gate, `127` unknown command.

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

Compiling agent behavior to a reusable artifact is an active research area, and running the model once at design time rather than on every execution is not a new idea. surfacer takes two positions that are less common.

**The IR is neutral.** Comparable systems emit a plan their own runtime consumes; adopting the tool means adopting the runtime. Here the descriptor is a plain JSON file, and every consumer is downstream of it, including consumers nobody has written yet. The site under `www/` is one: it renders the emitters without importing the CLI.

**Recon comes first.** Most work in this space starts from a task already defined, and makes repeating it cheaper. surfacer starts earlier, from a surface nobody documented, and asks what it exposes at all.

## Target surfaces

surfacer targets surfaces **without official CLIs or APIs**: government portals, internal systems, regional SaaS, legacy software. It does not compete with vendor CLIs like gh, stripe, or aws.

Prefer surfaces you already have the right to reach: your own accounts, your company's own systems, public records you are entitled to read. The generated interface inherits whatever permission you already had, and nothing more.

## Status

Early development. The pipeline works end-to-end for public HTML sites. Auth support exists but is minimal. Extractors auto-detect repeating patterns but field naming uses heuristics (LLM naming coming soon).

## License

MIT
