//! UI-agnostic operations that make up the tool: download, and (to come)
//! extract, import into Tart, and first-boot provisioning. Nothing here
//! prints or draws — front-ends (the clap CLI now, a ratatui TUI later)
//! call these and render progress themselves.

pub mod download;
