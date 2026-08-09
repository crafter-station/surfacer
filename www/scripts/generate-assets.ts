/**
 * Generates favicon and social images from `public/logo.svg`.
 *
 * Safe to re-run: every output is overwritten from the same source.
 *
 *   bun run scripts/generate-assets.ts
 */

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const PUBLIC = join(ROOT, "public");
const APP = join(ROOT, "app");

const INK = "#ededed";
const PAPER = "#0a0a0a";

/** The mark, colored and sized for a standalone raster. */
function markSvg(color: string, size: number): Buffer {
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 64 64" fill="none" color="${color}">
      <rect x="4" y="18" width="24" height="24" rx="4" stroke="currentColor" stroke-width="3"/>
      <path d="M28 30h8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
      <path d="M36 30V12h6" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M36 30h6" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
      <path d="M36 30v18h6" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
      <circle cx="50" cy="12" r="6" fill="currentColor"/>
      <circle cx="50" cy="30" r="6" fill="currentColor"/>
      <circle cx="50" cy="48" r="6" fill="currentColor"/>
    </svg>`,
  );
}

function socialSvg(width: number, height: number): Buffer {
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
      <rect width="${width}" height="${height}" fill="${PAPER}"/>
      <g transform="translate(96, ${height / 2 - 176}) scale(2.4)" color="${INK}" fill="none">
        <rect x="4" y="18" width="24" height="24" rx="4" stroke="currentColor" stroke-width="3"/>
      <path d="M28 30h8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
      <path d="M36 30V12h6" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
      <path d="M36 30h6" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
      <path d="M36 30v18h6" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
      <circle cx="50" cy="12" r="6" fill="currentColor"/>
      <circle cx="50" cy="30" r="6" fill="currentColor"/>
      <circle cx="50" cy="48" r="6" fill="currentColor"/>
      </g>
      <text x="96" y="${height / 2 + 52}" font-family="ui-monospace, SFMono-Regular, Menlo, monospace" font-size="64" font-weight="500" fill="${INK}">surfacer</text>
      <text x="96" y="${height / 2 + 112}" font-family="ui-sans-serif, system-ui, -apple-system, sans-serif" font-size="30" fill="#8f8f8f">Generate the interface instead of writing it.</text>
    </svg>`,
  );
}

async function main() {
  await mkdir(PUBLIC, { recursive: true });

  const og = join(PUBLIC, "og.png");
  await sharp(socialSvg(1200, 630)).png().toFile(og);

  const ogTwitter = join(PUBLIC, "og-twitter.png");
  await sharp(socialSvg(1200, 600)).png().toFile(ogTwitter);

  // ICO needs its own encoder; sharp writes PNG entries and `to-ico` packs them.
  const { default: toIco } = await import("to-ico");
  const entries = await Promise.all(
    [16, 32, 48].map((size) =>
      sharp(markSvg(INK, size)).resize(size, size).png().toBuffer(),
    ),
  );
  await writeFile(join(APP, "favicon.ico"), await toIco(entries));

  // Next serves `app/icon.png` at higher resolutions than the ICO covers.
  await sharp(markSvg(INK, 180))
    .resize(180, 180)
    .png()
    .toFile(join(APP, "icon.png"));
  await sharp(markSvg(INK, 180))
    .resize(180, 180)
    .flatten({ background: PAPER })
    .png()
    .toFile(join(APP, "apple-icon.png"));

  console.log(
    "wrote og.png, og-twitter.png, favicon.ico, icon.png, apple-icon.png",
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
