use probe_code::language::bash::BashLanguage;
use probe_code::language::c::CLanguage;
use probe_code::language::cpp::CppLanguage;
use probe_code::language::csharp::CSharpLanguage;
use probe_code::language::go::GoLanguage;
use probe_code::language::html::HtmlLanguage;
use probe_code::language::java::JavaLanguage;
use probe_code::language::javascript::JavaScriptLanguage;
use probe_code::language::language_trait::LanguageImpl;
use probe_code::language::markdown::MarkdownLanguage;
use probe_code::language::php::PhpLanguage;
use probe_code::language::python::PythonLanguage;
use probe_code::language::qml::QmlLanguage;
use probe_code::language::ruby::RubyLanguage;
use probe_code::language::rust::RustLanguage;
use probe_code::language::swift::SwiftLanguage;
use probe_code::language::typescript::TypeScriptLanguage;
use probe_code::language::yaml::YamlLanguage;

/// Bash-family shebang detection for extensionless files.
///
/// Mirrors the shebang handling in `lsp-daemon/src/language_detector.rs`,
/// restricted to the shell family: `#!/bin/sh`, `#!/bin/bash`,
/// `#!/usr/bin/env sh`, `#!/usr/bin/env bash`, including env-flag forms like
/// `#!/usr/bin/env -S bash -e`. Returns the synthetic extension `"sh"` so the
/// existing `get_language_impl` / `get_pooled_parser` plumbing works
/// unchanged. Non-shell interpreters (`python`, `node`, …) return `None`.
pub fn shebang_language_extension(first_line: &str) -> Option<&'static str> {
    let body = first_line.strip_prefix("#!")?;
    for token in body.split_whitespace() {
        let basename = token.rsplit('/').next().unwrap_or(token);
        match basename {
            "sh" | "bash" => return Some("sh"),
            // `/usr/bin/env bash`: keep scanning for the interpreter name.
            "env" => continue,
            // env options such as `-S` before the interpreter name.
            _ if token.starts_with('-') => continue,
            // The first real token is a different interpreter.
            _ => return None,
        }
    }
    None
}

/// Resolve the effective language extension for a file whose content (or at
/// least its first line) is already in memory: the real extension when the
/// path has one, otherwise a bash-family shebang sniff. Only extensionless
/// candidates are sniffed; binary and non-shebang extensionless files yield
/// `""`, exactly as before.
pub fn effective_extension<'a>(path: &'a std::path::Path, first_line: Option<&str>) -> &'a str {
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        return ext;
    }
    first_line
        .and_then(shebang_language_extension)
        .unwrap_or("")
}

/// Shebang sniff for extensionless files on paths whose content has not been
/// read yet (language-filter file enumeration). Reads at most the first 512
/// bytes; returns `None` for files with an extension, unreadable files,
/// non-UTF-8 (binary) files, and non-shell shebangs.
pub fn shebang_extension_for_path(path: &std::path::Path) -> Option<&'static str> {
    use std::io::{BufRead, Read};
    if path.extension().is_some() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut first_line = String::new();
    std::io::BufReader::new(file)
        .take(512)
        .read_line(&mut first_line)
        .ok()?;
    shebang_language_extension(&first_line)
}

/// Factory function to get the appropriate language implementation based on file extension
pub fn get_language_impl(extension: &str) -> Option<Box<dyn LanguageImpl>> {
    match extension {
        "rs" => Some(Box::new(RustLanguage::new())),
        "js" | "jsx" => Some(Box::new(JavaScriptLanguage::new())),
        "ts" => Some(Box::new(TypeScriptLanguage::new_typescript())),
        "tsx" => Some(Box::new(TypeScriptLanguage::new_tsx())),
        "py" => Some(Box::new(PythonLanguage::new())),
        "go" => Some(Box::new(GoLanguage::new())),
        "c" | "h" => Some(Box::new(CLanguage::new())),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(Box::new(CppLanguage::new())),
        "java" => Some(Box::new(JavaLanguage::new())),
        "rb" => Some(Box::new(RubyLanguage::new())),
        "php" => Some(Box::new(PhpLanguage::new())),
        "swift" => Some(Box::new(SwiftLanguage::new())),
        "cs" => Some(Box::new(CSharpLanguage::new())),
        "html" | "htm" => Some(Box::new(HtmlLanguage::new())),
        "md" | "markdown" => Some(Box::new(MarkdownLanguage::new())),
        "yaml" | "yml" => Some(Box::new(YamlLanguage::new())),
        "sh" | "bash" => Some(Box::new(BashLanguage::new())),
        "qml" => Some(Box::new(QmlLanguage::new())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shebang_detects_sh_and_bash() {
        assert_eq!(shebang_language_extension("#!/bin/sh"), Some("sh"));
        assert_eq!(shebang_language_extension("#!/bin/bash"), Some("sh"));
        assert_eq!(shebang_language_extension("#!/usr/bin/bash"), Some("sh"));
        assert_eq!(shebang_language_extension("#!/usr/bin/env sh"), Some("sh"));
        assert_eq!(
            shebang_language_extension("#!/usr/bin/env bash"),
            Some("sh")
        );
        assert_eq!(
            shebang_language_extension("#!/usr/bin/env -S bash -e"),
            Some("sh")
        );
        assert_eq!(shebang_language_extension("#!/bin/bash -euo pipefail"), Some("sh"));
        assert_eq!(
            shebang_language_extension("#!/bin/sh\r"),
            Some("sh"),
            "CRLF first line"
        );
    }

    #[test]
    fn shebang_rejects_non_shell_and_non_shebang() {
        assert_eq!(shebang_language_extension("#!/usr/bin/python3"), None);
        assert_eq!(shebang_language_extension("#!/usr/bin/env python"), None);
        assert_eq!(shebang_language_extension("#!/usr/bin/env node"), None);
        assert_eq!(shebang_language_extension("#!/bin/zsh"), None);
        assert_eq!(shebang_language_extension("#!/usr/bin/fish"), None);
        // `sh`/`bash` must be the full basename, not a substring.
        assert_eq!(shebang_language_extension("#!/bin/shx"), None);
        assert_eq!(shebang_language_extension("#!/opt/bashful"), None);
        // No shebang at all.
        assert_eq!(shebang_language_extension("echo hello"), None);
        assert_eq!(shebang_language_extension(""), None);
        // Bare env with no interpreter.
        assert_eq!(shebang_language_extension("#!/usr/bin/env"), None);
    }

    #[test]
    fn effective_extension_prefers_real_extension() {
        use std::path::Path;
        // Real extension wins; no sniffing needed.
        assert_eq!(
            effective_extension(Path::new("deploy.sh"), None),
            "sh"
        );
        assert_eq!(
            effective_extension(Path::new("deploy.sh"), Some("#!/usr/bin/python3")),
            "sh",
            "extension beats shebang"
        );
        // Extensionless + shell shebang -> synthetic "sh".
        assert_eq!(
            effective_extension(
                Path::new("bin/omarchy-menu"),
                Some("#!/usr/bin/env bash")
            ),
            "sh"
        );
        // Extensionless without shell shebang -> unaffected.
        assert_eq!(
            effective_extension(Path::new("bin/Makefile"), Some("all:")),
            ""
        );
        assert_eq!(effective_extension(Path::new("bin/tool"), None), "");
        assert_eq!(
            effective_extension(Path::new("bin/script"), Some("#!/usr/bin/python3")),
            ""
        );
    }
}
