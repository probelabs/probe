use once_cell::sync::Lazy;
use std::collections::HashSet;

/// Static set of special case terms that should be treated as exceptions
/// These terms are used in compound word detection and special handling
pub static EXCEPTION_TERMS: Lazy<HashSet<String>> = Lazy::new(|| {
    vec![
        // Network and security related terms
        "network",
        "firewall",
        // Common technology terms
        "rpc",
        "api",
        "http",
        "json",
        "xml",
        "html",
        "css",
        "js",
        "db",
        "sql",
        // Common software architecture terms
        "handler",
        "controller",
        "service",
        "repository",
        "manager",
        "factory",
        "provider",
        "client",
        "server",
        "config",
        "util",
        "helper",
        "storage",
        "cache",
        "queue",
        "worker",
        "job",
        "task",
        "event",
        "listener",
        "callback",
        "middleware",
        "filter",
        "validator",
        "converter",
        "transformer",
        "parser",
        "serializer",
        "deserializer",
        "encoder",
        "decoder",
        "reader",
        "writer",
    ]
    .into_iter()
    .map(String::from)
    .collect()
});

/// Checks if a term is in the exception list
pub fn is_exception_term(term: &str) -> bool {
    EXCEPTION_TERMS.contains(&term.to_lowercase())
}
