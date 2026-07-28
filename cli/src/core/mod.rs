//! UI-agnostic operations that make up the tool: download the seed, extract its
//! disk, import into Tart, and run it. First-boot provisioning is still to come.
//! Nothing here prints or draws — front-ends (the clap CLI now, a ratatui TUI
//! later) call these and render progress themselves.

pub mod download;
pub mod extract;
pub mod tart;
