import { createHighlighter, type Highlighter } from "shiki";
import { helpGrammar } from "./help-grammar";

/**
 * One highlighter for the whole process. Creating one per render loads the
 * grammars again each time, which is slow enough to notice.
 */
let instance: Promise<Highlighter> | undefined;

function get(): Promise<Highlighter> {
  instance ??= createHighlighter({
    themes: ["github-dark-dimmed", "github-light"],
    langs: ["typescript", "javascript", "json", "bash", helpGrammar],
  });
  return instance;
}

export async function highlight(code: string, lang: string): Promise<string> {
  const highlighter = await get();
  return highlighter.codeToHtml(code, {
    lang,
    themes: { light: "github-light", dark: "github-dark-dimmed" },
    defaultColor: false,
  });
}
