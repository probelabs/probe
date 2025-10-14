use once_cell::sync::Lazy;
use std::collections::HashSet;

use crate::language_detector::Language;

// Base default sets
static RUST_IMPL_NAMES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "default",
        "clone",
        "copy",
        "debug",
        "display",
        "from",
        "into",
        "asref",
        "asmut",
        "deref",
        "derefmut",
        "partialeq",
        "eq",
        "partialord",
        "ord",
        "hash",
        "send",
        "sync",
        "unpin",
        "sized",
        "borrow",
        "borrowmut",
        "toowned",
        "tryfrom",
        "tryinto",
    ])
});

static RUST_REF_NAMES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    // Clone impl list and add common trait method names that explode (e.g., fmt)
    let mut s: HashSet<&'static str> = RUST_IMPL_NAMES.iter().copied().collect();
    s.insert("fmt");
    s
});

static JS_CORE_TYPES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "array", "promise", "map", "set", "weakmap", "weakset", "object", "string", "number",
        "boolean", "symbol", "bigint", "date", "regexp", "error",
    ])
});

static JS_CORE_METHODS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "tostring",
        "valueof",
        "constructor",
        "map",
        "filter",
        "reduce",
        "foreach",
        "keys",
        "values",
        "entries",
        "includes",
        "push",
        "pop",
        "shift",
        "unshift",
        "splice",
        "concat",
        "slice",
        "then",
        "catch",
        "finally",
        "get",
        "set",
        "has",
        "add",
        "delete",
        "clear",
        "apply",
        "call",
        "bind",
    ])
});

fn load_augmented(
    base: &HashSet<&'static str>,
    env_add: &str,
    env_remove: &str,
) -> HashSet<String> {
    let mut set: HashSet<String> = base.iter().map(|s| (*s).to_string()).collect();
    if let Ok(add) = std::env::var(env_add) {
        for t in add.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            set.insert(t.to_ascii_lowercase());
        }
    }
    if let Ok(remove) = std::env::var(env_remove) {
        for t in remove
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            set.remove(&t.to_ascii_lowercase());
        }
    }
    set
}

fn normalized(name: &str) -> String {
    name.to_ascii_lowercase()
}

pub fn should_skip_impls(language: Language, name: &str, kind: &str) -> bool {
    // Global disable
    if std::env::var("PROBE_LSP_IMPL_SKIP_CORE")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return false;
    }

    let n = normalized(name);
    match language {
        Language::Rust => {
            let set = load_augmented(
                &RUST_IMPL_NAMES,
                "PROBE_LSP_SKIPLIST_RUST_IMPLS_ADD",
                "PROBE_LSP_SKIPLIST_RUST_IMPLS_REMOVE",
            );
            set.contains(&n)
        }
        Language::JavaScript | Language::TypeScript => {
            let types = load_augmented(
                &JS_CORE_TYPES,
                "PROBE_LSP_SKIPLIST_JS_TYPES_ADD",
                "PROBE_LSP_SKIPLIST_JS_TYPES_REMOVE",
            );
            let methods = load_augmented(
                &JS_CORE_METHODS,
                "PROBE_LSP_SKIPLIST_JS_METHODS_ADD",
                "PROBE_LSP_SKIPLIST_JS_METHODS_REMOVE",
            );
            if kind.eq_ignore_ascii_case("interface") || kind.eq_ignore_ascii_case("class") {
                types.contains(&n)
            } else if kind.eq_ignore_ascii_case("method") || kind.eq_ignore_ascii_case("function") {
                methods.contains(&n)
            } else {
                // Fall back: match either set
                types.contains(&n) || methods.contains(&n)
            }
        }
        _ => false,
    }
}

pub fn should_skip_refs(language: Language, name: &str, kind: &str) -> bool {
    if std::env::var("PROBE_LSP_REFS_SKIP_CORE")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return false;
    }

    let n = normalized(name);
    match language {
        Language::Rust => {
            let set = load_augmented(
                &RUST_REF_NAMES,
                "PROBE_LSP_SKIPLIST_RUST_REFS_ADD",
                "PROBE_LSP_SKIPLIST_RUST_REFS_REMOVE",
            );
            set.contains(&n)
        }
        Language::JavaScript | Language::TypeScript => {
            // By default, do not skip refs as aggressively; allow env to add patterns.
            let types = load_augmented(
                &JS_CORE_TYPES,
                "PROBE_LSP_SKIPLIST_JS_REFS_TYPES_ADD",
                "PROBE_LSP_SKIPLIST_JS_REFS_TYPES_REMOVE",
            );
            let methods = load_augmented(
                &JS_CORE_METHODS,
                "PROBE_LSP_SKIPLIST_JS_REFS_METHODS_ADD",
                "PROBE_LSP_SKIPLIST_JS_REFS_METHODS_REMOVE",
            );
            if kind.eq_ignore_ascii_case("interface") || kind.eq_ignore_ascii_case("class") {
                types.contains(&n)
            } else if kind.eq_ignore_ascii_case("method") || kind.eq_ignore_ascii_case("function") {
                methods.contains(&n)
            } else {
                false
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_skiplist_matches_core() {
        assert!(should_skip_impls(Language::Rust, "Default", "trait"));
        assert!(should_skip_refs(Language::Rust, "fmt", "method"));
        assert!(!should_skip_impls(Language::Rust, "QueryPlan", "struct"));
    }

    #[test]
    fn js_skiplist_matches_core() {
        assert!(should_skip_impls(Language::JavaScript, "Array", "class"));
        assert!(should_skip_impls(
            Language::TypeScript,
            "toString",
            "method"
        ));
        assert!(!should_skip_impls(
            Language::TypeScript,
            "CustomType",
            "interface"
        ));
    }
}
