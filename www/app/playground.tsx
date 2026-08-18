"use client";

import { useCallback, useMemo, useState } from "react";
import { TARGETS, type TargetId } from "@/lib/emit";
import type { Example, SiteDescriptor } from "@/lib/types";
import { CopyButton } from "./copy-block";

/**
 * Highlighting strategy: server-rendered Shiki for the pristine example, plain
 * monospace once you edit.
 *
 * Measured before choosing (bun build --minify, gzipped):
 *   oniguruma engine + typescript/javascript/json .... 1094 KB raw / 298 KB gz
 *   javascript engine + typescript/javascript/json ...  536 KB raw /  89 KB gz
 *   javascript engine + json only ....................  189 KB raw /  58 KB gz
 *
 * The playground needs five languages, so the honest client-side number is the
 * middle row plus the custom help grammar. Shipping ~89 KB gzipped of syntax
 * highlighter to color text the visitor is actively typing over costs more than
 * it returns, and it would land on every visitor including the ones who never
 * edit. So the first paint keeps the highlighted server render, and the moment
 * the descriptor changes the output switches to plain monospace, which is what
 * a terminal shows anyway.
 *
 * The tradeoff is visible rather than hidden: the panel says when it is showing
 * live output, so nobody mistakes the color change for a rendering bug.
 */

type ParseState =
  | { ok: true; descriptor: SiteDescriptor }
  | { ok: false; error: string };

function parseDescriptor(source: string): ParseState {
  let parsed: unknown;
  try {
    parsed = JSON.parse(source);
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : "Invalid JSON",
    };
  }

  // The emitters read meta and operations without guarding, so a well-formed
  // JSON document that is not a descriptor would still throw inside emit. Check
  // the shape here and report it the same way as a syntax error.
  if (typeof parsed !== "object" || parsed === null) {
    return { ok: false, error: "Descriptor must be a JSON object" };
  }
  const candidate = parsed as Partial<SiteDescriptor>;
  if (typeof candidate.meta?.siteName !== "string") {
    return { ok: false, error: "Missing meta.siteName" };
  }
  if (typeof candidate.meta?.sourceUrl !== "string") {
    return { ok: false, error: "Missing meta.sourceUrl" };
  }
  if (!Array.isArray(candidate.operations)) {
    return { ok: false, error: "Missing operations array" };
  }
  return { ok: true, descriptor: parsed as SiteDescriptor };
}

export function Playground({
  examples,
  panes,
}: {
  examples: Example[];
  /** Pre-highlighted HTML for the unedited descriptors, keyed `${slug}:${targetId}`. */
  panes: Record<string, string>;
}) {
  // Hacker News opens the playground on purpose. It is the one descriptor a
  // visitor can audit: open the site in another tab and compare. The others
  // stay in the selector, and SUNAT carries its argument down in the auth
  // section instead of competing for this slot.
  const first =
    examples.find((e) => e.slug === "news-ycombinator-com") ?? examples[0];
  const [slug, setSlug] = useState(first?.slug ?? "");
  const [target, setTarget] = useState<TargetId>("help");
  const [draft, setDraft] = useState<string | null>(null);

  const example = examples.find((e) => e.slug === slug) ?? first;

  const pristine = useMemo(
    () => (example ? `${JSON.stringify(example.descriptor, null, 2)}\n` : ""),
    [example],
  );

  const source = draft ?? pristine;
  const edited = draft !== null && draft !== pristine;

  // Every keystroke reparses and re-emits. The descriptors are small enough
  // (13 KB at the largest) that this stays imperceptible, so there is no
  // debounce to explain away.
  const parsed = useMemo(() => parseDescriptor(source), [source]);

  const output = useMemo(() => {
    if (!parsed.ok) return null;
    const entries: { id: TargetId; text: string; error?: string }[] = [];
    for (const t of TARGETS) {
      try {
        entries.push({ id: t.id, text: t.emit(parsed.descriptor) });
      } catch (error) {
        // One emitter throwing must not take the other five with it.
        entries.push({
          id: t.id,
          text: "",
          error:
            error instanceof Error
              ? error.message
              : "This emitter could not read the descriptor",
        });
      }
    }
    return entries;
  }, [parsed]);

  const onSelect = useCallback((next: string) => {
    setSlug(next);
    // Switching descriptors discards the draft, otherwise the editor would show
    // one surface while the selector claims another.
    setDraft(null);
  }, []);

  if (!example) {
    return (
      <p className="text-sm text-neutral-500">
        No descriptors found in <code className="font-mono">examples/</code>.
      </p>
    );
  }

  const active = output?.find((o) => o.id === target);
  const opCount = parsed.ok ? parsed.descriptor.operations.length : null;
  const serverHtml = panes[`${example.slug}:${target}`] ?? "";

  return (
    <div className="overflow-hidden rounded-md border border-neutral-200 dark:border-neutral-900">
      <div className="flex flex-wrap items-center gap-x-1 gap-y-1.5 border-b border-neutral-200 px-2 py-1.5 dark:border-neutral-900">
        <label htmlFor="playground-descriptor" className="sr-only">
          Descriptor
        </label>
        <select
          id="playground-descriptor"
          value={slug}
          onChange={(e) => onSelect(e.target.value)}
          className="mr-1 rounded bg-transparent px-1.5 py-1 font-mono text-xs text-neutral-600 outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-neutral-400 hover:text-neutral-900 dark:text-neutral-400 dark:hover:text-neutral-100"
        >
          {examples.map((e) => (
            <option key={e.slug} value={e.slug}>
              {e.descriptor.meta.siteName}
            </option>
          ))}
        </select>

        {TARGETS.map((t) => (
          <button
            key={t.id}
            type="button"
            aria-pressed={target === t.id}
            onClick={() => setTarget(t.id)}
            className={`rounded px-2 py-1 font-mono text-xs transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-neutral-400 ${
              target === t.id
                ? "bg-neutral-100 text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100"
                : "text-neutral-500 hover:text-neutral-900 dark:hover:text-neutral-200"
            }`}
          >
            {t.label}
          </button>
        ))}

        <span className="ml-auto flex items-center gap-2 pr-1">
          {edited ? (
            <button
              type="button"
              onClick={() => setDraft(null)}
              className="rounded px-2 py-1 font-mono text-xs text-neutral-500 transition-colors hover:text-neutral-900 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-neutral-400 dark:hover:text-neutral-100"
            >
              reset
            </button>
          ) : null}
          <span className="font-mono text-xs text-neutral-400 dark:text-neutral-600">
            {opCount === null ? "invalid IR" : `${opCount} ops`}
          </span>
        </span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2">
        <div className="flex min-w-0 flex-col border-b border-neutral-200 lg:border-b-0 lg:border-r dark:border-neutral-900">
          <div className="flex items-center justify-between border-b border-neutral-200 px-3 py-1.5 dark:border-neutral-900">
            <span className="font-mono text-[11px] uppercase tracking-widest text-neutral-400 dark:text-neutral-600">
              {example.descriptor.meta.siteName}.surfacer.json
            </span>
            <span className="font-mono text-[11px] text-neutral-400 dark:text-neutral-600">
              {edited ? "edited" : "source"}
            </span>
          </div>
          <label htmlFor="playground-ir" className="sr-only">
            Descriptor JSON, editable
          </label>
          <textarea
            id="playground-ir"
            value={source}
            onChange={(e) => setDraft(e.target.value)}
            spellCheck={false}
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            aria-describedby={parsed.ok ? undefined : "playground-parse-error"}
            className="h-64 w-full resize-none bg-transparent p-4 font-mono text-xs leading-relaxed text-neutral-700 outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-neutral-400 lg:h-96 dark:text-neutral-300"
          />
        </div>

        <div className="flex min-w-0 flex-col">
          <div className="flex items-center justify-between border-b border-neutral-200 px-3 py-1.5 dark:border-neutral-900">
            <span className="font-mono text-[11px] uppercase tracking-widest text-neutral-400 dark:text-neutral-600">
              emit {target}
            </span>
            <span className="flex items-center gap-2">
              {active?.text ? (
                <CopyButton
                  text={active.text}
                  className="border-0 bg-transparent px-1 py-0 backdrop-blur-none dark:bg-transparent"
                />
              ) : null}
            </span>
          </div>

          {!parsed.ok ? (
            <output
              id="playground-parse-error"
              className="h-64 w-full min-w-0 overflow-auto p-4 font-mono text-xs leading-relaxed lg:h-96"
            >
              <span className="text-red-600 dark:text-red-400">
                {parsed.error}
              </span>
              <span className="mt-2 block text-neutral-600 dark:text-neutral-400">
                The descriptor has to parse before anything can compile from it.
                Fix the JSON, or press reset.
              </span>
            </output>
          ) : edited ? (
            <div className="h-64 w-full min-w-0 overflow-auto p-4 font-mono text-xs leading-relaxed lg:h-96">
              {active?.error ? (
                <span className="text-red-600 dark:text-red-400">
                  {active.error}
                </span>
              ) : (
                <pre className="whitespace-pre text-neutral-700 dark:text-neutral-300">
                  {active?.text}
                </pre>
              )}
            </div>
          ) : (
            <div
              className="h-64 w-full min-w-0 overflow-auto p-4 font-mono text-xs leading-relaxed lg:h-96 [&_pre]:bg-transparent"
              // Shiki output for the unedited descriptor, generated on the server.
              // biome-ignore lint/security/noDangerouslySetInnerHtml: server-rendered
              dangerouslySetInnerHTML={{ __html: serverHtml }}
            />
          )}
        </div>
      </div>

      <p className="border-t border-neutral-200 px-3 py-2 text-xs text-neutral-500 dark:border-neutral-900">
        {edited
          ? "Compiled in your browser from the descriptor on the left, by the same emitters the CLI runs."
          : "Edit the descriptor and every target recompiles as you type."}
      </p>
    </div>
  );
}
