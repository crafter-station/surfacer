// Content normalization for the surfacer TypeScript runtime.
//
// WHY THIS FILE EXISTS, AND WHAT IT IS NOT
//
// The Rust path does not hand raw HTML to the extractor. `execute.rs` shells
// out to `defuddle`, and the extractor runs over defuddle's cleaned `content`.
// That is not an implementation detail: recon authors the IR's selectors while
// looking at defuddle's output, so the selectors encode defuddle's structure.
//
// Measured on https://news.ycombinator.com/news with the committed HN
// descriptor, using the same selectors through the same Rust extractor:
//
//   defuddle content -> 30 rows, title "A 3D fruit fly on macOS desktop...",
//                       url "https://github.com/DenisSergeevitch/desktop-fly"
//   raw fetched HTML -> 30 rows, title "", url "vote?id=49353221&how=up..."
//
// Same selectors, same extractor, different input: the raw page keeps an
// upvote anchor inside the votelinks cell, so `td:last-child a:first-of-type`
// resolves to the vote link and `td:first-child` shifts. Defuddle empties that
// cell because the anchor wraps a `div` with no text.
//
// A standalone binary cannot call defuddle: it is an npm program, and requiring
// it would break exactly the promise the ts-cli target makes. So this file
// carries the one transform the measurement showed is load-bearing, and claims
// nothing beyond it.
//
// This is NOT a defuddle reimplementation. Defuddle scores blocks for
// readability, strips boilerplate, and computes a word count. None of that is
// here, because none of it was measured to change a selector's result. What is
// here is the structural cleanup that makes the IR's selectors resolve to the
// same elements they resolve to on the Rust path.

import { Element, elementChildren, isElement, textOf } from "./html";

/** Elements that carry no content and no structure a selector should count. */
const DROPPED_TAGS = new Set<string>([
  "script",
  "style",
  "noscript",
  "template",
  "iframe",
  "form",
  "input",
  "button",
  "select",
  "textarea",
  "svg",
  "canvas",
]);

/**
 * Remove the elements defuddle removes, in the one respect that changes what a
 * selector resolves to: a subtree that renders no text is not content.
 *
 * The two rules, each traceable to the diff between defuddle's output and the
 * raw page:
 *
 *  1. Drop the tags in DROPPED_TAGS outright. Defuddle's output for the HN page
 *     contains none of them, while the raw page has script, form, and input.
 *
 *  2. Unwrap a `div` and drop an `a` that renders no text. The upvote control
 *     is `<a><div class="votearrow"></div></a>`: an anchor whose subtree has no
 *     text at all. Defuddle leaves `<td></td>` in its place. An anchor with text
 *     is content and stays, which is why the story link and the domain link
 *     survive this pass.
 *
 * Verified: after this pass, the committed HN descriptor's selectors produce
 * the same field values through this runtime that they produce through the
 * Rust extractor over defuddle's content.
 */
export function normalizeForExtraction(root: Element): void {
  pruneNode(root);
}

function pruneNode(element: Element): void {
  const kept: Element[] = [];

  for (const child of element.children) {
    if (!isElement(child)) {
      kept.push(child);
      continue;
    }

    if (DROPPED_TAGS.has(child.tag)) continue;

    pruneNode(child);

    // An anchor with no rendered text is a control, not content. Dropping it
    // is what turns the votelinks cell into the empty cell the selectors were
    // written against.
    if (child.tag === "a" && textOf(child).trim() === "" && !hasImage(child)) {
      continue;
    }

    // A `div` inside content is layout. Defuddle's HN output has no div at all,
    // and its children are kept in place, so unwrapping preserves any text the
    // div happened to hold instead of discarding it.
    if (child.tag === "div") {
      for (const grandchild of child.children) {
        if (isElement(grandchild)) grandchild.parent = element;
        kept.push(grandchild);
      }
      continue;
    }

    kept.push(child);
  }

  element.children = kept;
  reindex(element);
}

/** An anchor wrapping only an image still shows something, so it is content. */
function hasImage(element: Element): boolean {
  if (element.tag === "img") return true;
  for (const child of elementChildren(element)) {
    if (hasImage(child)) return true;
  }
  return false;
}

/** Recompute sibling positions after a prune, so `:first-child` and friends
 *  answer about the tree the selectors actually run against. */
function reindex(element: Element): void {
  let position = 0;
  for (const child of element.children) {
    if (!isElement(child)) continue;
    child.indexInParent = position;
    position = position + 1;
  }
}

/**
 * Collapse HTML to text, mirroring `html_to_text` in
 * `crates/surfacer-app/src/execute.rs`.
 *
 * Kept in step with the Rust version deliberately: it is what fills
 * `ExecResult.content` when no extractor applies, so the two paths' raw output
 * stays comparable.
 */
export function htmlToText(html: string): string {
  let text = html;
  text = text.split("<br>").join("\n");
  text = text.split("<br/>").join("\n");
  text = text.split("<br />").join("\n");
  for (const close of ["</p>", "</div>", "</li>", "</tr>", "</h1>", "</h2>", "</h3>", "</h4>"]) {
    text = text.split(close).join("\n");
  }

  let result = "";
  let inTag = false;
  for (const ch of text) {
    if (ch === "<") {
      inTag = true;
      continue;
    }
    if (ch === ">") {
      inTag = false;
      continue;
    }
    if (!inTag) result = result + ch;
  }

  result = result.split("&amp;").join("&");
  result = result.split("&lt;").join("<");
  result = result.split("&gt;").join(">");
  result = result.split("&quot;").join('"');
  result = result.split("&#x27;").join("'");
  result = result.split("&nbsp;").join(" ");

  const lines: string[] = [];
  for (const line of result.split("\n")) {
    const trimmed = line.trim();
    if (trimmed !== "") lines.push(trimmed);
  }
  return lines.join("\n");
}
