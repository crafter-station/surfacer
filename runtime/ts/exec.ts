// The unstructured result shape, ported from `ExecResult` in
// `crates/surfacer-app/src/execute.rs`.
//
// This is what a command returns when no extractor applies, which is the path
// the Rust side takes after `extract_items` yields None. Keeping the field names
// and the serialization identical is what lets a caller move between the two
// paths without rewriting its parser.

import { htmlToText } from "./normalize";

export interface ExecResult {
  status: number;
  url: string;
  title: string;
  content: string;
  /**
   * Words in the extracted content.
   *
   * On the Rust path this comes from defuddle, which computes it while parsing.
   * This runtime has no defuddle, so the value is counted from the text it
   * produced. The two numbers are computed from different inputs and will not
   * generally agree; the cross-path test names this as an expected difference
   * rather than asserting equality.
   */
  wordCount: number;
}

/** Build an ExecResult from fetched HTML, mirroring the Rust fallback path. */
export function execResultFromHtml(
  status: number,
  url: string,
  title: string,
  html: string,
): ExecResult {
  const content = htmlToText(html);
  return {
    status: status,
    url: url,
    title: title,
    content: content,
    wordCount: countWords(content),
  };
}

function countWords(text: string): number {
  let count = 0;
  let inWord = false;
  for (const ch of text) {
    const isSpace = ch === " " || ch === "\n" || ch === "\t" || ch === "\r";
    if (isSpace) {
      inWord = false;
      continue;
    }
    if (!inWord) {
      count = count + 1;
      inWord = true;
    }
  }
  return count;
}

/** Serialize an ExecResult the way `format_json` does: two-space indent, camelCase. */
export function formatExecJson(result: ExecResult): string {
  return JSON.stringify(result, null, 2);
}
