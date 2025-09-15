//! Mock LSP server infrastructure for testing LSP daemon integration
//!
//! This module provides mock implementations of various language servers
//! with configurable response patterns for testing purposes.

pub mod gopls_mock;
pub mod protocol;
pub mod pylsp_mock;
pub mod rust_analyzer_mock;
pub mod server;
pub mod tsserver_mock;
