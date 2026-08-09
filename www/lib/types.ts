/**
 * Descriptor shapes, mirroring `crates/surfacer-ir`.
 *
 * Kept apart from the loader so client components can import types without
 * pulling `node:fs` into the browser bundle.
 */

export type OperationKind = "read" | "write" | "other";

/**
 * Auth modes, mirroring `crates/surfacer-ir/src/auth.rs`. Only the fields the
 * OpenAPI emitter reads are typed here; the rest ride through as unknown.
 */
export type AuthMode =
  | { kind: "none" }
  | {
      kind: "apiKey";
      location: "header" | "query";
      name: string;
      valuePrefix?: string;
    }
  | {
      kind: "oAuth2";
      grant: "password" | "clientCredentials" | "authorizationCode";
      tokenUrl: string;
      scopes?: string[];
    }
  | {
      kind: "browserBootstrappedToken";
      acquire: { loginUrl: string };
      use: { location: "header" | "query"; name: string };
    };

export interface ParamDescriptor {
  name: string;
  varies?: boolean;
  example?: string;
  observations?: number;
}

export interface HttpEndpoint {
  namespace: string[];
  method: string;
  path: string;
  description: string;
  operationKind: OperationKind;
  params?: ParamDescriptor[];
}

export interface OperationDescriptor {
  commandPath: string[];
  summary: string;
  description: string;
  operationKind: OperationKind;
  transport: { kind: string; endpointIndex?: number; actionIndex?: number };
  auth?: AuthMode;
}

export interface SiteDescriptor {
  meta: {
    siteName: string;
    displayName: string;
    sourceUrl: string;
    irVersion: string;
  };
  provenance: {
    generatedAt: string;
    technique: string;
    classifierBucket: string;
    probeDurationSec: number;
  };
  operations: OperationDescriptor[];
  http?: { endpoints: HttpEndpoint[]; auth?: AuthMode };
  ax?: unknown;
}

/**
 * Effective auth for an operation: its own override, else the surface default.
 * Mirrors `resolve_auth` in the Rust IR.
 */
export function resolveAuth(
  descriptor: SiteDescriptor,
  op: OperationDescriptor,
): AuthMode | undefined {
  return op.auth ?? descriptor.http?.auth;
}

export interface Example {
  slug: string;
  descriptor: SiteDescriptor;
  bytes: number;
}

/** Resolve the endpoint an operation points at, when it has one. */
export function endpointFor(
  descriptor: SiteDescriptor,
  op: OperationDescriptor,
): HttpEndpoint | undefined {
  if (op.transport.kind !== "http") return undefined;
  const index = op.transport.endpointIndex;
  if (index === undefined) return undefined;
  return descriptor.http?.endpoints[index];
}

export function paramsFor(
  descriptor: SiteDescriptor,
  op: OperationDescriptor,
): ParamDescriptor[] {
  return endpointFor(descriptor, op)?.params ?? [];
}
