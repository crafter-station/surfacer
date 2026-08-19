// CSS selector matching for the surfacer TypeScript runtime.
//
// The Rust path compiles selectors with `scraper`, which implements the full
// Selectors Level 4 grammar. This engine covers the subset that recon actually
// writes into an IR, and it reports an unsupported selector instead of silently
// matching nothing. That distinction matters: a selector that quietly matches
// nothing degrades to "no items found" and the caller sees raw content with no
// reason, which is the exact failure mode this runtime exists to remove.

import {
  Element,
  descendants,
  elementChildren,
  isElement,
  textOf,
} from "./html";

export type AttrOp = "exists" | "equals" | "prefix" | "suffix" | "contains" | "dash" | "word";

export interface AttrTest {
  name: string;
  op: AttrOp;
  value: string;
}

export interface PseudoTest {
  name: string;
  /** The argument of a functional pseudo-class, already parsed when it is a
   *  selector (`:has`, `:not`), otherwise the raw text (`:nth-child`). */
  selector: SelectorList | undefined;
  raw: string;
}

export interface CompoundSelector {
  tag: string | undefined;
  ids: string[];
  classes: string[];
  attrs: AttrTest[];
  pseudos: PseudoTest[];
}

export type Combinator = "descendant" | "child" | "adjacent" | "sibling";

export interface ComplexPart {
  combinator: Combinator;
  compound: CompoundSelector;
}

/** A complex selector: a first compound, then combinator-joined compounds.
 *
 *  `leading` is set for a relative selector (`> td`, `+ tr`), which appears
 *  inside `:has()`. There the selector is anchored at the subject rather than
 *  at the document, so the combinator before the first compound is part of the
 *  match rather than a parse error. */
export interface ComplexSelector {
  first: CompoundSelector;
  rest: ComplexPart[];
  leading: Combinator | undefined;
}

export type SelectorList = ComplexSelector[];

/** Raised for a selector this engine does not implement. Callers turn it into
 *  a named reason rather than an empty match set. */
export class UnsupportedSelectorError extends Error {}

const SUPPORTED_PSEUDOS = new Set<string>([
  "has",
  "not",
  "first-child",
  "last-child",
  "only-child",
  "first-of-type",
  "last-of-type",
  "nth-child",
  "nth-of-type",
  "empty",
  "root",
]);

interface Cursor {
  source: string;
  pos: number;
}

function peek(c: Cursor): string {
  return c.pos < c.source.length ? c.source[c.pos] : "";
}

function skipSpace(c: Cursor): boolean {
  const start = c.pos;
  while (c.pos < c.source.length) {
    const ch = c.source[c.pos];
    if (ch === " " || ch === "\t" || ch === "\n" || ch === "\r" || ch === "\f") {
      c.pos = c.pos + 1;
      continue;
    }
    break;
  }
  return c.pos > start;
}

function isIdentChar(ch: string): boolean {
  if (ch === "") return false;
  return (
    (ch >= "a" && ch <= "z") ||
    (ch >= "A" && ch <= "Z") ||
    (ch >= "0" && ch <= "9") ||
    ch === "-" ||
    ch === "_" ||
    ch === "\\" ||
    ch.charCodeAt(0) > 127
  );
}

function readIdent(c: Cursor): string {
  let out = "";
  while (c.pos < c.source.length) {
    const ch = c.source[c.pos];
    if (ch === "\\") {
      // An escaped character is taken verbatim, which is how a selector
      // addresses a class containing a colon or a slash.
      if (c.pos + 1 < c.source.length) {
        out = out + c.source[c.pos + 1];
        c.pos = c.pos + 2;
        continue;
      }
      c.pos = c.pos + 1;
      continue;
    }
    if (!isIdentChar(ch)) break;
    out = out + ch;
    c.pos = c.pos + 1;
  }
  return out;
}

function readString(c: Cursor): string {
  const quote = c.source[c.pos];
  c.pos = c.pos + 1;
  let out = "";
  while (c.pos < c.source.length) {
    const ch = c.source[c.pos];
    if (ch === "\\" && c.pos + 1 < c.source.length) {
      out = out + c.source[c.pos + 1];
      c.pos = c.pos + 2;
      continue;
    }
    if (ch === quote) {
      c.pos = c.pos + 1;
      return out;
    }
    out = out + ch;
    c.pos = c.pos + 1;
  }
  throw new UnsupportedSelectorError("unterminated string in selector");
}

/** Read a balanced parenthesized argument, honoring nesting and strings. */
function readParenArg(c: Cursor): string {
  if (peek(c) !== "(") throw new UnsupportedSelectorError("expected (");
  c.pos = c.pos + 1;
  let depth = 1;
  let out = "";
  while (c.pos < c.source.length) {
    const ch = c.source[c.pos];
    if (ch === '"' || ch === "'") {
      const quote = ch;
      out = out + ch;
      c.pos = c.pos + 1;
      while (c.pos < c.source.length) {
        const inner = c.source[c.pos];
        out = out + inner;
        c.pos = c.pos + 1;
        if (inner === "\\" && c.pos < c.source.length) {
          out = out + c.source[c.pos];
          c.pos = c.pos + 1;
          continue;
        }
        if (inner === quote) break;
      }
      continue;
    }
    if (ch === "(") depth = depth + 1;
    if (ch === ")") {
      depth = depth - 1;
      if (depth === 0) {
        c.pos = c.pos + 1;
        return out;
      }
    }
    out = out + ch;
    c.pos = c.pos + 1;
  }
  throw new UnsupportedSelectorError("unbalanced ( in selector");
}

function parseCompound(c: Cursor): CompoundSelector {
  const compound: CompoundSelector = {
    tag: undefined,
    ids: [],
    classes: [],
    attrs: [],
    pseudos: [],
  };
  let matched = false;

  while (c.pos < c.source.length) {
    const ch = peek(c);

    if (ch === "*") {
      c.pos = c.pos + 1;
      matched = true;
      continue;
    }
    if (ch === "#") {
      c.pos = c.pos + 1;
      compound.ids.push(readIdent(c));
      matched = true;
      continue;
    }
    if (ch === ".") {
      c.pos = c.pos + 1;
      compound.classes.push(readIdent(c));
      matched = true;
      continue;
    }
    if (ch === "[") {
      c.pos = c.pos + 1;
      compound.attrs.push(parseAttr(c));
      matched = true;
      continue;
    }
    if (ch === ":") {
      c.pos = c.pos + 1;
      // A pseudo-element (::before) selects no element in the tree. Matching it
      // as if it were a pseudo-class would invent matches.
      if (peek(c) === ":") {
        throw new UnsupportedSelectorError("pseudo-elements are not selectable");
      }
      compound.pseudos.push(parsePseudo(c));
      matched = true;
      continue;
    }
    if (isIdentChar(ch) && compound.tag === undefined && !matched) {
      compound.tag = readIdent(c).toLowerCase();
      matched = true;
      continue;
    }
    break;
  }

  if (!matched) {
    throw new UnsupportedSelectorError("empty compound selector");
  }
  return compound;
}

function parseAttr(c: Cursor): AttrTest {
  skipSpace(c);
  const name = readIdent(c).toLowerCase();
  if (name === "") throw new UnsupportedSelectorError("attribute selector with no name");
  skipSpace(c);

  let op: AttrOp = "exists";
  const ch = peek(c);
  if (ch === "]") {
    c.pos = c.pos + 1;
    return { name: name, op: op, value: "" };
  }
  if (ch === "=") {
    op = "equals";
    c.pos = c.pos + 1;
  } else if (ch === "^" || ch === "$" || ch === "*" || ch === "|" || ch === "~") {
    if (ch === "^") op = "prefix";
    if (ch === "$") op = "suffix";
    if (ch === "*") op = "contains";
    if (ch === "|") op = "dash";
    if (ch === "~") op = "word";
    c.pos = c.pos + 1;
    if (peek(c) !== "=") throw new UnsupportedSelectorError("malformed attribute operator");
    c.pos = c.pos + 1;
  } else {
    throw new UnsupportedSelectorError("malformed attribute selector");
  }

  skipSpace(c);
  const quote = peek(c);
  const value = quote === '"' || quote === "'" ? readString(c) : readIdent(c);
  skipSpace(c);
  // A case-insensitivity flag changes matching semantics, so accepting it
  // silently would answer a different question than the selector asked.
  if (peek(c) === "i" || peek(c) === "I" || peek(c) === "s" || peek(c) === "S") {
    throw new UnsupportedSelectorError("attribute matching flags are not supported");
  }
  if (peek(c) !== "]") throw new UnsupportedSelectorError("unterminated attribute selector");
  c.pos = c.pos + 1;
  return { name: name, op: op, value: value };
}

function parsePseudo(c: Cursor): PseudoTest {
  const name = readIdent(c).toLowerCase();
  if (!SUPPORTED_PSEUDOS.has(name)) {
    throw new UnsupportedSelectorError("unsupported pseudo-class :" + name);
  }
  if (peek(c) !== "(") {
    return { name: name, selector: undefined, raw: "" };
  }
  const arg = readParenArg(c);
  if (name === "has" || name === "not") {
    return { name: name, selector: parseSelectorList(arg), raw: arg };
  }
  return { name: name, selector: undefined, raw: arg.trim() };
}

/** Parse a comma-separated selector list. Throws for anything unsupported. */
export function parseSelectorList(source: string): SelectorList {
  const cursor: Cursor = { source: source, pos: 0 };
  const list: SelectorList = [];

  while (true) {
    skipSpace(cursor);

    // A relative selector opens with a combinator, which only happens inside a
    // functional pseudo-class like `:has(> td)`. Recording it here keeps the
    // anchoring decision in the matcher instead of losing it at parse time.
    let leading: Combinator | undefined = undefined;
    const lead = peek(cursor);
    if (lead === ">" || lead === "+" || lead === "~") {
      if (lead === ">") leading = "child";
      if (lead === "+") leading = "adjacent";
      if (lead === "~") leading = "sibling";
      cursor.pos = cursor.pos + 1;
      skipSpace(cursor);
    }

    const first = parseCompound(cursor);
    const rest: ComplexPart[] = [];

    while (true) {
      const hadSpace = skipSpace(cursor);
      const ch = peek(cursor);
      if (ch === "" || ch === ",") break;

      let combinator: Combinator = "descendant";
      if (ch === ">" || ch === "+" || ch === "~") {
        if (ch === ">") combinator = "child";
        if (ch === "+") combinator = "adjacent";
        if (ch === "~") combinator = "sibling";
        cursor.pos = cursor.pos + 1;
        skipSpace(cursor);
      } else if (!hadSpace) {
        throw new UnsupportedSelectorError("unexpected character in selector: " + ch);
      }

      rest.push({ combinator: combinator, compound: parseCompound(cursor) });
    }

    list.push({ first: first, rest: rest, leading: leading });
    skipSpace(cursor);
    if (peek(cursor) !== ",") break;
    cursor.pos = cursor.pos + 1;
  }

  if (list.length === 0) throw new UnsupportedSelectorError("empty selector");
  return list;
}

function classList(element: Element): string[] {
  const raw = element.attrs.get("class");
  if (raw === undefined || raw === "") return [];
  const out: string[] = [];
  for (const part of raw.split(/\s+/)) {
    if (part !== "") out.push(part);
  }
  return out;
}

function attrMatches(element: Element, test: AttrTest): boolean {
  const actual = element.attrs.get(test.name);
  if (actual === undefined) return false;
  if (test.op === "exists") return true;
  if (test.op === "equals") return actual === test.value;
  // An empty operand never matches for the substring operators, per the spec.
  if (test.value === "") return false;
  if (test.op === "prefix") return actual.startsWith(test.value);
  if (test.op === "suffix") return actual.endsWith(test.value);
  if (test.op === "contains") return actual.indexOf(test.value) >= 0;
  if (test.op === "dash") return actual === test.value || actual.startsWith(test.value + "-");
  for (const word of actual.split(/\s+/)) {
    if (word === test.value) return true;
  }
  return false;
}

/** Parse an An+B argument into its coefficients. */
function parseNth(raw: string): { a: number; b: number } {
  const text = raw.replace(/\s+/g, "").toLowerCase();
  if (text === "odd") return { a: 2, b: 1 };
  if (text === "even") return { a: 2, b: 0 };

  const match = text.match(/^([+-]?\d*)n([+-]\d+)?$/);
  if (match === null) {
    const plain = parseInt(text, 10);
    if (Number.isNaN(plain)) throw new UnsupportedSelectorError("malformed nth argument: " + raw);
    return { a: 0, b: plain };
  }
  const rawA = match[1];
  let a = 1;
  if (rawA === "-") a = -1;
  else if (rawA !== "" && rawA !== "+") a = parseInt(rawA, 10);
  const b = match[2] === undefined ? 0 : parseInt(match[2], 10);
  return { a: a, b: b };
}

function nthMatches(a: number, b: number, position: number): boolean {
  if (a === 0) return position === b;
  const offset = position - b;
  if (offset === 0) return true;
  if (a > 0) return offset > 0 && offset % a === 0;
  return offset < 0 && offset % a === 0;
}

function sameTypeSiblings(element: Element): Element[] {
  const parent = element.parent;
  if (parent === undefined) return [element];
  const out: Element[] = [];
  for (const sibling of elementChildren(parent)) {
    if (sibling.tag === element.tag) out.push(sibling);
  }
  return out;
}

function pseudoMatches(element: Element, pseudo: PseudoTest, root: Element): boolean {
  const parent = element.parent;
  const siblings = parent === undefined ? [element] : elementChildren(parent);

  if (pseudo.name === "first-child") return siblings.length > 0 && siblings[0] === element;
  if (pseudo.name === "last-child") {
    return siblings.length > 0 && siblings[siblings.length - 1] === element;
  }
  if (pseudo.name === "only-child") return siblings.length === 1;
  if (pseudo.name === "root") return element.parent === root || element.parent === undefined;
  if (pseudo.name === "empty") {
    for (const child of element.children) {
      if (isElement(child)) return false;
      if (child.text.trim() !== "") return false;
    }
    return true;
  }
  if (pseudo.name === "first-of-type") {
    const typed = sameTypeSiblings(element);
    return typed.length > 0 && typed[0] === element;
  }
  if (pseudo.name === "last-of-type") {
    const typed = sameTypeSiblings(element);
    return typed.length > 0 && typed[typed.length - 1] === element;
  }
  if (pseudo.name === "nth-child" || pseudo.name === "nth-of-type") {
    const pool = pseudo.name === "nth-child" ? siblings : sameTypeSiblings(element);
    let position = 0;
    for (let i = 0; i < pool.length; i = i + 1) {
      if (pool[i] === element) {
        position = i + 1;
        break;
      }
    }
    const coeff = parseNth(pseudo.raw);
    return nthMatches(coeff.a, coeff.b, position);
  }
  if (pseudo.name === "not") {
    const list = pseudo.selector;
    if (list === undefined) return true;
    for (const complex of list) {
      if (matchesComplex(element, complex, root)) return false;
    }
    return true;
  }
  if (pseudo.name === "has") {
    const list = pseudo.selector;
    if (list === undefined) return false;
    // `:has(> td)` anchors at the subject, so a relative selector starting with
    // a combinator is evaluated against the subject's own descendants rather
    // than the document. This is the case the HN descriptor depends on.
    for (const complex of list) {
      if (hasMatch(element, complex, root)) return true;
    }
    return false;
  }
  return false;
}

/**
 * Evaluate a `:has()` argument against a subject.
 *
 * The argument is anchored at the subject: `:has(> td > span)` asks for a `td`
 * child of the subject, not a `td` anywhere in the document. Matching it
 * unanchored is the difference between "rows that contain a story link" and
 * "every row on a page that has one somewhere", which on the HN descriptor is
 * the difference between 30 items and every table row in the document.
 */
function hasMatch(subject: Element, complex: ComplexSelector, root: Element): boolean {
  for (const candidate of descendants(subject)) {
    if (!matchesComplex(candidate, complex, root)) continue;
    if (!isAnchoredTo(candidate, complex, subject)) continue;
    return true;
  }
  return false;
}

/**
 * True when a match found for a relative selector actually hangs off the
 * subject the way the leading combinator demands.
 *
 * `matchesComplex` verifies the shape from the candidate leftwards but knows
 * nothing about the anchor, so without this a `> td` argument would accept a
 * `td` nested several levels below the subject.
 */
function isAnchoredTo(
  candidate: Element,
  complex: ComplexSelector,
  subject: Element,
): boolean {
  if (complex.leading === undefined) return isWithin(candidate, subject);

  // Walk back to the element the first compound matched, which is the one the
  // leading combinator constrains relative to the subject.
  let head = candidate;
  for (let i = complex.rest.length - 1; i >= 0; i = i - 1) {
    const part = complex.rest[i];
    if (part.combinator === "child" || part.combinator === "descendant") {
      const parent = head.parent;
      if (parent === undefined) return false;
      head = parent;
      if (part.combinator === "descendant") {
        // A descendant step can span any depth, so the exact head is not
        // recoverable by walking one level. Fall back to containment, which is
        // the weakest true statement rather than a guess.
        return isWithin(candidate, subject);
      }
      continue;
    }
    return isWithin(candidate, subject);
  }

  if (complex.leading === "child") return head.parent === subject;
  if (complex.leading === "adjacent") return previousSibling(head) === subject;
  let probe = previousSibling(head);
  while (probe !== undefined) {
    if (probe === subject) return true;
    probe = previousSibling(probe);
  }
  return false;
}

function isWithin(node: Element, ancestor: Element): boolean {
  let probe = node.parent;
  while (probe !== undefined) {
    if (probe === ancestor) return true;
    probe = probe.parent;
  }
  return false;
}

function matchesCompound(element: Element, compound: CompoundSelector, root: Element): boolean {
  if (compound.tag !== undefined && element.tag !== compound.tag) return false;
  for (const id of compound.ids) {
    if (element.attrs.get("id") !== id) return false;
  }
  if (compound.classes.length > 0) {
    const classes = classList(element);
    for (const wanted of compound.classes) {
      let found = false;
      for (const actual of classes) {
        if (actual === wanted) {
          found = true;
          break;
        }
      }
      if (!found) return false;
    }
  }
  for (const attr of compound.attrs) {
    if (!attrMatches(element, attr)) return false;
  }
  for (const pseudo of compound.pseudos) {
    if (!pseudoMatches(element, pseudo, root)) return false;
  }
  return true;
}

/**
 * Match a complex selector against a subject, walking the combinators right to
 * left. Right-to-left is what makes `a b c` cheap: it starts from the one
 * candidate rather than enumerating every `a` in the document.
 */
export function matchesComplex(
  subject: Element,
  complex: ComplexSelector,
  root: Element,
): boolean {
  if (complex.rest.length === 0) {
    return matchesCompound(subject, complex.first, root);
  }

  const last = complex.rest[complex.rest.length - 1];
  if (!matchesCompound(subject, last.compound, root)) return false;

  const head: ComplexSelector = {
    first: complex.first,
    rest: complex.rest.slice(0, complex.rest.length - 1),
    leading: complex.leading,
  };

  if (last.combinator === "child") {
    const parent = subject.parent;
    if (parent === undefined) return false;
    return matchesComplex(parent, head, root);
  }
  if (last.combinator === "descendant") {
    let probe = subject.parent;
    while (probe !== undefined) {
      if (matchesComplex(probe, head, root)) return true;
      probe = probe.parent;
    }
    return false;
  }
  if (last.combinator === "adjacent") {
    const previous = previousSibling(subject);
    if (previous === undefined) return false;
    return matchesComplex(previous, head, root);
  }
  let probe = previousSibling(subject);
  while (probe !== undefined) {
    if (matchesComplex(probe, head, root)) return true;
    probe = previousSibling(probe);
  }
  return false;
}

function previousSibling(element: Element): Element | undefined {
  const parent = element.parent;
  if (parent === undefined) return undefined;
  const siblings = elementChildren(parent);
  for (let i = 0; i < siblings.length; i = i + 1) {
    if (siblings[i] === element) {
      return i === 0 ? undefined : siblings[i - 1];
    }
  }
  return undefined;
}

/** A compiled selector, parsed once and matched many times. */
export interface Selector {
  list: SelectorList;
  source: string;
}

export function compileSelector(source: string): Selector {
  return { list: parseSelectorList(source), source: source };
}

/** True when the element matches any complex selector in the list. */
export function matches(element: Element, selector: Selector, root: Element): boolean {
  for (const complex of selector.list) {
    if (matchesComplex(element, complex, root)) return true;
  }
  return false;
}

/** Every descendant of `scope` matching the selector, in document order. */
export function selectAll(scope: Element, selector: Selector, root: Element): Element[] {
  const out: Element[] = [];
  for (const candidate of descendants(scope)) {
    if (matches(candidate, selector, root)) out.push(candidate);
  }
  return out;
}

/** The first matching descendant, or undefined. */
export function selectFirst(
  scope: Element,
  selector: Selector,
  root: Element,
): Element | undefined {
  for (const candidate of descendants(scope)) {
    if (matches(candidate, selector, root)) return candidate;
  }
  return undefined;
}

/** Trimmed text of an element, matching the Rust field extractor's shape. */
export function elementText(element: Element): string {
  return textOf(element);
}
