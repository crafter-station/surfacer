import { createHighlighter, type Highlighter } from "shiki";

/**
 * One highlighter for the whole process. Creating one per render loads the
 * grammars again each time, which is slow enough to notice.
 */
let instance: Promise<Highlighter> | undefined;

function get(): Promise<Highlighter> {
  instance ??= createHighlighter({
    themes: ["github-dark-dimmed", "github-light"],
    langs: ["typescript", "javascript", "bash"],
  });
  return instance;
}

/** Shiki has no `text` grammar; plain output is escaped and left alone. */
function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export async function highlight(code: string, lang: string): Promise<string> {
  if (lang === "text") {
    return `<pre class="shiki"><code>${escapeHtml(code)}</code></pre>`;
  }

  const highlighter = await get();
  return highlighter.codeToHtml(code, {
    lang,
    themes: { light: "github-light", dark: "github-dark-dimmed" },
    defaultColor: false,
  });
}
