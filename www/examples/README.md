# Descriptor copies

Mirrors of `../../examples/`. The site reads the repo copy when it can, and
falls back to these when the build root is `www/`, which is how Vercel builds
it.

Refresh with `bun run sync`. `bun run check:sync` fails if the two disagree.
