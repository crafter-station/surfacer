"use client";

import { useCallback, useEffect, useRef, useState } from "react";

/**
 * A command block you can copy without selecting it by hand.
 *
 * `navigator.clipboard` needs a secure context, so it is absent over plain HTTP
 * and in a few older browsers. The fallback path selects the text in a detached
 * textarea and asks the document to copy it, which works everywhere the modern
 * API does not.
 */
async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through to the legacy path
  }

  try {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(area);
    return ok;
  } catch {
    return false;
  }
}

export function CopyButton({
  text,
  label = "Copy",
  className = "",
}: {
  text: string;
  label?: string;
  className?: string;
}) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const onCopy = useCallback(async () => {
    const ok = await copyText(text);
    setState(ok ? "copied" : "failed");
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setState("idle"), 1600);
  }, [text]);

  return (
    <button
      type="button"
      onClick={onCopy}
      // The label changes on copy, so screen readers hear the result without a
      // separate live region.
      aria-label={
        state === "copied"
          ? "Copied to clipboard"
          : state === "failed"
            ? "Copy failed"
            : label
      }
      className={`rounded border border-neutral-200 bg-white/80 px-2 py-1 font-mono text-[11px] text-neutral-500 backdrop-blur transition-colors hover:text-neutral-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-neutral-400 dark:border-neutral-800 dark:bg-neutral-950/80 dark:text-neutral-500 dark:hover:text-neutral-100 ${className}`}
    >
      {state === "copied" ? "copied" : state === "failed" ? "failed" : "copy"}
    </button>
  );
}

/**
 * A shell block with a copy button. `lines` is the literal text copied, so what
 * lands on the clipboard is exactly what is on screen.
 */
export function CommandBlock({
  lines,
  caption,
}: {
  lines: string[];
  caption?: string;
}) {
  const text = lines.join("\n");

  return (
    <div>
      {/*
        The button sits in its own row above the scroll area rather than
        floating over it. Overlaid, it covered the tail of the longest command
        (the raw.githubusercontent URL) at every width narrow enough to matter.
      */}
      <div className="overflow-hidden rounded-md border border-neutral-200 dark:border-neutral-900">
        <div className="flex items-center justify-between border-b border-neutral-200 px-3 py-1.5 dark:border-neutral-900">
          <span className="font-mono text-[11px] uppercase tracking-widest text-neutral-400 dark:text-neutral-600">
            shell
          </span>
          <CopyButton
            text={text}
            className="border-0 bg-transparent px-1 py-0 backdrop-blur-none dark:bg-transparent"
          />
        </div>
        <pre className="w-full min-w-0 overflow-x-auto p-4 font-mono text-xs leading-relaxed text-neutral-700 dark:text-neutral-300">
          <code>
            {lines.map((line, i) => (
              <span
                // Command lines are fixed and never reorder, so the index is stable.
                // biome-ignore lint/suspicious/noArrayIndexKey: static list
                key={i}
                className="block"
              >
                {line.startsWith("#") ? (
                  <span className="text-neutral-400 dark:text-neutral-600">
                    {line}
                  </span>
                ) : (
                  line
                )}
              </span>
            ))}
          </code>
        </pre>
      </div>
      {caption ? (
        <p className="mt-2 text-sm text-neutral-500">{caption}</p>
      ) : null}
    </div>
  );
}
