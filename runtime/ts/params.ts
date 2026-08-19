// Parameter validation for the surfacer TypeScript runtime.
//
// This logic was written inside the ts-cli emitter, which meant the MCP target
// shipped without it: the same descriptor, called through a tool instead of a
// CLI, sent a request with no parameters and got the target's error page back
// with a 200. Moving it here is the point of the shared runtime. One
// implementation, two callers, no drift.

export interface Param {
  name: string;
  example: string;
  /** How many observed requests to this endpoint carried the parameter. */
  observations: number;
  /** True when the value differed across observations, which is the strongest
   *  available evidence that the caller controls it. */
  varies: boolean;
}

export interface MissingParam {
  name: string;
  example: string;
  observedIn: number;
  callerControlled: boolean;
}

export interface MissingParamsError {
  error: "missing parameter";
  command: string;
  missing: MissingParam[];
  hint: string;
}

/**
 * The parameters an operation requires that the caller did not supply.
 *
 * The rule is evidence, not a contract: recon watched traffic and the target
 * publishes no spec, so "present in every observation" is the strongest thing
 * that can honestly be said. A parameter seen in some requests but not all is
 * reported by `schema` and never enforced here.
 *
 * Why this must fail before the fetch: without the check the target replies 200
 * with its own error page and the command exits zero, which reads as success to
 * whatever called it.
 */
export function missingParams(params: Param[], provided: Set<string>): Param[] {
  const out: Param[] = [];
  for (const param of params) {
    if (param.observations > 0 && !provided.has(param.name)) out.push(param);
  }
  return out;
}

/** The structured error body for a missing-parameter failure. */
export function missingParamsError(command: string, missing: Param[]): MissingParamsError {
  const hintParts: string[] = [];
  for (const param of missing) {
    hintParts.push(param.name + "=" + (param.example !== "" ? param.example : "<value>"));
  }

  return {
    error: "missing parameter",
    command: command,
    missing: missing.map((param) => ({
      name: param.name,
      example: param.example,
      observedIn: param.observations,
      callerControlled: param.varies,
    })),
    hint: command + " " + hintParts.join(" "),
  };
}

/**
 * EX_USAGE from sysexits, the exit code for a malformed invocation.
 *
 * Distinct from the 77 (EX_NOPERM) a blocked operation kind returns, so a
 * caller can tell "you called this wrong" from "the target refused".
 */
export const EX_USAGE = 64;
