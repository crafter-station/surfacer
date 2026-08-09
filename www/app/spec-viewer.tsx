"use client";

import { ApiReferenceReact } from "@scalar/api-reference-react";
import "@scalar/api-reference-react/style.css";

/**
 * Read-only Scalar view of an emitted OpenAPI document.
 *
 * The SUNAT declaration endpoints need a real browser-captured token to answer,
 * so a live "Send request" here would only fail or ask for credentials. The view
 * exists to show the shape and the three auth states, not to fire requests, so
 * the interactive client is hidden.
 */
export function SpecViewer({ spec }: { spec: string }) {
  return (
    <div className="spec-viewer">
      <ApiReferenceReact
        configuration={{
          content: spec,
          hideClientButton: true,
          hideDownloadButton: false,
          hideTestRequestButton: true,
          layout: "classic",
          darkMode: true,
        }}
      />
    </div>
  );
}
