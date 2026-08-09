"use client";

import { useMemo, useState } from "react";
import { TARGETS, type TargetId } from "@/lib/emit";
import type { Example } from "@/lib/types";

export function Playground({ examples }: { examples: Example[] }) {
  const [slug, setSlug] = useState(examples[0]?.slug ?? "");
  const [target, setTarget] = useState<TargetId>("help");

  const example = examples.find((e) => e.slug === slug) ?? examples[0];

  const output = useMemo(() => {
    if (!example) return "";
    const t = TARGETS.find((t) => t.id === target) ?? TARGETS[0];
    try {
      return t.emit(example.descriptor);
    } catch (error) {
      return `Failed to render: ${error instanceof Error ? error.message : String(error)}`;
    }
  }, [example, target]);

  if (!example) {
    return (
      <p className="text-sm text-neutral-500">
        No descriptors found in <code>examples/</code>.
      </p>
    );
  }

  const opCount = example.descriptor.operations.length;
  const paramCount =
    example.descriptor.http?.endpoints.reduce(
      (sum, e) => sum + (e.params?.length ?? 0),
      0,
    ) ?? 0;

  return (
    <div className="rounded-lg border border-neutral-200 dark:border-neutral-800">
      <div className="flex flex-wrap items-center gap-2 border-b border-neutral-200 px-3 py-2 dark:border-neutral-800">
        <select
          value={slug}
          onChange={(e) => setSlug(e.target.value)}
          className="rounded border border-neutral-300 bg-transparent px-2 py-1 text-sm dark:border-neutral-700"
          aria-label="Descriptor"
        >
          {examples.map((e) => (
            <option key={e.slug} value={e.slug}>
              {e.descriptor.meta.displayName}
            </option>
          ))}
        </select>

        <div className="flex gap-1" role="tablist" aria-label="Emit target">
          {TARGETS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={target === t.id}
              onClick={() => setTarget(t.id)}
              className={`rounded px-2 py-1 font-mono text-xs transition-colors ${
                target === t.id
                  ? "bg-neutral-900 text-white dark:bg-white dark:text-neutral-900"
                  : "text-neutral-600 hover:bg-neutral-100 dark:text-neutral-400 dark:hover:bg-neutral-900"
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        <span className="ml-auto font-mono text-xs text-neutral-500">
          {opCount} operations, {paramCount} params
        </span>
      </div>

      <pre className="max-h-96 overflow-auto p-4 font-mono text-xs leading-relaxed">
        <code>{output}</code>
      </pre>
    </div>
  );
}
