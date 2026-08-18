"use client";

import { ApiReferenceReact } from "@scalar/api-reference-react";
import "@scalar/api-reference-react/style.css";
import { useEffect, useState } from "react";

/**
 * Read-only Scalar view of an emitted OpenAPI document.
 *
 * The SUNAT declaration endpoints need a real browser-captured token to answer,
 * so a live "Send request" here would only fail or ask for credentials. The view
 * exists to show the shape and the three auth states, not to fire requests, so
 * the interactive client is hidden.
 */
export function SpecViewer({ spec }: { spec: string }) {
  // Scalar takes its theme as a prop rather than reading the page, so a
  // hardcoded value left a dark panel sitting in the middle of the light page.
  // Resolved after mount, since the server cannot know the visitor's setting.
  const [dark, setDark] = useState(true);

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const prefersDark = () =>
      document.documentElement.classList.contains("dark") || query.matches;

    // Scalar writes `dark-mode` onto the host page's <body>, not onto its own
    // container, and it does so after mounting. Setting the prop alone loses
    // the race, so the class is corrected here too and re-corrected whenever
    // Scalar rewrites it.
    const sync = () => {
      const wantDark = prefersDark();
      setDark(wantDark);
      document.body.classList.toggle("dark-mode", wantDark);
      document.body.classList.toggle("light-mode", !wantDark);
    };

    sync();
    query.addEventListener("change", sync);

    const observer = new MutationObserver(() => {
      const wantDark = prefersDark();
      const isDark = document.body.classList.contains("dark-mode");
      if (isDark !== wantDark) sync();
    });
    observer.observe(document.body, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => {
      query.removeEventListener("change", sync);
      observer.disconnect();
    };
  }, []);

  return (
    <div className="spec-viewer">
      <ApiReferenceReact
        configuration={{
          content: spec,
          hideClientButton: true,
          hideDownloadButton: false,
          hideTestRequestButton: true,
          layout: "classic",
          darkMode: dark,
        }}
      />
    </div>
  );
}
