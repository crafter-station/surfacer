import { emitOpenapi, TARGETS } from "@/lib/emit";
import { highlight } from "@/lib/highlight";
import { listExamples } from "@/lib/ir";
import { CommandBlock } from "./copy-block";
import { Playground } from "./playground";
import { SpecViewer } from "./spec-viewer";

const REPO = "https://github.com/crafter-station/surfacer";
const RECON =
  "https://github.com/crafter-station/skills/tree/main/skills/surface-recon";

const LINK =
  "underline decoration-neutral-300 underline-offset-4 transition-colors hover:text-neutral-900 dark:decoration-neutral-700 dark:hover:text-neutral-100";

/** Prose stays at reading width; the playground gets the full column. */
const PROSE = "mx-auto max-w-2xl px-6";

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="font-mono text-xs uppercase tracking-widest text-neutral-500">
      {children}
    </h2>
  );
}

export default async function Home() {
  const examples = await listExamples();

  // The SUNAT declaraciones example is the one that carries all three auth
  // states, so its emitted OpenAPI is what the Scalar viewer shows. Emitted from
  // the same descriptor the playground reads, so it never drifts.
  const sunatAuth = examples.find(
    (e) => e.slug === "sunat-declaraciones",
  )?.descriptor;
  const sunatSpec = sunatAuth ? emitOpenapi(sunatAuth) : null;

  // Shiki runs on the server, so the unedited state of every pane is
  // highlighted at build time. Once the visitor edits, the playground emits in
  // the browser and renders plain text instead. See the note in playground.tsx.
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
    <main className="w-full py-20 sm:py-28">
      <header className={PROSE}>
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

        <p className="mt-8 text-balance text-3xl font-medium leading-[1.15] tracking-tight sm:text-4xl">
          Generate the interface instead of writing it.
        </p>
        <p className="mt-4 max-w-xl text-balance text-lg leading-snug text-neutral-600 dark:text-neutral-400">
          Describe a surface once in a JSON file. Compile it into a CLI, an MCP
          server, an OpenAPI document, and a native binary that cannot disagree
          with each other, because nothing hand-wrote them.
        </p>
      </header>

      {/*
        The problem before the product. Peru's tax portal is the case that
        shaped the auth model: the fields stay disabled until a background call
        returns, so driving the DOM never works. Both sides of this are true of
        the same form, and the right side is the descriptor in the playground
        below.
      */}
      <section className={`${PROSE} mt-16`}>
        <SectionLabel>Before and after, on one real form</SectionLabel>
        <div className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2">
          <div className="rounded-md border border-neutral-200 p-4 dark:border-neutral-900">
            <p className="font-mono text-[11px] uppercase tracking-widest text-neutral-400 dark:text-neutral-600">
              A portal with no API
            </p>
            <pre className="mt-3 overflow-x-auto font-mono text-xs leading-relaxed text-neutral-500">
              <code>{`click  #periodo
wait   until enabled
       (fields load from a
        background call)
click  #btnConsultar
parse  the rendered table
retry  when the markup moves`}</code>
            </pre>
          </div>
          <div className="rounded-md border border-neutral-300 p-4 dark:border-neutral-700">
            <p className="font-mono text-[11px] uppercase tracking-widest text-neutral-400 dark:text-neutral-600">
              The same form, compiled
            </p>
            <pre className="mt-3 overflow-x-auto font-mono text-xs leading-relaxed text-neutral-700 dark:text-neutral-300">
              <code>{`sunat-declaraciones \\
  f616 periodo --json

GET /v1/.../obtenerPeriodo/032026
Header: IdCache`}</code>
            </pre>
          </div>
        </div>
        <p className="mt-4 leading-relaxed text-neutral-600 dark:text-neutral-400">
          Underneath that form sits a JSON API. Reaching it needs a session
          token the portal mints only during its own browser login, which is why
          the DOM was the only door anyone found. surfacer captures the token
          once and reads the API headless for its hour.
        </p>
      </section>

      <section
        className={`${PROSE} mt-16 space-y-4 leading-relaxed text-neutral-600 dark:text-neutral-400`}
      >
        <SectionLabel>What it does</SectionLabel>
        <p className="pt-1">
          Hand-written CLIs drift.{" "}
          <code className="font-mono text-sm">--json</code> lands on some
          commands, a banner breaks the parse, a subcommand never reaches{" "}
          <code className="font-mono text-sm">--help</code>. Every gap costs an
          agent a retry.
        </p>
        <p>
          surfacer compiles one descriptor into every interface, so they cannot
          disagree. It does not map surfaces itself: that judgment belongs to
          the{" "}
          <a href={RECON} className={LINK}>
            surface-recon
          </a>{" "}
          skill or to a person writing the descriptor by hand. Bring your own
          IR.
        </p>
      </section>

      {/*
        The playground is the argument, not a demo of it. These emitters are a
        second consumer of the IR that never imports the Rust CLI, which is the
        neutrality claim from the README running in the visitor's browser.
      */}
      <section className="mt-16">
        <div className={PROSE}>
          <SectionLabel>One IR, six interfaces</SectionLabel>
          <p className="mt-3 leading-relaxed text-neutral-600 dark:text-neutral-400">
            The descriptor on the left is the source. Everything on the right is
            a build artifact. Edit the descriptor and every target recompiles as
            you type. Try renaming a command, or giving{" "}
            <code className="font-mono text-sm">user</code> a second parameter.
          </p>
        </div>
        <div className="mx-auto mt-4 max-w-6xl px-6">
          <Playground examples={examples} panes={panes} />
        </div>
        {/*
          neutral-500 measures 4.18:1 on the dark background, under the 4.5:1
          AA floor, so prose this long uses the 600/400 pair (7.81:1 light,
          7.85:1 dark) and neutral-500 stays for short captions only.
        */}
        <div
          className={`${PROSE} mt-3 space-y-3 text-sm leading-relaxed text-neutral-600 dark:text-neutral-400`}
        >
          <p>
            It opens on Hacker News because that one you can check. Ten
            operations, eight of them plain reads,{" "}
            <code className="font-mono text-xs">threads</code> and{" "}
            <code className="font-mono text-xs">user</code> taking an{" "}
            <code className="font-mono text-xs">id</code>. Open the site in
            another tab and compare it against the descriptor.
          </p>
          <p>
            Hacker News also publishes{" "}
            <a href="https://github.com/HackerNews/API" className={LINK}>
              an official API
            </a>
            , so nobody needs surfacer to read it. That is the reason it is the
            example here: you can audit the output against a published contract
            and confirm the descriptor invents nothing. The targets worth
            compiling are the ones with no such contract, which is what the auth
            section below is about.
          </p>
          <p>
            The emitters running here are TypeScript ports of the ones in the
            CLI, reading the same descriptors from{" "}
            <a href={`${REPO}/tree/main/examples`} className={LINK}>
              examples/
            </a>
            . The CLI stays authoritative for the byte-exact artifact. That this
            page can compile an IR without importing the CLI is the point: the
            IR is a plain file, and every consumer is downstream of it.
          </p>
        </div>
      </section>

      <section className={`${PROSE} mt-16`}>
        <SectionLabel>Start</SectionLabel>
        <p className="mt-3 leading-relaxed text-neutral-600 dark:text-neutral-400">
          Install surfacer and run a real command against Hacker News in four
          lines.
        </p>
        <div className="mt-4">
          <CommandBlock
            lines={[
              "curl -fsSL https://surfacer.dev/install.sh | sh",
              "curl -O https://raw.githubusercontent.com/crafter-station/surfacer/main/examples/news-ycombinator-com.surfacer.json",
              "surfacer install ./news-ycombinator-com.surfacer.json --dest ~/.cargo/bin",
              "news-ycombinator-com news --json | jq '.items[].fields.title.value'",
            ]}
            caption="No LLM tokens at runtime. macOS and Linux."
          />
        </div>
        <p className="mt-6 leading-relaxed text-neutral-600 dark:text-neutral-400">
          Or emit a target from an IR you already have.{" "}
          <code className="font-mono text-sm">--out-dir</code> goes before the
          target.
        </p>
        <div className="mt-4">
          <CommandBlock
            lines={[
              "surfacer lint ./news-ycombinator-com.surfacer.json",
              "surfacer emit --out-dir ./build ts-cli ./news-ycombinator-com.surfacer.json",
              "surfacer emit --out-dir ./build mcp ./news-ycombinator-com.surfacer.json",
            ]}
          />
        </div>
      </section>

      <section
        className={`${PROSE} mt-16 space-y-4 leading-relaxed text-neutral-600 dark:text-neutral-400`}
      >
        <SectionLabel>Auth the others skip</SectionLabel>
        <p className="pt-1">
          SDK generators assume a token you can mint yourself. Many portals
          don&apos;t work that way: they hand out a session token only inside
          their own browser login, with an audience your own client can never
          request. That single gap is why so many surfaces have no client at
          all.
        </p>
        <p>
          surfacer models it. It captures the token once from the browser, then
          reads the API headless until it expires. The IR keeps acquisition and
          use separate, so the browser step runs once and the headless calls run
          for the token&apos;s whole life. That mode is why{" "}
          <a
            href="https://github.com/crafter-research/sunat-cli"
            className={LINK}
          >
            SUNAT
          </a>{" "}
          reads clean today.
        </p>
        <p>
          Auth attaches at the surface level and can be overridden per
          operation, because one host often mixes several. Peru&apos;s tax
          portal runs all three at once, and the{" "}
          <code className="font-mono text-sm">sunat-declaraciones</code>{" "}
          descriptor in the playground carries them together:
        </p>
        <ul className="space-y-2 border-l border-neutral-200 pl-4 dark:border-neutral-900">
          <li>
            <code className="font-mono text-sm">oAuth2</code> as the surface
            default, for the SIRE sales register.
          </li>
          <li>
            <code className="font-mono text-sm">browserBootstrappedToken</code>{" "}
            overriding it on the F616 monthly declaration, the form whose fields
            stay disabled until a background call returns.
          </li>
          <li>
            <code className="font-mono text-sm">none</code>, stated outright, on
            the public padron lookup. An operation that needs no credentials
            says so instead of leaving a caller to find out.
          </li>
        </ul>
      </section>

      {sunatSpec ? (
        <section className="mt-16" id="spec">
          <div className={PROSE}>
            <SectionLabel>The OpenAPI it emits</SectionLabel>
            <p className="mt-3 text-sm leading-relaxed text-neutral-600 dark:text-neutral-400">
              Those three states, compiled. This is the same SUNAT descriptor
              emitted as OpenAPI 3.1 and rendered in Scalar: OAuth2 on the SIRE
              operation, <code className="font-mono text-xs">security: []</code>{" "}
              on the padron, and an{" "}
              <code className="font-mono text-xs">x-surfacer-auth</code>{" "}
              extension on F616, where OpenAPI has no vocabulary for the browser
              mode and the emitter declares it openly rather than faking an API
              key. Read only, since those endpoints answer to a real
              browser-captured token.
            </p>
          </div>
          <div className="mx-auto mt-4 max-w-5xl px-6">
            <div className="overflow-hidden rounded-md border border-neutral-200 dark:border-neutral-900">
              <SpecViewer spec={sunatSpec} />
            </div>
          </div>
        </section>
      ) : null}

      <section
        className={`${PROSE} mt-16 space-y-4 leading-relaxed text-neutral-600 dark:text-neutral-400`}
      >
        <SectionLabel>When the surface moves</SectionLabel>
        <p className="pt-1">
          This is the step the other three exist for. A surface with no official
          API has no deprecation notice either, and the usual failure is not a
          badly written client, it is that the target moved and nothing said so
          until an answer came back wrong.
        </p>
        <div className="my-4">
          <CommandBlock
            lines={["surfacer check news-ycombinator-com --json"]}
          />
        </div>
        <p>
          <code className="font-mono text-sm">check</code> takes up to three
          endpoints from the IR as canaries, fetches their signatures, and
          compares them against a stored fingerprint. When one changes you
          update the descriptor and re-emit every target, which is cheaper than
          patching an integration you hand-wrote a year ago.
        </p>
        <p>
          Two limits worth knowing. Drift covers HTTP only, and an IR whose
          operations are not HTTP has nothing to fingerprint. And a canary is
          evidence, not proof: three endpoints answering unchanged does not mean
          the response bodies kept their shape. Drift detected is a strong
          signal, drift not detected is a weak one.
        </p>
      </section>

      <section className={`${PROSE} mt-16`}>
        <SectionLabel>Install</SectionLabel>
        <div className="mt-4">
          <CommandBlock
            lines={["curl -fsSL https://surfacer.dev/install.sh | sh"]}
          />
        </div>
        <p className="mt-3 text-sm text-neutral-500">
          macOS and Linux.{" "}
          <a href={`${REPO}/releases/latest`} className={LINK}>
            Releases
          </a>
          , or build from source with{" "}
          <code className="font-mono text-xs">
            cargo install --git {REPO} surfacer
          </code>
          .
        </p>
        <p className="mt-3 text-sm text-neutral-500">
          Early development. The pipeline works end to end for public HTML
          sites, from an IR through every emitter.
        </p>
      </section>

      <footer
        className={`${PROSE} mt-20 flex items-center gap-3 text-sm text-neutral-500`}
      >
        <span className="flex items-center gap-3 border-t border-neutral-200 pt-6 dark:border-neutral-900 w-full">
          <a href={REPO} className={LINK}>
            GitHub
          </a>
          <span className="text-neutral-300 dark:text-neutral-800">/</span>
          <a href="https://crafter.run" className={LINK}>
            Crafter Station
          </a>
        </span>
      </footer>
    </main>
  );
}
