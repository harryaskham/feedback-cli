//! `mcp` re-exports the feedback tool registrar in a dedicated module so host
//! crates can write `use feedback_cli::mcp::register_feedback_tools;` without
//! reaching into the crate root, mirroring the `updatable-cli` layout.

pub use crate::register_feedback_tools;
