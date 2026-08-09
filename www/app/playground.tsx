"use client";

import { useState } from "react";
import { TARGETS, type TargetId } from "@/lib/emit";
import type { Example } from "@/lib/types";

export function Playground({
  examples,
  panes,
}: {
  examples: Example[];
  /** Pre-highlighted HTML, keyed `${slug}:${targetId}`. */
  panes: Record<string, string>;
}) {
  const [slug, setSlug] = useState(examples[0]?.slug ?? "");
  const [target, setTarget] = useState<TargetId>("help");

  const example = examples.find((e) => e.slug === slug) ?? examples[0];

  if (!example) {
    return (
      <p className="text-sm text-neutral-500">
        No descriptors found in <code>examples/</code>.
      </p>
    );
  }

  const html = panes[`${example.slug}:${target}`] ?? "";
  const opCount = example.descriptor.operations.length;

  return (
    <div className="overflow-hidden rounded-md border border-neutral-200 dark:border-neutral-900">
      <div className="flex items-center gap-1 border-b border-neutral-200 px-2 py-1.5 dark:border-neutral-900">
        <select
          value={slug}
          onChange={(e) => setSlug(e.target.value)}
          className="mr-1 rounded bg-transparent px-1.5 py-1 font-mono text-xs text-neutral-600 outline-none hover:text-neutral-900 dark:text-neutral-400 dark:hover:text-neutral-100"
          aria-label="Descriptor"
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
            className={`rounded px-2 py-1 font-mono text-xs transition-colors ${
              target === t.id
                ? "bg-neutral-100 text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100"
                : "text-neutral-500 hover:text-neutral-900 dark:hover:text-neutral-200"
            }`}
          >
            {t.label}
          </button>
        ))}

        <span className="ml-auto pr-1 font-mono text-xs text-neutral-400 dark:text-neutral-600">
          {opCount} ops
        </span>
      </div>

      <div
        className="max-h-80 overflow-auto p-4 font-mono text-xs leading-relaxed [&_pre]:bg-transparent"
        // Shiki output, generated on the server from a checked-in descriptor.
        // biome-ignore lint/security/noDangerouslySetInnerHtml: server-rendered
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}
