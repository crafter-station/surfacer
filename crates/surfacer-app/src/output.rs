//! Output mode, shared by every command that reports something.
//!
//! surfacer emits CLIs that are expected to be agent-first, and the rules it
//! enforces on them apply to it too: a command's result belongs on stdout as
//! data, its narration belongs on stderr, and a caller that is not a terminal
//! gets machine output without having to ask. The same list is enforced for
//! both in `crates/surfacer-emit-cli/tests/agent_first_rules.rs`.
//!
//! The reason this is a module rather than a flag each command reads: the
//! decision has two independent inputs (an explicit `--json` and whether
//! stdout is a terminal), and a command that consults only the flag silently
//! breaks piping. Deciding once, here, is what keeps that from happening per
//! command.

use std::io::IsTerminal;

/// How a command should report its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A person is reading. Narrate to stderr, with color when allowed.
    Human,
    /// A machine is reading. One JSON document on stdout, nothing else.
    Json,
}

impl Mode {
    /// Resolve the output mode from the flag and the terminal.
    ///
    /// JSON when asked for it explicitly, and JSON when stdout is not a
    /// terminal even without the flag, because a piped invocation is a machine
    /// reading whether or not it knew to pass a flag.
    pub fn resolve(json_flag: bool) -> Self {
        if json_flag || !std::io::stdout().is_terminal() {
            Mode::Json
        } else {
            Mode::Human
        }
    }

    pub fn is_json(self) -> bool {
        self == Mode::Json
    }
}

/// Print one JSON document to stdout. This is the only thing that writes data
/// there; everything else in the CLI narrates to stderr.
pub fn emit_json(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
}

/// Whether ANSI color is allowed. Color is a property of the human stream, so
/// it follows stderr rather than stdout, and `NO_COLOR` disables it outright.
pub fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal()
}

/// Exit codes, so a caller can tell a bad request from a broken world.
///
/// An agent that cannot distinguish them retries the ones it should not: a
/// malformed IR will fail identically forever, while an unreachable host is
/// worth trying again.
pub mod exit {
    /// The command did what was asked.
    pub const OK: i32 = 0;
    /// The caller asked for something invalid: a malformed IR, an unknown
    /// site, a file that is not there. Retrying without changing the input
    /// produces the same result.
    pub const USER_ERROR: i32 = 1;
    /// Something outside the caller's control failed: a network error, a
    /// filesystem error, a target that did not answer. Retrying may work.
    pub const SYSTEM_ERROR: i32 = 2;
}

/// Classify a failure into one of the exit codes above.
///
/// The distinction is by cause, not by severity: an `io::Error` anywhere in
/// the chain means the filesystem or the network failed, which is worth
/// retrying, and everything else means the request itself was wrong, which is
/// not. Transport failures reach here wrapped as `io::Error`, so this covers
/// an unreachable host without taking a dependency on the HTTP client.
pub fn exit_code_for(error: &anyhow::Error) -> i32 {
    for cause in error.chain() {
        if cause.downcast_ref::<std::io::Error>().is_some() {
            return exit::SYSTEM_ERROR;
        }
    }
    exit::USER_ERROR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_flag_forces_json() {
        assert!(Mode::resolve(true).is_json());
    }

    #[test]
    fn a_transport_failure_is_a_system_error() {
        let io = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err = anyhow::Error::new(io).context("probing the target");
        assert_eq!(exit_code_for(&err), exit::SYSTEM_ERROR);
    }

    #[test]
    fn a_bad_request_is_a_user_error() {
        let err = anyhow::anyhow!("duplicate command path: invoice list");
        assert_eq!(exit_code_for(&err), exit::USER_ERROR);
    }

    #[test]
    fn the_three_codes_are_distinct() {
        // An agent branches on these, so collapsing any two silently changes
        // retry behavior at the call site.
        assert_ne!(exit::OK, exit::USER_ERROR);
        assert_ne!(exit::USER_ERROR, exit::SYSTEM_ERROR);
    }
}
