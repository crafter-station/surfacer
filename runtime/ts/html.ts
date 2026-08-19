// Minimal HTML parser for the surfacer TypeScript runtime.
//
// Why this exists at all: the Rust path parses with `scraper` (html5ever), and
// the emitted TS binary cannot link it. It also cannot depend on an npm parser,
// because an npm import breaks the standalone-binary promise the ts-cli target
// makes. So the runtime carries the smallest parser that reproduces the field
// values the Rust path produces for the descriptors surfacer actually emits.
//
// What this is NOT: an html5ever-equivalent tree builder. It does not implement
// the HTML5 insertion modes, so it does not synthesize an implied <tbody>, does
// not reconstruct active formatting elements, and does not recover from
// arbitrary misnesting the way a browser does. Those gaps are listed in
// runtime/ts/README.md next to the measurement that shows they do not change
// the emitted descriptors' output.

/** Node kinds. A number rather than a string union so the discriminant check
 *  is an integer compare in the compiled binary. */
export const KIND_TEXT = 0;
export const KIND_ELEMENT = 1;

/**
 * A node in the parsed tree: either an element or a text node.
 *
 * WHY ONE STRUCT INSTEAD OF `Element | TextNode`
 *
 * The natural TypeScript shape here is a discriminated union of two interfaces
 * with an `is` type guard. That shape parses correctly under Bun and segfaults
 * (exit 139) in a scriptc-compiled binary as soon as a function recurses
 * through the union. Reduced to 20 lines: an interface holding an array of
 * `El | Tx`, a `n is El` guard, and a recursive count over the children is
 * enough to reproduce it, at a tree depth of 8, while the identical program
 * with one struct and a numeric tag returns the right answer.
 *
 * Since the emitted CLI ships as a native binary, a shape that only works under
 * a JS runtime is not usable here. One struct, one integer discriminant, no
 * type guard: text nodes carry an empty `tag` and elements carry an empty
 * `text`, which costs two unused fields per node and keeps the tree walkable in
 * compiled form.
 */
export interface Element {
  kind: number;
  tag: string;
  text: string;
  attrs: Map<string, string>;
  children: Element[];
  parent: Element | undefined;
  /** Position among the parent's element children, 0-based. Used by the
   *  positional pseudo-classes, which would otherwise need a parent scan. */
  indexInParent: number;
}

/** Retained as the name the rest of the runtime reads, now one struct. */
export type Node = Element;
export type TextNode = Element;

export function isElement(node: Element): boolean {
  return node.kind === KIND_ELEMENT;
}

// Elements that never have a closing tag. A parser that pushed these on the
// open-element stack would nest every following sibling inside them, which is
// how a naive parser turns a flat row of cells into a staircase.
const VOID_TAGS = new Set<string>([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

// Elements whose content is raw text: a `<` inside them starts no tag. Parsing
// their bodies as markup is how a stray `if (a < b)` in a script swallows the
// rest of the document.
const RAW_TEXT_TAGS = new Set<string>(["script", "style"]);

// Tags that close an open peer when a new one opens. HN's markup leaves <td>
// and <tr> unclosed in places, and without this the cells nest inside each
// other, which breaks `td:first-child` / `td:last-child` for every row.
//
// Stored as space-delimited strings rather than a Map of Sets: scriptc has no
// Map slot for a Set value (SC1090/SC2009), and a whole-word scan over a short
// string answers the same question with a static lowering.
const IMPLIED_END: Map<string, string> = new Map<string, string>([
  ["li", " li "],
  ["p", " p "],
  ["td", " td th "],
  ["th", " td th "],
  ["tr", " tr td th "],
  ["option", " option "],
  ["thead", " thead tbody tfoot "],
  ["tbody", " thead tbody tfoot "],
  ["tfoot", " thead tbody tfoot "],
]);

/** True when a space-delimited list contains the tag as a whole word. */
function listHas(list: string, tag: string): boolean {
  return list.indexOf(" " + tag + " ") >= 0;
}

const NAMED_ENTITIES: Map<string, string> = new Map<string, string>([
  ["amp", "&"],
  ["lt", "<"],
  ["gt", ">"],
  ["quot", '"'],
  ["apos", "'"],
  ["nbsp", " "],
  ["#39", "'"],
  ["#x27", "'"],
  ["#x2F", "/"],
  ["#47", "/"],
]);

/**
 * Decode the entities that appear in attribute values and text.
 *
 * Numeric escapes are decoded generally; named ones come from the table above.
 * An unknown entity is left verbatim rather than dropped, so a literal "&" in
 * content survives instead of silently eating the text that follows it.
 */
export function decodeEntities(input: string): string {
  if (input.indexOf("&") < 0) return input;

  let out = "";
  let i = 0;
  while (i < input.length) {
    const ch = input[i];
    if (ch !== "&") {
      out = out + ch;
      i = i + 1;
      continue;
    }
    const semi = input.indexOf(";", i + 1);
    // A bare "&" with no terminator is content, not a broken entity.
    if (semi < 0 || semi - i > 12) {
      out = out + ch;
      i = i + 1;
      continue;
    }
    const body = input.slice(i + 1, semi);
    const named = NAMED_ENTITIES.get(body);
    if (named !== undefined) {
      out = out + named;
      i = semi + 1;
      continue;
    }
    if (body.length > 1 && body[0] === "#") {
      const isHex = body[1] === "x" || body[1] === "X";
      const digits = isHex ? body.slice(2) : body.slice(1);
      const code = isHex ? parseInt(digits, 16) : parseInt(digits, 10);
      if (!Number.isNaN(code) && code > 0 && code <= 0x10ffff) {
        out = out + codePointToString(code);
        i = semi + 1;
        continue;
      }
    }
    out = out + ch;
    i = i + 1;
  }
  return out;
}

/**
 * A code point as a string, spelled with fromCharCode and an explicit surrogate
 * pair because `String.fromCodePoint` has no scriptc lowering yet (SC2020).
 * Astral characters reach here through numeric entities such as `&#128512;`.
 */
function codePointToString(code: number): string {
  if (code <= 0xffff) return String.fromCharCode(code);
  const offset = code - 0x10000;
  const high = 0xd800 + (offset >> 10);
  const low = 0xdc00 + (offset & 0x3ff);
  return String.fromCharCode(high) + String.fromCharCode(low);
}

function makeElement(tag: string, parent: Element | undefined): Element {
  return {
    kind: KIND_ELEMENT,
    tag: tag,
    text: "",
    attrs: new Map<string, string>(),
    children: [],
    parent: parent,
    indexInParent: 0,
  };
}

function makeText(text: string, parent: Element | undefined): Element {
  return {
    kind: KIND_TEXT,
    tag: "",
    text: text,
    attrs: new Map<string, string>(),
    children: [],
    parent: parent,
    indexInParent: 0,
  };
}

function appendChild(parent: Element, child: Element): void {
  if (child.kind === KIND_ELEMENT) {
    let count = 0;
    for (const existing of parent.children) {
      if (existing.kind === KIND_ELEMENT) count = count + 1;
    }
    child.indexInParent = count;
  }
  parent.children.push(child);
}

/** Parse an attribute list starting after the tag name. */
function parseAttributes(source: string, start: number, end: number, into: Element): void {
  let i = start;
  while (i < end) {
    while (i < end && isSpace(source[i])) i = i + 1;
    if (i >= end) break;
    if (source[i] === "/") {
      i = i + 1;
      continue;
    }

    let nameStart = i;
    while (i < end && !isSpace(source[i]) && source[i] !== "=" && source[i] !== "/") {
      i = i + 1;
    }
    const name = source.slice(nameStart, i).toLowerCase();
    if (name === "") {
      i = i + 1;
      continue;
    }

    while (i < end && isSpace(source[i])) i = i + 1;
    if (i >= end || source[i] !== "=") {
      // A valueless attribute (`disabled`) is present with an empty value, so
      // `[disabled]` matches it while `[disabled="x"]` does not.
      into.attrs.set(name, "");
      continue;
    }

    i = i + 1;
    while (i < end && isSpace(source[i])) i = i + 1;
    if (i >= end) {
      into.attrs.set(name, "");
      break;
    }

    const quote = source[i];
    let value = "";
    if (quote === '"' || quote === "'") {
      i = i + 1;
      const close = source.indexOf(quote, i);
      const stop = close < 0 || close > end ? end : close;
      value = source.slice(i, stop);
      i = stop + 1;
    } else {
      const valueStart = i;
      while (i < end && !isSpace(source[i])) i = i + 1;
      value = source.slice(valueStart, i);
    }
    into.attrs.set(name, decodeEntities(value));
  }
}

function isSpace(ch: string): boolean {
  return ch === " " || ch === "\t" || ch === "\n" || ch === "\r" || ch === "\f";
}

/** `haystack.startsWith(needle, at)`, spelled so scriptc can lower it.
 *  The two-argument form of startsWith has no static lowering (SC2020), and a
 *  slice comparison is the same test with one allocation. */
function startsAt(haystack: string, needle: string, at: number): boolean {
  return haystack.slice(at, at + needle.length) === needle;
}

/**
 * Parse an HTML document into an element tree rooted at a synthetic node.
 *
 * The root is synthetic (tag `#document`) so that a document with several
 * top-level elements, or with content before <html>, still has one parent to
 * select from.
 */
export function parseDocument(html: string): Element {
  const root = makeElement("#document", undefined);
  let current = root;
  let i = 0;

  while (i < html.length) {
    const lt = html.indexOf("<", i);
    if (lt < 0) {
      pushText(current, html.slice(i));
      break;
    }
    if (lt > i) {
      pushText(current, html.slice(i, lt));
    }

    // Comments and doctype/CDATA carry no selectable structure.
    if (startsAt(html, "<!--", lt)) {
      const close = html.indexOf("-->", lt + 4);
      i = close < 0 ? html.length : close + 3;
      continue;
    }
    if (startsAt(html, "<!", lt) || startsAt(html, "<?", lt)) {
      const close = html.indexOf(">", lt + 2);
      i = close < 0 ? html.length : close + 1;
      continue;
    }

    const isClosing = html[lt + 1] === "/";
    const gt = findTagEnd(html, lt);
    if (gt < 0) {
      pushText(current, html.slice(lt));
      break;
    }

    if (isClosing) {
      const name = html.slice(lt + 2, gt).trim().toLowerCase();
      // Walk up to the matching open element. If the tag was never opened the
      // stack is left alone, which keeps stray `</div>` from unwinding the tree.
      let probe: Element | undefined = current;
      while (probe !== undefined && probe !== root) {
        if (probe.tag === name) {
          current = probe.parent !== undefined ? probe.parent : root;
          break;
        }
        probe = probe.parent;
      }
      i = gt + 1;
      continue;
    }

    const inner = html.slice(lt + 1, gt);
    let nameEnd = 0;
    while (
      nameEnd < inner.length &&
      !isSpace(inner[nameEnd]) &&
      inner[nameEnd] !== "/"
    ) {
      nameEnd = nameEnd + 1;
    }
    const tag = inner.slice(0, nameEnd).toLowerCase();
    if (tag === "") {
      i = gt + 1;
      continue;
    }

    // Close any peer this tag implicitly ends before opening it.
    const closes = IMPLIED_END.get(tag);
    if (closes !== undefined) {
      while (current !== root && listHas(closes, current.tag)) {
        current = current.parent !== undefined ? current.parent : root;
      }
    }

    const element = makeElement(tag, current);
    parseAttributes(inner, nameEnd, inner.length, element);
    appendChild(current, element);

    const selfClosing = inner.endsWith("/");
    if (VOID_TAGS.has(tag) || selfClosing) {
      i = gt + 1;
      continue;
    }

    if (RAW_TEXT_TAGS.has(tag)) {
      const closeTag = "</" + tag;
      const close = indexOfIgnoreCase(html, closeTag, gt + 1);
      if (close < 0) {
        i = html.length;
      } else {
        const closeEnd = html.indexOf(">", close);
        i = closeEnd < 0 ? html.length : closeEnd + 1;
      }
      continue;
    }

    current = element;
    i = gt + 1;
  }

  return root;
}

/**
 * Find the `>` that ends a tag, skipping any inside a quoted attribute value.
 *
 * HN inlines JavaScript in `onclick` handlers containing `>`, and stopping at
 * the first `>` there truncates the tag and orphans the rest of the row.
 */
function findTagEnd(html: string, from: number): number {
  let i = from + 1;
  let quote = "";
  while (i < html.length) {
    const ch = html[i];
    if (quote !== "") {
      if (ch === quote) quote = "";
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === ">") {
      return i;
    }
    i = i + 1;
  }
  return -1;
}

function indexOfIgnoreCase(haystack: string, needle: string, from: number): number {
  return haystack.toLowerCase().indexOf(needle.toLowerCase(), from);
}

function pushText(parent: Element, raw: string): void {
  if (raw === "") return;
  parent.children.push(makeText(decodeEntities(raw), parent));
}

/**
 * All text under an element, concatenated in document order.
 *
 * This mirrors `ElementRef::text()` in scraper, which yields the text nodes of
 * the subtree with no separator inserted between them.
 */
export function textOf(node: Element): string {
  let out = "";
  for (const child of node.children) {
    if (child.kind === KIND_ELEMENT) {
      out = out + textOf(child);
    } else {
      out = out + child.text;
    }
  }
  return out;
}

/** Element children only, which is what the CSS combinators walk. */
export function elementChildren(node: Element): Element[] {
  const out: Element[] = [];
  for (const child of node.children) {
    if (child.kind === KIND_ELEMENT) out.push(child);
  }
  return out;
}

/** Every element in the subtree, in document order, excluding `node` itself. */
export function descendants(node: Element): Element[] {
  const out: Element[] = [];
  collectDescendants(node, out);
  return out;
}

function collectDescendants(node: Element, out: Element[]): void {
  for (const child of node.children) {
    if (child.kind !== KIND_ELEMENT) continue;
    out.push(child);
    collectDescendants(child, out);
  }
}
