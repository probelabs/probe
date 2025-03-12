// Chat module components
mod cli;
mod errors;
mod history;
mod models;
mod probe_chat;
mod tools;

// Re-export the main chat function for use in lib.rs
#[allow(unused_imports)]
pub use cli::handle_chat;
