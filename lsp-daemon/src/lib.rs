// LSP Daemon Library
// Exports public interfaces for client implementations

// Core modules
pub mod protocol;
pub mod ipc;
pub mod socket_path;
pub mod language_detector;

// Internal modules - exposed for direct client use
pub mod lsp_server;
pub mod lsp_registry;

// Internal modules (not exposed)
mod daemon;
mod pool;

// Re-export commonly used types
pub use protocol::{
    DaemonRequest, DaemonResponse, CallHierarchyResult, CallHierarchyItem, 
    DaemonStatus, LanguageInfo, PoolStatus, MessageCodec,
    parse_call_hierarchy_from_lsp,
};

pub use language_detector::{Language, LanguageDetector};
pub use socket_path::{get_default_socket_path, normalize_executable, remove_socket_file};
pub use ipc::{IpcStream, IpcListener};

// Re-export daemon for binary use
pub use daemon::{LspDaemon, start_daemon_background};