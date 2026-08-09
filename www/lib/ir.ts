import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import type { Example, SiteDescriptor } from "./types";

/**
 * The site reads the same descriptors the CLI ships, straight from
 * `examples/`. Nothing here is a copy: if an IR changes, the page changes.
 *
 * Server-only. Client components import shapes from `./types` instead.
 */
/**
 * Locally the descriptors live one level up, in the repo. On Vercel the build
 * root is `www/`, so a copy lives here too. `bun run sync` refreshes it and a
 * CI check fails if the two ever disagree.
 */
const CANDIDATE_DIRS = [
  join(process.cwd(), "..", "examples"),
  join(process.cwd(), "examples"),
];

export async function listExamples(): Promise<Example[]> {
  let dir: string | undefined;
  let names: string[] = [];
  for (const candidate of CANDIDATE_DIRS) {
    try {
      const found = await readdir(candidate);
      if (found.some((n) => n.endsWith(".surfacer.json"))) {
        dir = candidate;
        names = found;
        break;
      }
    } catch {
      // try the next location
    }
  }
  if (!dir) return [];

  const examples = await Promise.all(
    names
      .filter((n) => n.endsWith(".surfacer.json"))
      .map(async (name) => {
        const raw = await readFile(join(dir, name), "utf8");
        return {
          slug: name.replace(".surfacer.json", ""),
          descriptor: JSON.parse(raw) as SiteDescriptor,
          bytes: raw.length,
        };
      }),
  );

  return examples.sort((a, b) => a.slug.localeCompare(b.slug));
}
