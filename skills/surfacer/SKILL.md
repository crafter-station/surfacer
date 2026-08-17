---
name: surfacer
description: "Compile a mapped surface into working interfaces and keep them alive as the target changes. Use when the user has a recon report or an IR for a service without a usable API, wants a CLI, an OpenAPI spec, or an MCP server generated from it, asks to check whether a mapped target has drifted, or says surfacer, emit, or bring your own IR. Also use after surface-recon produces an IR."
---

# surfacer

Compile a description of a surface into interfaces, then detect when the surface moves under you.

Install: see the repository README. Requires no runtime.

## What this is

surfacer is a compiler. Its input is an IR: a JSON file describing what a target exposes, which operations exist, and how they authenticate. Its outputs are six emitted interfaces, all from that one file.

It does not map surfaces itself. That work needs judgment about terrain, official specs, and what counts as observed, and it belongs to `surface-recon`. Bring your own IR, from that skill or written by hand.

The reason to keep the IR rather than the generated code: a target with no official API has no deprecation policy either. `surfacer check` tells you when it moved, and re-emitting is cheaper than rewriting an integration you hand-wrote a year ago. The IR is the source. The six interfaces are build artifacts.

## The loop

```
IR (.surfacer.json)
  -> surfacer lint      validate before anything downstream
  -> surfacer install   register it under a site name
  -> surfacer emit      generate interfaces
  -> surfacer check     detect drift later
```

## Getting an IR

Run `surface-recon` against the target and ask for the IR target. It classifies the terrain, checks for an official spec first, observes real traffic, and writes both a report and a `.surfacer.json`.

Writing one by hand is supported and sometimes faster: a target with a published OpenAPI spec needs a translation, not a recon.

Either way `surfacer lint` is the gate. It rejects an empty site name, an IR with no operations, an empty command path or description, a declared but empty HTTP surface, an empty endpoint path, and duplicate command paths. That last one is the rule people hit: real targets have operations whose natural names collide, and every emitter names operations from `commandPath`, so a duplicate makes the emitted surface ambiguous rather than merely ugly.

## Commands

```bash
surfacer lint <ir-path>            # validate an IR
surfacer install <ir-path>         # register under its site name
surfacer emit <target> <ir-path>   # generate an interface
surfacer exec <site> [args...]     # run an operation directly
surfacer check <site>              # detect drift against the live target
surfacer auth login <site>         # capture a session for authenticated sites
surfacer schema                    # this command surface, as JSON
surfacer shell                     # interactive
```

Start with `surfacer schema` rather than `--help`. It returns the commands, their arguments, and the exit codes as data, which is what this document would otherwise ask you to read as prose.

**Output follows the same rules surfacer enforces on the CLIs it emits.** `lint` and `check` return JSON on stdout whenever stdout is not a terminal, with no flag needed, and `--json` forces it at a terminal. Human narration goes to stderr, so piping a command never mixes prose into the payload. A failed `lint` still returns a document naming every error; the exit code is what marks it as a failure.

Exit codes are three: `0` succeeded, `1` the request was wrong (a malformed IR, an unknown site) and will fail the same way if retried, `2` the filesystem or network failed and retrying may work.

`--out-dir` on `emit` goes before the target: `surfacer emit --out-dir ./build ts-cli ir.json`.

## The six targets

| Target | What it produces | Reach for it when |
|---|---|---|
| `cli` | TypeScript CLI, compilable to a native binary | A human or an agent drives the target from a terminal |
| `shim` | Rust shim | The caller is Rust, or you want one static binary |
| `openapi` | OpenAPI 3.1 spec | Existing tooling consumes specs, or you want documentation |
| `mcp` | MCP server over stdio | An agent should call the target as tools |
| `just-bash` | just-bash config | The project already runs on that harness |
| `help` | Help text | You want the surface readable before generating anything |

Emitting more than one is normal, and it is the reason the IR exists as a separate artifact. A spec for the documentation, an MCP server for the agent, and a CLI for the human all describe the same surface, and they cannot disagree because they compile from the same file.

## Drift

`surfacer check <site>` takes up to three endpoints from the IR as canaries, fetches their signatures, and compares them against a stored fingerprint. The first run saves the baseline. Later runs report which canaries changed.

Two limits worth knowing before you rely on it:

**It only covers HTTP.** An IR whose operations are not HTTP has nothing to fingerprint, and check says so and exits cleanly. Terrain that is a file format, a device, or an accessibility tree gets no drift signal today.

**A canary is evidence, not proof.** Three endpoints answering unchanged does not mean the response bodies kept their shape. Drift detected is a strong signal; drift not detected is a weak one.

**Reading `check` from a script, one field matters more than the rest.** When drift is found the stored baseline is rewritten, so the next run reports clean whether or not anything was fixed. `fingerprintUpdated` is true on exactly the run that carried the signal. A caller that polls and ignores it will see drift once and never again.

When drift is real, the fix is a new IR. Where that comes from depends on how the current one was made, and `check` says which.

## Authentication

The IR names where a secret lives, never the secret. `secretRef` points at an environment variable, a file under the site config directory, or a value acquired at runtime. An IR is safe to commit; that property only holds because nothing resolves a secret at authoring time.

Four modes are modeled: none, API key, OAuth2, and a browser-bootstrapped token. The last one covers a token only the target's own browser session can mint, captured once through `surfacer auth login` and replayed headless until it expires. Its renewal strategy is to prompt a human, because no automated re-acquisition exists for a credential that requires a login.

## Boundaries

**Map a surface you are entitled to use.** The same boundary `surface-recon` states: a service you have access to, documentation you may read. An authentication wall you were not given is the edge.

**An emitted interface inherits the target's stability, not the compiler's.** An undocumented endpoint can break on any deploy. That is what `check` is for, and it is why the IR is worth keeping.
