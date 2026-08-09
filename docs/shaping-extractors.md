---
type: shaping
project: surfacer
feature: extractors
created: 2026-04-14
status: shaping-active
depends_on: surfacer-ir SiteDescriptor, surfacer recon --auto, surfacer exec
---

# Extractors — Shaping

> How surfacer turns raw HTML responses into structured, navigable data.
> Without extractors, `surfacer exec` dumps text. With extractors,
> it returns items you can pipe, filter, and drill into.

## Problem

`news-ycombinator-com news` currently returns this:

```
Hacker News new | past | comments | ask | show | jobs | submit login
1. Claude Code Routines  (claude.com)
374 points by matthieu_bl 7 hours ago | hide | 238 comments
2. Rare concert recordings...
```

Raw text. No structure. No links. No way to say "open story #1" or
"show me stories with >500 points" or pipe to jq. The output is
functionally equivalent to curl — surfacer adds no value over reading
the HTML directly.

## Outcome

`news-ycombinator-com news` should return this:

### Human mode (default)
```
  Hacker News — Front Page

   1  Claude Code Routines                                374 pts
      claude.com · matthieu_bl · 7h · 238 comments

   2  Rare concert recordings on Internet Archive         497 pts
      techcrunch.com · jrm-veris · 10h · 150 comments

   3  Fuck the cloud (2009)                               55 pts
      textfiles.com · downbad_ · 2h · 26 comments

  Showing 1-10 of 30 · next: news --page 2

  Drill down:
    news-ycombinator-com open 1        Open story #1 in browser
    news-ycombinator-com comments 1    View comments for story #1
    news-ycombinator-com news --page 2 Next page
    news-ycombinator-com news --json   Machine-readable output
```

### Agent mode (--json)
```json
{
  "items": [
    {
      "index": 1,
      "title": "Claude Code Routines",
      "url": "https://code.claude.com/docs/en/routines",
      "domain": "claude.com",
      "points": 374,
      "author": "matthieu_bl",
      "age": "7 hours ago",
      "comments": 238,
      "commentsUrl": "https://news.ycombinator.com/item?id=47768133"
    }
  ],
  "page": 1,
  "totalItems": 30,
  "nextPage": "news --page 2"
}
```

Each item is a first-class object with typed fields. The output is
pipeable (`| jq '.items[] | select(.points > 500)'`), navigable
(`open 1` drills into story #1), and paginated.

## Requirements (R)

| ID | Requirement | Status |
|----|-------------|--------|
| EX-R0 | Operations in the IR can optionally have an `extractor` field that describes how to parse the response into structured items | Must-have |
| EX-R1 | Extractors are defined once during recon and reused at runtime with zero LLM calls | Must-have |
| EX-R2 | The recon agent auto-generates extractors using heuristics + one LLM call per endpoint to name fields | Must-have |
| EX-R3 | Extractors work on HTML responses (the common case for sites without APIs) | Must-have |
| EX-R4 | Extractors work on JSON responses (for REST APIs that return structured data) | Must-have |
| EX-R5 | Human output renders items as a formatted list with index numbers, key fields inline, and drill-down hints | Must-have |
| EX-R6 | JSON output returns items as a typed array with named fields | Must-have |
| EX-R7 | Items with URLs are navigable via `<site> open <index>` which opens in the browser | Must-have |
| EX-R8 | Pagination is supported via `--page N` flag when the extractor detects multiple pages | Nice-to-have |
| EX-R9 | The extractor schema in the IR is human-readable and editable by maintainers | Must-have |
| EX-R10 | Extractors degrade gracefully: if extraction fails, fall back to raw text output (current behavior) | Must-have |
| EX-R11 | Maintainers can manually edit extractors after recon to fix field names or selectors | Must-have |

## IR Schema Extension

### OperationDescriptor gets an optional `extractor` field

```rust
pub struct OperationDescriptor {
    pub command_path: Vec<String>,
    pub summary: String,
    pub description: String,
    pub operation_kind: OperationKind,
    pub transport: OperationTransport,
    pub extractor: Option<Extractor>,  // NEW
}
```

### The Extractor enum

```rust
#[serde(tag = "type")]
pub enum Extractor {
    List(ListExtractor),
    Detail(DetailExtractor),
    Raw,  // explicit "no extraction, return text"
}
```

Two primary types:

**ListExtractor** — for pages that show a list of items (HN front page, search results, inbox, tables):

```rust
pub struct ListExtractor {
    pub item_pattern: ItemPattern,
    pub fields: Vec<FieldDef>,
    pub pagination: Option<PaginationDef>,
}

pub struct ItemPattern {
    pub selector: String,        // CSS-like: "tr.athing" or heuristic tag
    pub strategy: PatternStrategy,
}

pub enum PatternStrategy {
    CssSelector,     // use the selector as CSS
    RepeatingBlock,  // auto-detect repeating DOM structures
    TableRows,       // <table> with <tr> items
    ListItems,       // <ul>/<ol> with <li> items
}

pub struct FieldDef {
    pub name: String,          // "title", "url", "points", "author"
    pub field_type: FieldType,
    pub selector: String,      // relative to item: "a.storylink"
    pub attribute: Option<String>, // "href", "textContent" (default)
}

pub enum FieldType {
    Text,
    Url,
    Number,
    DateTime,
}

pub struct PaginationDef {
    pub next_selector: String,   // CSS selector for "next page" link
    pub page_param: String,      // query param name: "p", "page"
}
```

**DetailExtractor** — for single-item pages (story detail, profile, receipt):

```rust
pub struct DetailExtractor {
    pub fields: Vec<FieldDef>,
}
```

### JSON serialization example

In the IR file:

```json
{
  "commandPath": ["news"],
  "description": "Front page stories",
  "operationKind": "read",
  "transport": { "kind": "http", "endpointIndex": 7 },
  "extractor": {
    "type": "list",
    "itemPattern": {
      "selector": "tr.athing",
      "strategy": "repeatingBlock"
    },
    "fields": [
      { "name": "title", "fieldType": "text", "selector": "a", "attribute": null },
      { "name": "url", "fieldType": "url", "selector": "a", "attribute": "href" },
      { "name": "domain", "fieldType": "text", "selector": "span.sitebit a", "attribute": null },
      { "name": "points", "fieldType": "number", "selector": "+tr span.score", "attribute": null },
      { "name": "author", "fieldType": "text", "selector": "+tr a.hnuser", "attribute": null },
      { "name": "age", "fieldType": "text", "selector": "+tr span.age a", "attribute": null },
      { "name": "commentsUrl", "fieldType": "url", "selector": "+tr a:last-child", "attribute": "href" }
    ],
    "pagination": {
      "nextSelector": "a.morelink",
      "pageParam": "p"
    }
  }
}
```

### Why CSS selectors + a field naming LLM call (Opción 3 from discussion)

The heuristic side detects:
- Repeating DOM structures (same tag/class pattern appearing N times)
- Table rows
- List items
- Repeating divs with same class

The LLM call (one per endpoint, during recon only) receives:
- The first 3 items of the detected repeating pattern (raw HTML)
- The field selectors that the heuristic found
- Prompt: "name these fields semantically (title, url, price, author, date, etc.)"

The LLM response is a JSON mapping: `{"field_0": "title", "field_1": "url", ...}`.
This costs ~500 tokens per endpoint. For HN with 11 endpoints, that's ~5500 tokens total
during recon. Zero tokens at runtime.

## Shape: Extraction Pipeline

```
surfacer exec <site> <command>
  ↓
load IR → find operation → check extractor
  ↓
[has extractor?]
  ↓ yes                        ↓ no
  fetch URL via defuddle       fetch URL via defuddle
  ↓                            ↓
  parse HTML into DOM          return raw text (current behavior)
  ↓
  apply ItemPattern to find items
  ↓
  for each item: extract fields via FieldDefs
  ↓
  build Vec<ExtractedItem>
  ↓
  [--json?]
    ↓ yes                      ↓ no
    serialize items as JSON    render human list with indices
                                + drill-down hints
```

### New commands enabled by extractors

Once items are extracted with indices, three new virtual commands become available:

```bash
<site> open <index>       # opens item URL in browser
<site> open <index> --json  # returns single item as JSON
<site> <command> --page N  # pagination
```

These don't need their own IR operations — they're derived from the extractor at runtime.

## Recon Integration

During `surfacer recon --auto`:

```
for each endpoint discovered:
  1. fetch the page (already done for HAR)
  2. heuristic: detect repeating patterns in the DOM
  3. if pattern found:
     a. extract first 3 items as sample HTML
     b. LLM call: "name these fields" → field names
     c. build ListExtractor with selectors + field names
     d. attach to OperationDescriptor.extractor
  4. if no pattern found:
     a. set extractor = None (raw text fallback)
```

The LLM call is the ONLY place in the entire pipeline where an LLM
is invoked. It happens during recon (maintainer pays once), not during
exec (consumer pays never). Consistent with the thesis.

## Implementation plan

### Phase 1 — IR schema + types (small)
- Add `Extractor`, `ListExtractor`, `DetailExtractor`, `FieldDef`, etc. to `surfacer-ir`
- Add `extractor: Option<Extractor>` to `OperationDescriptor`
- Serde derives, tests for roundtrip

### Phase 2 — Runtime extraction (medium)
- Add HTML DOM parser to `surfacer-app` (probably `scraper` crate for CSS selectors)
- Implement `extract_items(html: &str, extractor: &ListExtractor) -> Vec<ExtractedItem>`
- Update `exec_with_ir` to use extractor when present
- Human output renderer for extracted items (indexed list with fields)
- JSON output renderer for extracted items
- `open <index>` command to open item URL in browser

### Phase 3 — Recon auto-extraction (medium-large)
- Add DOM pattern detection to `surfacer-probe` or `surfacer-classifier`
- Heuristics: repeating elements, table rows, list items
- LLM field naming call via `claude -p` during recon
- Wire into auto-recon loop: after fetching each page, run extraction detection
- Save extractors in IR

### Phase 4 — Polish
- Pagination support
- Detail pages (single-item extraction)
- Maintainer editing of extractors post-recon
- Extractor validation in `surfacer lint`

## Dependencies

- `scraper` crate (CSS selector engine for Rust) — well-maintained, used by many
- `claude` CLI for the LLM field naming call (already available)
- No new external services

## Non-goals

- Not building a general-purpose web scraper (Apify, Scrapy territory)
- Not handling infinite scroll / JS-rendered content in v1 (agent-browser + eval would be v2)
- Not supporting complex nested extractors (items within items) in v1
- Not building a visual extractor builder UI in v1

## Decision needed

**D1 — CSS selectors vs accessibility tree for item detection?**

CSS selectors work on the raw HTML. AX tree works on the rendered DOM.
For sites like HN (HTML table), CSS selectors are straightforward.
For SPAs that render via JS, CSS selectors on the raw HTML won't work
(the HTML is empty before JS hydration).

Options:
- (a) CSS selectors on defuddle output (works for HTML sites, fails for JS SPAs)
- (b) AX tree pattern detection via agent-browser snapshot (works for all sites, more complex)
- (c) CSS selectors first, AX tree fallback (covers both cases)

Recommendation: **(a) for v1**, upgrade to (c) later. The sites surfacer targets
(government portals, legacy banking, SaaS without APIs) are overwhelmingly
server-rendered HTML. JS SPAs are the OpenAI dashboard case which is a
different archetype and can wait for v2.

**D2 — Which LLM for field naming?**

Options:
- (a) Claude via `claude -p --model haiku` — cheapest, fast, good enough for naming 5-7 fields
- (b) Claude via `claude -p --model sonnet` — better understanding, still cheap for one call
- (c) No LLM — heuristic field naming from HTML attributes (class names, input labels)

Recommendation: **(a) haiku**. Field naming is the simplest LLM task possible
("here are 3 HTML snippets, name these 5 fields"). Haiku handles this trivially.
One call per endpoint during recon, ~500 tokens each. Total cost for HN (11 endpoints):
~$0.005. Even with 50 endpoints, it's under $0.03.

**D3 — Pagination: query param rewrite or full re-fetch?**

When user does `news --page 2`:
- (a) Rewrite the URL with `?p=2` and re-fetch via defuddle
- (b) Follow the "next" link from the page via agent-browser

Recommendation: **(a) query param rewrite**. Simpler, deterministic,
works for most sites. Falls back to (b) for sites with opaque pagination
(cursor-based, POST-based).
