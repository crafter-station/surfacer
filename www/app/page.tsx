import { emitOpenapi, TARGETS } from "@/lib/emit";
import { highlight } from "@/lib/highlight";
import { listExamples } from "@/lib/ir";
import { Playground } from "./playground";
import { SpecViewer } from "./spec-viewer";

const REPO = "https://github.com/crafter-station/surfacer";

export default async function Home() {
  const examples = await listExamples();

  // The SUNAT declaraciones example is the one that carries all three auth
  // states, so its emitted OpenAPI is what the Scalar viewer shows. Emitted from
  // the same descriptor the playground reads, so it never drifts.
  const sunatAuth = examples.find(
    (e) => e.slug === "sunat-declaraciones",
  )?.descriptor;
  const sunatSpec = sunatAuth ? emitOpenapi(sunatAuth) : null;

  // Shiki runs on the server, so every pane is highlighted at build time and
  // the client only swaps which one is visible.
  const panes: Record<string, string> = {};
  await Promise.all(
    examples.flatMap((example) =>
      TARGETS.map(async (target) => {
        panes[`${example.slug}:${target.id}`] = await highlight(
          target.emit(example.descriptor),
          target.lang,
        );
      }),
    ),
  );

  return (
    <main className="mx-auto max-w-2xl px-6 py-20 sm:py-28">
      <header>
        <div className="flex items-center gap-2.5">
          <svg
            viewBox="0 0 64 64"
            fill="none"
            aria-hidden="true"
            className="h-5 w-5 text-neutral-900 dark:text-neutral-100"
          >
            <rect
              x="4"
              y="18"
              width="24"
              height="24"
              rx="4"
              stroke="currentColor"
              strokeWidth="3"
            />
            <path
              d="M28 30h8"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
            />
            <path
              d="M36 30V12h6"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M36 30h6"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
            />
            <path
              d="M36 30v18h6"
              stroke="currentColor"
              strokeWidth="3"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <circle cx="50" cy="12" r="6" fill="currentColor" />
            <circle cx="50" cy="30" r="6" fill="currentColor" />
            <circle cx="50" cy="48" r="6" fill="currentColor" />
          </svg>
          <h1 className="font-mono text-xl font-medium tracking-tight">
            surfacer
          </h1>
        </div>
        <p className="mt-3 text-balance text-lg leading-snug text-neutral-600 dark:text-neutral-400">
          Generate the interface instead of writing it.
        </p>
      </header>

      <section className="mt-14 space-y-4 leading-relaxed text-neutral-600 dark:text-neutral-400">
        <p>
          Hand-written CLIs drift.{" "}
          <code className="font-mono text-sm">--json</code> lands on some
          commands, a banner breaks the parse, a subcommand never reaches{" "}
          <code className="font-mono text-sm">--help</code>. Every gap costs an
          agent a retry.
        </p>
        <p>
          surfacer compiles one descriptor into every interface, so they cannot
          disagree. Bring your own IR, from the{" "}
          <a
            href="https://github.com/crafter-station/skills/tree/main/skills/surface-recon"
            className="underline decoration-neutral-700 underline-offset-4 hover:text-neutral-400"
          >
            surface-recon
          </a>{" "}
          skill or written by hand.
        </p>
        <p>
          The descriptor is the source and the interfaces are build artifacts,
          which is what makes the next part possible: a surface with no official
          API has no deprecation notice either.{" "}
          <code className="font-mono text-sm">surfacer check</code> tells you it
          moved, and you re-emit instead of rewriting the client.
        </p>
      </section>

      <section className="mt-14">
        <h2 className="font-mono text-xs uppercase tracking-widest text-neutral-500">
          One IR, many interfaces
        </h2>
        <div className="mt-4">
          <Playground examples={examples} panes={panes} />
        </div>
        <p className="mt-3 text-sm text-neutral-500">
          Read live from{" "}
          <a
            href={`${REPO}/tree/main/examples`}
            className="underline decoration-neutral-700 underline-offset-4 hover:text-neutral-400"
          >
            examples/
          </a>
          .
        </p>
      </section>

      {sunatSpec ? (
        <section className="mt-14" id="spec">
          <h2 className="font-mono text-xs uppercase tracking-widest text-neutral-500">
            The OpenAPI it emits
          </h2>
          <p className="mt-3 text-sm text-neutral-500">
            The curated SUNAT descriptor emitted as OpenAPI 3.1, shown in
            Scalar. The three auth states carry through: OAuth2 on SIRE, an{" "}
            <code className="font-mono text-xs">x-surfacer-auth</code> extension
            on the browser-only F616 form, and a public padron lookup. Read
            only, since those endpoints answer to a real browser-captured token.
          </p>
          <div className="mt-4 overflow-hidden rounded-md border border-neutral-200 dark:border-neutral-900">
            <SpecViewer spec={sunatSpec} />
          </div>
        </section>
      ) : null}

      <section className="mt-14 space-y-4 leading-relaxed text-neutral-600 dark:text-neutral-400">
        <h2 className="font-mono text-xs uppercase tracking-widest text-neutral-500">
          Auth the others skip
        </h2>
        <p className="mt-4">
          SDK generators assume a token you can mint yourself. Many portals
          don&apos;t work that way: they hand out a session token only inside
          their own browser login, with an audience your own client can never
          request.
        </p>
        <p>
          surfacer models that. It captures the token once from the browser,
          then reads the API headless until it expires. A tax form that fought
          every DOM automation becomes a handful of authenticated calls. That
          mode is why{" "}
          <a
            href="https://github.com/crafter-research/sunat-cli"
            className="underline decoration-neutral-700 underline-offset-4 hover:text-neutral-400"
          >
            SUNAT
          </a>{" "}
          reads clean today.
        </p>
      </section>

      <section className="mt-14">
        <h2 className="font-mono text-xs uppercase tracking-widest text-neutral-500">
          Install
        </h2>
        <pre className="mt-4 overflow-x-auto rounded-md border border-neutral-200 p-4 font-mono text-xs dark:border-neutral-900">
          <code>curl -fsSL https://surfacer.dev/install.sh | sh</code>
        </pre>
        <p className="mt-3 text-sm text-neutral-500">
          macOS and Linux.{" "}
          <a
            href={`${REPO}/releases/latest`}
            className="underline decoration-neutral-700 underline-offset-4 hover:text-neutral-400"
          >
            Releases
          </a>
          , or build from source with{" "}
          <code className="font-mono text-xs">
            cargo install --git {REPO} surfacer
          </code>
          .
        </p>
      </section>

      <footer className="mt-20 flex items-center gap-3 border-t border-neutral-200 pt-6 text-sm text-neutral-500 dark:border-neutral-900">
        <a
          href={REPO}
          className="underline decoration-neutral-700 underline-offset-4 hover:text-neutral-400"
        >
          GitHub
        </a>
        <span className="text-neutral-300 dark:text-neutral-800">/</span>
        <a
          href="https://crafter.run"
          className="underline decoration-neutral-700 underline-offset-4 hover:text-neutral-400"
        >
          Crafter Station
        </a>
      </footer>
    </main>
  );
}
