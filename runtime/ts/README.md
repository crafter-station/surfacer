# surfacer TypeScript runtime

The execution logic the standalone emitters carry. Written once per language,
not once per emitter.

## Why it exists

Generic behavior used to live only in Rust. `crates/surfacer-app/src/extract.rs`
applies an IR's extractor to HTML, and `execute.rs` shapes the result. The shim
target inherited both by shelling out to `surfacer exec`, so it got the
extractor for free. The standalone targets inherited neither.

Measured on the committed HN descriptor, same IR and same operation, before this
runtime existed:

| path | `news` returns |
|---|---|
| shim (`surfacer exec ... --json`) | 30 items with `title`, `domain`, `rank` |
| emitted ts-cli (`hn news`) | raw HTML |
| emitted mcp | raw HTML, and no parameter check at all |

That contradicted the README's claim that "every command gets the same flag
handling, the same JSON output, because one emitter wrote all of them".

There were two undeclared classes of emitter: **delegating** ones (the shim,
which requires surfacer installed and inherits everything) and **standalone**
ones (ts-cli and mcp, which depend on nothing and therefore have to carry the
logic themselves). This directory is what the standalone ones carry.

## Layout

| file | what it holds |
|---|---|
| `html.ts` | HTML parser producing a selectable tree |
| `css.ts` | CSS selector matching over that tree |
| `normalize.ts` | the content cleanup the IR's selectors assume, plus `htmlToText` |
| `extract.ts` | the port of `extract.rs`, and the JSON envelope `execute.rs` writes |
| `params.ts` | the observed-parameter rule, shared by every target |
| `exec.ts` | the `ExecResult` shape for the unstructured fallback |
| `index.ts` | re-exports, for reading the runtime as a module |

The emitter inlines these (see `crates/surfacer-emit-cli/src/runtime_ts.rs`), so
editing a copy inside an emitted file accomplishes nothing: regenerate instead.

## What is lost relative to the Rust path

These are measured, not assumed. Each one is a real difference a caller can
observe.

### 1. No defuddle, so normalization is reimplemented and narrower

`surfacer exec` runs `defuddle parse <url> --json` and extracts from the cleaned
`content`. Recon authors the IR's selectors while looking at that output, so the
selectors encode defuddle's structure, not the raw page's.

A standalone binary cannot call defuddle: it is an npm program, and depending on
it voids the promise that the binary needs nothing installed. So `normalize.ts`
reimplements the part of the cleanup that changes what a selector resolves to.

Measured on the HN front page, same selectors through the same Rust extractor:

```
defuddle content -> 30 rows, title "A 3D fruit fly on macOS desktop ...",
                    url "https://github.com/DenisSergeevitch/desktop-fly"
raw fetched HTML -> 30 rows, title "", url "vote?id=49353221&how=up&goto=news"
```

The raw page keeps an upvote anchor inside the votelinks cell, so
`td:last-child a:first-of-type` resolves to the vote link. Defuddle empties that
cell because the anchor wraps a `div` with no text. Dropping text-empty anchors
and unwrapping `div` reproduces the correct values, which the golden test checks.

**What is lost:** defuddle's readability scoring, its boilerplate removal, and
its `wordCount`. A descriptor whose selectors depend on some other defuddle
transform will extract differently here. The golden test covers the committed
descriptor; a new descriptor that relies on a different transform would need
its own fixture to prove the two paths still agree.

### 2. `wordCount` is computed, not inherited

On the Rust path it comes from defuddle, computed over the page defuddle
cleaned. Here it is counted from the text this runtime produced. The two numbers
describe different inputs and are not expected to match. The golden test names
this as an expected difference rather than asserting equality, which would force
one side to fake the other's number.

### 3. AX-tree extraction yields nothing, on both paths

An `axTree` field source needs an accessibility tree, which a browser produces.
This runtime has no browser.

Worth being precise: this is not a gap versus Rust. `extract_field` in
`extract.rs` returns `ExtractedValue::Missing` for every `FieldSource::AxTree`,
and `extract_list` reads only `css_selector`. Both paths yield nothing for an
AX-only descriptor. The difference is that this runtime reports a named reason
(`noCssSelector`) on stderr instead of degrading silently.

### 4. The CSS engine covers a subset of Selectors Level 4

`scraper` implements the full grammar. This engine implements what recon writes:
tag, id, class, attribute (all six operators), the descendant, child, adjacent
and sibling combinators, `:has`, `:not`, `:first-child`, `:last-child`,
`:only-child`, `:first-of-type`, `:last-of-type`, `:nth-child`, `:nth-of-type`,
`:empty`, and `:root`.

Anything else raises `UnsupportedSelectorError`, which the extractor turns into
a named failure. It never silently matches nothing, because "no items" and "I
could not read that selector" are different facts and a caller has to be able
to tell them apart.

### 5. The HTML parser is not a spec tree builder

It handles void elements, raw-text elements, quoted and unquoted attributes,
implied end tags for table and list elements, entity decoding, and a `>` inside
a quoted attribute value. It does not implement the HTML5 insertion modes: no
implied `tbody`, no active formatting element reconstruction, no adoption
agency. Malformed markup that a browser silently repairs may parse differently
here.

### 6. No colour in human output

`format_extracted_human` colours through `owo_colors` and checks `NO_COLOR` plus
a tty. Reproducing that would be a second implementation of a decision that has
no bearing on what a command means, so the emitted CLI prints plain text.

## Constraints the compiled target imposes

The emitted ts-cli compiles to a native binary with `scriptc`, which is stricter
than a JS runtime. Three constraints shaped this code, each found by running a
compiled binary rather than by reading docs:

1. **No discriminated union with a type guard across a recursive walk.** An
   `Element | TextNode` union with an `is` guard parses fine under Bun and
   segfaults (exit 139) in a compiled binary as soon as a function recurses
   through it, at a tree depth of 8. `html.ts` therefore uses one struct with a
   numeric `kind` field.

2. **Record types must match exactly.** An optional or nullable field
   (`attribute?: string | null`) makes two literals infer different types, and
   an array of them stops unifying (SC2002/SC2003). The IR's `Option<String>`
   becomes `""` at the emitter boundary, and every field is always written.

3. **A missing record key traps.** Reading an absent key returns `undefined`
   under a JS runtime; a compiled binary has no `undefined` for a typed slot and
   raises "record has no key". Every field read goes through `hasField` first,
   and absence reads as `""` or `NaN`.

## Running the runtime's own checks

```bash
bun test runtime/ts            # unit tests for the parser, selectors, extractor
cargo test -p surfacer-emit-cli --test cross_emitter_golden
```

The golden test is the one that matters: it runs the Rust extractor and this
runtime over fixtures captured from the same fetch, and fails if they disagree.
