// Extractor port for the surfacer TypeScript runtime.
//
// This is a port of `crates/surfacer-app/src/extract.rs`, kept deliberately
// close to it: same fallback order, same "an item with every field missing is
// not an item" filter, same 1-based index, same per-type coercions. The Rust
// path is the reference. Where this file cannot do what Rust does, it says so
// in a comment and returns a named reason rather than an empty result, because
// an empty result is indistinguishable from "the page had nothing".

import { Element, isElement, textOf } from "./html";
import {
  Selector,
  UnsupportedSelectorError,
  compileSelector,
  matches,
  selectAll,
  selectFirst,
} from "./css";
import { normalizeForExtraction } from "./normalize";
import { parseDocument } from "./html";

export type FieldType = "text" | "url" | "number" | "dateTime";

// EVERY FIELD BELOW IS REQUIRED AND NON-NULLABLE, WHICH IS DELIBERATE.
//
// The obvious modelling here is `attribute?: string | null`, mirroring the IR's
// `Option<String>`. That shape does not survive compilation: scriptc requires a
// record literal to match its expected type exactly, or to width-coerce by a
// rule that `null | string | undefined` does not satisfy (SC2002/SC2003). An
// emitter writing `attribute: null` for one field and `attribute: "href"` for
// the next produces two different inferred types, and the array of them stops
// unifying.
//
// So absence is spelled as the empty string, and the emitter always writes
// every field. The runtime reads "" as "not set", which is unambiguous for the
// four fields involved: a CSS selector, an attribute name, an AX role, and a
// regex pattern are all meaningless when empty, so no real value is lost.
//
// The IR keeps its `Option<String>`; the translation happens in the emitter,
// which is the one place that already knows it is generating for this target.

export type PatternStrategy = "css" | "axTree" | "cssThenAx";
export type PaginationStrategy = "queryParam" | "nextLink";
export type SourceKind = "css" | "axTree";

/**
 * A field's source, flattened into one record rather than a union of a CSS and
 * an AX variant.
 *
 * Flattened for the same reason: the two variants have different fields, so an
 * array holding both stops unifying under the exact-match rule. `from`
 * discriminates, and the branch that does not apply leaves its fields empty.
 */
export interface FieldSource {
  from: SourceKind;
  /** CSS: the selector, relative to the item element. */
  selector: string;
  /** CSS: the attribute to read, or "" to read the element's text. */
  attribute: string;
  /** AX: the role to match. */
  role: string;
  /** AX: a regex the accessible name must match, or "". */
  namePattern: string;
  /** AX: the property to read, or "". */
  property: string;
}

export interface FieldDef {
  name: string;
  fieldType: FieldType;
  source: FieldSource;
}

export interface ItemPattern {
  strategy: PatternStrategy;
  /** "" when the pattern is AX-only, which this runtime cannot satisfy. */
  cssSelector: string;
  axRole: string;
  axNamePattern: string;
}

export interface PaginationDef {
  strategy: PaginationStrategy;
  nextCssSelector: string;
  pageParam: string;
}

/**
 * An extractor, flattened from the IR's three-arm enum for the same reason as
 * FieldSource. `type` discriminates; a detail or raw extractor carries an empty
 * item pattern and, for raw, no fields.
 */
export interface Extractor {
  type: "list" | "detail" | "raw";
  itemPattern: ItemPattern;
  fields: FieldDef[];
  pagination: PaginationDef;
}

/** A field value, tagged exactly as `ExtractedValue` serializes in the IR. */
export type ExtractedValue =
  | { type: "text"; value: string }
  | { type: "url"; value: string }
  | { type: "number"; value: number }
  | { type: "dateTime"; value: string }
  | { type: "missing" };

export interface ExtractedItem {
  index: number;
  /** Serialized with sorted keys, because the Rust side holds these in a
   *  BTreeMap and serde emits them in key order. A cross-language golden test
   *  compares the JSON, so the ordering is part of the contract. */
  fields: Record<string, ExtractedValue>;
}

/**
 * Why an extraction produced nothing.
 *
 * The Rust path returns `Option<Vec<_>>` and the caller falls back to raw
 * content, so a `None` there is silent by design. Here the reason is carried
 * out so the CLI can print it, because a user who asked for structure and got
 * raw text deserves to know which of these happened.
 */
export type ExtractionFailure =
  | { reason: "notAList"; detail: string }
  | { reason: "noCssSelector"; detail: string }
  | { reason: "unsupportedSelector"; detail: string }
  | { reason: "noMatches"; detail: string };

export interface ExtractionOutcome {
  items: ExtractedItem[] | undefined;
  failure: ExtractionFailure | undefined;
}

const MISSING: ExtractedValue = { type: "missing" };

function isMissing(value: ExtractedValue): boolean {
  return value.type === "missing";
}

/**
 * Extract items from an HTML document per an extractor.
 *
 * Mirrors `extract_items`: only the list extractor produces items. Detail and
 * raw return nothing, which sends the caller to the raw-content path exactly as
 * the Rust match arms do.
 */
export function extractItems(html: string, extractor: Extractor): ExtractionOutcome {
  if (extractor.type === "detail") {
    return {
      items: undefined,
      failure: {
        reason: "notAList",
        detail: "detail extractors have no item list; the Rust path returns None here too",
      },
    };
  }
  if (extractor.type === "raw") {
    return {
      items: undefined,
      failure: { reason: "notAList", detail: "raw extractor requests unstructured content" },
    };
  }

  const root = parseDocument(html);
  normalizeForExtraction(root);
  return extractList(root, extractor);
}

/**
 * The list path. `extract.rs` tries the plain selection first and falls back to
 * a sibling-aware pass for table layouts where a row's metadata lives in the
 * next row. Both are here, in that order.
 */
function extractList(root: Element, list: Extractor): ExtractionOutcome {
  const css = list.itemPattern.cssSelector;

  // The AX strategies address an accessibility tree, which is produced by a
  // browser. This runtime compiles to a binary with no browser and no CDP
  // connection, so there is no tree to query. The Rust extractor is in the same
  // position: `extract_field` returns Missing for every AxTree source, and
  // `extract_list` reads only `css_selector`. So a descriptor whose pattern is
  // AX-only yields nothing on both paths, and naming that is the honest move.
  if (css === "") {
    return {
      items: undefined,
      failure: {
        reason: "noCssSelector",
        detail:
          "item pattern strategy '" +
          list.itemPattern.strategy +
          "' has no cssSelector. An accessibility tree needs a browser, which a standalone binary has not got.",
      },
    };
  }

  let selector: Selector;
  try {
    selector = compileSelector(css);
  } catch (err) {
    if (err instanceof UnsupportedSelectorError) {
      return {
        items: undefined,
        failure: { reason: "unsupportedSelector", detail: css + ": " + err.message },
      };
    }
    throw err;
  }

  const direct = extractDirect(root, selector, list);
  if (direct.length > 0) return { items: direct, failure: undefined };

  const siblings = extractWithSiblings(root, selector, list);
  if (siblings.length > 0) return { items: siblings, failure: undefined };

  return {
    items: undefined,
    failure: {
      reason: "noMatches",
      detail: "selector '" + css + "' matched no element carrying any of the declared fields",
    },
  };
}

/** The plain pass: select items, read each field inside the item. */
function extractDirect(root: Element, selector: Selector, list: Extractor): ExtractedItem[] {
  const out: ExtractedItem[] = [];
  let index = 0;

  for (const element of selectAll(root, selector, root)) {
    index = index + 1;
    const fields: Record<string, ExtractedValue> = {};
    for (const field of list.fields) {
      fields[field.name] = extractField(element, field, root);
    }

    // An item where every field is Missing is a selector that matched a shape
    // rather than a record. Dropping it keeps a stray row from padding results.
    let anyPresent = false;
    for (const key of Object.keys(fields)) {
      if (!isMissing(fields[key])) {
        anyPresent = true;
        break;
      }
    }
    if (!anyPresent) continue;

    out.push({ index: index, fields: fields });
  }

  // The Rust version renumbers nothing: `index` comes from the enumeration over
  // all matches, before the empty-item filter. Keeping that means an item's
  // index matches its position on the page, which is what `open 3` relies on.
  return out;
}

/**
 * The sibling-aware pass, mirroring `extract_list_with_siblings`.
 *
 * Table layouts put a row's title in one `<tr>` and its metadata in the next.
 * For each field the Rust code tries the row, then the following row, then a
 * regex over the two rows' combined text. This does the same, in the same
 * order, with the same field-name-driven patterns.
 */
function extractWithSiblings(
  root: Element,
  selector: Selector,
  list: Extractor,
): ExtractedItem[] {
  const trSelector = compileSelector("tr");
  const allRows = selectAll(root, trSelector, root);

  const matchedIndices: number[] = [];
  for (let i = 0; i < allRows.length; i = i + 1) {
    if (matches(allRows[i], selector, root)) matchedIndices.push(i);
  }
  if (matchedIndices.length === 0) return [];

  const out: ExtractedItem[] = [];
  for (let itemIdx = 0; itemIdx < matchedIndices.length; itemIdx = itemIdx + 1) {
    const rowIdx = matchedIndices[itemIdx];
    const element = allRows[rowIdx];
    const fields: Record<string, ExtractedValue> = {};

    for (const field of list.fields) {
      const value = extractField(element, field, root);
      if (!isMissing(value)) {
        fields[field.name] = value;
        continue;
      }

      const sibling = rowIdx + 1 < allRows.length ? allRows[rowIdx + 1] : undefined;
      if (sibling !== undefined) {
        const siblingValue = extractField(sibling, field, root);
        if (!isMissing(siblingValue)) {
          fields[field.name] = siblingValue;
          continue;
        }
      }

      // Rust joins the row's text nodes with a space here, unlike the field
      // extractor which joins with nothing. The patterns below are written
      // against the spaced form, so the difference is load-bearing.
      const combined = spacedText(element);
      const siblingText = sibling === undefined ? "" : spacedText(sibling);
      fields[field.name] = extractFromText(
        combined + " " + siblingText,
        field.name,
        field.fieldType,
      );
    }

    let anyPresent = false;
    for (const key of Object.keys(fields)) {
      if (!isMissing(fields[key])) {
        anyPresent = true;
        break;
      }
    }
    if (!anyPresent) continue;

    out.push({ index: itemIdx + 1, fields: fields });
  }

  return out;
}

/** Text nodes joined with a space, matching `element.text().join(" ")`. */
function spacedText(element: Element): string {
  const parts: string[] = [];
  collectText(element, parts);
  return parts.join(" ");
}

function collectText(element: Element, into: string[]): void {
  for (const child of element.children) {
    if (isElement(child)) {
      collectText(child, into);
      continue;
    }
    into.push(child.text);
  }
}

/**
 * Last-resort text patterns, ported from `extract_from_text`.
 *
 * These are keyed by field name, not by field type, which is a deliberate
 * quirk of the Rust original: recon names a points field "points", so the name
 * is the strongest signal available about what the text means. Any other name
 * yields Missing there, and yields Missing here.
 */
function extractFromText(text: string, fieldName: string, fieldType: FieldType): ExtractedValue {
  if (fieldName === "points") {
    const match = text.match(/(\d+)\s*points?/);
    if (match === null) return MISSING;
    const parsed = Number.parseFloat(match[1]);
    return Number.isNaN(parsed) ? MISSING : { type: "number", value: parsed };
  }
  if (fieldName === "author") {
    const match = text.match(/by\s+(\w+)/);
    return match === null ? MISSING : { type: "text", value: match[1] };
  }
  if (fieldName === "age" || fieldName === "time") {
    const match = text.match(/(\d+\s*(?:minutes?|hours?|days?|months?|years?)\s*ago)/);
    return match === null ? MISSING : { type: "text", value: match[0] };
  }
  if (fieldName === "comments" || fieldName === "commentCount") {
    const match = text.match(/(\d+)\s*comments?/);
    if (match === null) return MISSING;
    const parsed = Number.parseFloat(match[1]);
    return Number.isNaN(parsed) ? MISSING : { type: "number", value: parsed };
  }
  // Rust returns Missing for every remaining name, for both branches of its
  // match on field_type. The parameter stays so the signature keeps mirroring
  // the original, and so a future type-driven rule has a place to land.
  void fieldType;
  return MISSING;
}

/** Dispatch a field to its source. AX sources are Missing, exactly as in Rust. */
function extractField(element: Element, field: FieldDef, root: Element): ExtractedValue {
  if (field.source.from === "css") {
    return extractCssField(element, field.source, field.fieldType, root);
  }
  // `extract_field` in extract.rs returns ExtractedValue::Missing for
  // FieldSource::AxTree unconditionally. The AX branch is not skipped here
  // because a binary lacks a browser; it is skipped because the reference
  // implementation skips it, and this runtime tracks the reference.
  return MISSING;
}

function extractCssField(
  element: Element,
  source: FieldSource,
  fieldType: FieldType,
  root: Element,
): ExtractedValue {
  let selector: Selector;
  try {
    selector = compileSelector(source.selector);
  } catch {
    // Rust returns Missing for a selector `scraper` refuses to parse. Same here,
    // so an exotic selector degrades a field instead of failing the command.
    return MISSING;
  }

  const target = selectFirst(element, selector, root);
  if (target === undefined) return MISSING;

  let raw: string;
  if (source.attribute !== "") {
    const attr = target.attrs.get(source.attribute);
    if (attr === undefined) return MISSING;
    raw = attr;
  } else {
    raw = textOf(target).trim();
  }

  if (raw === "") return MISSING;

  if (fieldType === "text" || fieldType === "dateTime") {
    // Rust maps both Text and DateTime to ExtractedValue::Text, so a dateTime
    // field serializes as `{"type":"text"}`. Matching that keeps the JSON
    // identical across the two paths.
    return { type: "text", value: raw };
  }
  if (fieldType === "url") {
    return { type: "url", value: raw };
  }

  // Number keeps only digits and dots, then parses. A value that survives the
  // filter but is not a number (say "1.2.3") falls back to Text, which is what
  // the Rust `unwrap_or` does.
  let digits = "";
  for (const ch of raw) {
    if ((ch >= "0" && ch <= "9") || ch === ".") digits = digits + ch;
  }
  const parsed = Number.parseFloat(digits);
  // Rust's `str::parse::<f64>` rejects "1.2.3" and trailing junk that
  // Number.parseFloat would happily truncate, so the shape of the check has to
  // be stricter than parseFloat alone to keep the two paths agreeing.
  if (digits === "" || Number.isNaN(parsed) || !isStrictF64(digits)) {
    return { type: "text", value: raw };
  }
  return { type: "number", value: parsed };
}

/** True when the whole string parses as an f64, the way Rust's parse does. */
function isStrictF64(text: string): boolean {
  let seenDot = false;
  let seenDigit = false;
  for (const ch of text) {
    if (ch === ".") {
      if (seenDot) return false;
      seenDot = true;
      continue;
    }
    if (ch < "0" || ch > "9") return false;
    seenDigit = true;
  }
  return seenDigit;
}

/** Ordered field keys, so serialization matches serde's BTreeMap ordering. */
export function sortedFieldKeys(item: ExtractedItem): string[] {
  return Object.keys(item.fields).sort();
}

/**
 * Serialize items the way `format_extracted_json` does: the same envelope, the
 * same key order, two-space indent.
 */
export function formatExtractedJson(
  items: ExtractedItem[],
  url: string,
  title: string,
): string {
  const ordered = items.map((item) => {
    const fields: Record<string, ExtractedValue> = {};
    for (const key of sortedFieldKeys(item)) {
      fields[key] = item.fields[key];
    }
    return { index: item.index, fields: fields };
  });

  return JSON.stringify(
    { url: url, title: title, itemCount: items.length, items: ordered },
    null,
    2,
  );
}

/**
 * True when an item carries a field under this name.
 *
 * Every accessor below goes through this rather than reading the key and
 * comparing to undefined. Reading an absent key returns undefined under a JS
 * runtime, but a scriptc-compiled binary has no representation for undefined in
 * a typed record slot and traps at runtime instead ("record has no key"). That
 * failure only appears on a path that reads an optional field, which is why it
 * survived until a human-formatted run asked for "points" on a page that has
 * none.
 */
export function hasField(item: ExtractedItem, name: string): boolean {
  for (const key of Object.keys(item.fields)) {
    if (key === name) return true;
  }
  return false;
}

/** The primary title of an item, mirroring `ExtractedItem::primary_title`. */
export function primaryTitle(item: ExtractedItem): string {
  for (const name of ["title", "name", "heading"]) {
    if (!hasField(item, name)) continue;
    const value = item.fields[name];
    if (value.type === "text" || value.type === "url" || value.type === "dateTime") {
      return value.value;
    }
  }
  return "";
}

/** The primary URL of an item, mirroring `ExtractedItem::primary_url`. */
export function primaryUrl(item: ExtractedItem): string {
  for (const name of ["url", "link", "href", "commentsUrl"]) {
    if (!hasField(item, name)) continue;
    const value = item.fields[name];
    if (value.type === "url") return value.value;
  }
  return "";
}

/** A text field, mirroring `ExtractedItem::get_text`. Absent reads as "". */
export function getText(item: ExtractedItem, name: string): string {
  if (!hasField(item, name)) return "";
  const value = item.fields[name];
  if (value.type === "text" || value.type === "url" || value.type === "dateTime") {
    return value.value;
  }
  return "";
}

/**
 * A numeric field, mirroring `ExtractedItem::get_number`.
 *
 * Absence reads as NaN rather than undefined, for the same reason as above: a
 * compiled binary cannot hold undefined in a number slot. Callers test with
 * Number.isNaN, which is unambiguous because an extracted number that failed to
 * parse is stored as text, never as NaN.
 */
export function getNumber(item: ExtractedItem, name: string): number {
  if (!hasField(item, name)) return Number.NaN;
  const value = item.fields[name];
  if (value.type !== "number") return Number.NaN;
  return value.value;
}
