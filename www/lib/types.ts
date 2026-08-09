/**
 * Descriptor shapes, mirroring `crates/surfacer-ir`.
 *
 * Kept apart from the loader so client components can import types without
 * pulling `node:fs` into the browser bundle.
 */

export type OperationKind = "read" | "write" | "other";

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
  http?: { endpoints: HttpEndpoint[] };
  ax?: unknown;
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
