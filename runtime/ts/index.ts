// The surfacer TypeScript runtime.
//
// Written once, imported by every standalone emitter. Before this existed the
// generic execution logic lived only in Rust (`extract.rs` and `execute.rs`),
// and the shim inherited it by shelling out to `surfacer exec` while the
// standalone targets did not inherit it at all. The result was two classes of
// emitter that nobody had declared: delegating ones that got the extractor for
// free, and standalone ones that shipped without it and returned raw HTML.
//
// The rule this file encodes: generic behavior is written once per language,
// not once per emitter.

export {
  Element,
  Node,
  TextNode,
  decodeEntities,
  descendants,
  elementChildren,
  isElement,
  parseDocument,
  textOf,
} from "./html";

export {
  Selector,
  SelectorList,
  UnsupportedSelectorError,
  compileSelector,
  matches,
  selectAll,
  selectFirst,
} from "./css";

export { htmlToText, normalizeForExtraction } from "./normalize";

export {
  ExtractedItem,
  ExtractedValue,
  ExtractionFailure,
  ExtractionOutcome,
  Extractor,
  FieldDef,
  FieldType,
  ItemPattern,
  PaginationDef,
  FieldSource,
  extractItems,
  formatExtractedJson,
  getNumber,
  getText,
  primaryTitle,
  primaryUrl,
  sortedFieldKeys,
} from "./extract";

export {
  EX_USAGE,
  MissingParam,
  MissingParamsError,
  Param,
  missingParams,
  missingParamsError,
} from "./params";

export { ExecResult, execResultFromHtml, formatExecJson } from "./exec";
