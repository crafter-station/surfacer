# examples

Ready-to-use IRs. Install one and the generated CLI works, with no mapping step of your own.

## Try it (4 lines)

```bash
cargo install --git https://github.com/crafter-station/surfacer surfacer
curl -O https://raw.githubusercontent.com/crafter-station/surfacer/main/examples/news-ycombinator-com.surfacer.json
surfacer install ./news-ycombinator-com.surfacer.json --dest ~/.cargo/bin
news-ycombinator-com news --json | jq '.items[0:3]'
```

That's it. No clone, no build, no browser.

If you want to use it interactively after install:

```bash
news-ycombinator-com --help
news-ycombinator-com news
news-ycombinator-com open 1
```

## Available examples

| IR | Site | Operations | Auth | Notes |
|---|---|---|---|---|
| `news-ycombinator-com.surfacer.json` | [news.ycombinator.com](https://news.ycombinator.com) | 10 | none | Server-rendered, `HttpOnly`. The one to start with. |
| `www-sunat-gob-pe.surfacer.json` | [www.sunat.gob.pe](https://www.sunat.gob.pe) | 16 | none | Peru's tax portal, public lookups only. Server-rendered, no public API. |
| `sunat-declaraciones.surfacer.json` | SUNAT declarations | 3 | oAuth2 | A curated fixture, not observed output. See below. |

`sunat-declaraciones` is the auth reference: it carries all three auth states on one host, an OAuth2 default, a browser-bootstrapped override, and an explicit public opt-out. Every auth field in it was verified against the live service. It exists because no single observed capture happened to contain all three, and the emitters needed one input that exercised every branch.

## Why these IRs are committed

An IR is source, not build output. It is what a mapping produced, in a form you can read, diff, and correct, and it is the input every emitter reads. Committing them means anyone can install a working CLI without repeating the mapping, which is the whole point of writing the descriptor down.

surfacer does not map surfaces itself. It reads an IR and emits from it. Yours comes from one of two places: the [`surface-recon`](https://github.com/crafter-station/skills) skill, which classifies the terrain, checks for an official spec before opening a browser, observes real traffic, and writes the descriptor, or your own hand if you already know the surface. Either way, run `surfacer lint` on it before anything downstream reads it.

When you have an IR for a surface worth sharing, drop the file here and open a PR. Every file in this directory is checked by `crates/surfacer-app/tests/examples_are_valid.rs`, which runs `surfacer lint` on each one and rejects command paths that read like URLs rather than something a person would type. That gate exists because both were wrong here at once: one descriptor had seven operations collapsed onto a single command path, and another named a command after a file that returns 404.

## What's in an IR

```jsonc
{
  "meta": { "siteName": "...", "displayName": "...", "irVersion": "0.1.0" },
  "provenance": { "technique": "http", "classifierBucket": "HttpOnly" },
  "operations": [
    {
      "commandPath": ["news"],
      "operationKind": "read",
      "transport": { "kind": "http", "endpointIndex": 2 },
      "extractor": { /* CSS selectors and field shape */ }
    }
  ],
  "http": { "endpoints": [ /* observed endpoints, and the surface auth */ ] },
  "ax":   { "actions":   [ /* accessibility tree captures */ ] }
}
```

`provenance.technique` says how the surface was mapped, and it is worth reading before trusting a descriptor. `http`, `ax`, and `hybrid` come from a mechanical probe. `agent` means a person or an agent mapped it with judgment, which produces a better descriptor and a less repeatable one, and `surfacer check` reads this field to decide what to tell you when the site drifts.

The IR is portable and every emitter is downstream of it: `surfacer emit cli` (a Rust shim), `ts-cli` (a TypeScript program [scriptc](https://scriptc.dev) compiles to a native binary), `mcp` (an MCP server), `openapi` (an OpenAPI 3.1 document), and `just-bash` (a [just-bash](https://github.com/vercel-labs/just-bash) `ExecutorConfig`).
