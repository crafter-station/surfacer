//! Browser-backed capture for surfacer.
//!
//! This crate no longer discovers a surface. Recon is done by an agent running
//! the `surfacer-recon` skill, which emits the IR directly ("bring your own
//! IR"). What remains here is runtime: driving a browser to acquire and hold
//! credentials so `surfacer exec` can reach an authenticated surface.

pub mod agent_browser;
pub mod capture;
pub mod har;
pub mod overlay;
pub mod paths;
pub mod token_capture;

pub use agent_browser::{BrowserProcess, ProbeSession};
pub use capture::*;
pub use overlay::ProbeOverlayEvent;
pub use token_capture::extract_token;
