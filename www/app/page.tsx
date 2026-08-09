import { listExamples } from "@/lib/ir";
import { Playground } from "./playground";

const REPO = "https://github.com/crafter-station/surfacer";

export default async function Home() {
  const examples = await listExamples();

  return (
    <main className="mx-auto max-w-3xl px-6 py-16 sm:py-24">
      <header>
        <h1 className="font-mono text-2xl font-semibold tracking-tight">
          surfacer
        </h1>
        <p className="mt-4 text-lg leading-relaxed text-neutral-700 dark:text-neutral-300">
          Generate the interface instead of writing it. Map a surface once, emit
          CLIs, agent tools, and native binaries that stay consistent because
          nothing hand-wrote them.
        </p>
      </header>

      <section className="mt-12">
        <p className="leading-relaxed text-neutral-600 dark:text-neutral-400">
          Software is increasingly operated by agents rather than people, and
          the interfaces they reach for were designed for humans. A CLI written
          by hand accumulates inconsistencies nobody notices until an agent hits
          them: <code className="font-mono text-sm">--json</code> on some
          commands and not others, a decorative header that breaks the parse, a
          subcommand missing from{" "}
          <code className="font-mono text-sm">--help</code> so the agent never
          learns it exists. Each one is a retry, a wasted token, or a wrong
          answer.
        </p>
        <p className="mt-4 leading-relaxed text-neutral-600 dark:text-neutral-400">
          surfacer removes the remembering. It maps a surface once into a
          declarative intermediate representation, then generates interfaces
          from it. Every command gets the same flag handling, the same JSON
          output, the same help, because one emitter wrote all of them. When the
          surface changes, you re-run recon instead of rewriting the client.
        </p>
        <p className="mt-4 leading-relaxed text-neutral-600 dark:text-neutral-400">
          That matters most where no interface exists at all. Most of the useful
          internet has no documented API, and reaching it means either paying an
          LLM on every run, or hand-writing a client that breaks the next time
          the site moves. That is why unofficial clients die: yt-dlp runs three
          release channels to keep up, and spotify-tui was abandoned when
          patching stopped scaling.
        </p>
      </section>

      <section className="mt-12">
        <h2 className="font-mono text-sm uppercase tracking-wider text-neutral-500">
          One IR, many interfaces
        </h2>
        <p className="mt-3 text-sm leading-relaxed text-neutral-600 dark:text-neutral-400">
          Every descriptor below is read straight from{" "}
          <code className="font-mono text-xs">examples/</code> in the repo. Pick
          one, pick a target, and see what the emitters produce.
        </p>
        <div className="mt-4">
          <Playground examples={examples} />
        </div>
      </section>

      <section className="mt-12">
        <h2 className="font-mono text-sm uppercase tracking-wider text-neutral-500">
          Install
        </h2>
        <pre className="mt-3 overflow-x-auto rounded-lg border border-neutral-200 p-4 font-mono text-xs dark:border-neutral-800">
          <code>
            {`curl -fsSL https://raw.githubusercontent.com/crafter-station/surfacer/main/install.sh | sh`}
          </code>
        </pre>
        <p className="mt-3 text-sm text-neutral-500">
          No release is published yet. Until one is, build from source with{" "}
          <code className="font-mono text-xs">
            cargo install --git {REPO} surfacer
          </code>
          .
        </p>
      </section>

      <footer className="mt-16 border-t border-neutral-200 pt-6 text-sm dark:border-neutral-800">
        <a
          href={REPO}
          className="text-neutral-600 underline-offset-4 hover:underline dark:text-neutral-400"
        >
          github.com/crafter-station/surfacer
        </a>
        <span className="mx-2 text-neutral-300 dark:text-neutral-700">/</span>
        <span className="text-neutral-500">
          built by{" "}
          <a
            href="https://crafter.run"
            className="underline-offset-4 hover:underline"
          >
            Crafter Station
          </a>
        </span>
      </footer>
    </main>
  );
}
