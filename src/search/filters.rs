use glob::Pattern;
use std::collections::HashSet;
use std::path::Path;

/// Search filters extracted from query hints like file:, ext:, type:, etc.
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// File path patterns (from file: and path: hints)
    pub file_patterns: Vec<String>,
    /// File extensions (from ext: hints)
    pub extensions: Vec<String>,
    /// File types using ripgrep type definitions (from type: hints)
    pub file_types: Vec<String>,
    /// Directory patterns (from dir: hints)
    pub dir_patterns: Vec<String>,
    /// Programming languages (from lang: hints)
    pub languages: Vec<String>,
    /// Exact filenames (from filename: hints or auto-detected)
    pub exact_filenames: Vec<String>,
}

impl SearchFilters {
    /// Create a new empty SearchFilters instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any filters are active
    pub fn is_empty(&self) -> bool {
        self.file_patterns.is_empty()
            && self.extensions.is_empty()
            && self.file_types.is_empty()
            && self.dir_patterns.is_empty()
            && self.languages.is_empty()
            && self.exact_filenames.is_empty()
    }

    /// Add a filter based on field name and values
    pub fn add_filter(&mut self, field_name: &str, values: Vec<String>) {
        match field_name.to_lowercase().as_str() {
            "file" | "path" => {
                self.file_patterns.extend(values);
            }
            "filename" => {
                // Exact filename matching
                self.exact_filenames.extend(values);
            }
            "ext" | "extension" => {
                // Split comma-separated values and normalize extensions
                for value in values {
                    for ext in value.split(',') {
                        let ext = ext.trim();
                        if !ext.is_empty() {
                            // Remove leading dot if present
                            let normalized = if let Some(stripped) = ext.strip_prefix('.') {
                                stripped.to_string()
                            } else {
                                ext.to_string()
                            };
                            self.extensions.push(normalized.to_lowercase());
                        }
                    }
                }
            }
            "type" => {
                // Split comma-separated values
                for value in values {
                    for file_type in value.split(',') {
                        let file_type = file_type.trim();
                        if !file_type.is_empty() {
                            self.file_types.push(file_type.to_lowercase());
                        }
                    }
                }
            }
            "dir" | "directory" => {
                self.dir_patterns.extend(values);
            }
            "lang" | "language" => {
                // Split comma-separated values and normalize language names
                for value in values {
                    for lang in value.split(',') {
                        let lang = lang.trim();
                        if !lang.is_empty() {
                            self.languages.push(normalize_language_name(lang));
                        }
                    }
                }
            }
            _ => {
                // Unknown filter type - ignore or log warning
                eprintln!("Warning: Unknown filter type '{}'", field_name);
            }
        }
    }

    /// Check if a file path matches all active filters
    pub fn matches_file(&self, path: &Path) -> bool {
        // Check exact filenames first (most specific)
        if !self.exact_filenames.is_empty() {
            if let Some(filename) = path.file_name() {
                let filename_str = filename.to_string_lossy();
                if self.exact_filenames.iter().any(|f| {
                    filename_str == f.as_str() || filename_str.eq_ignore_ascii_case(f.as_str())
                }) {
                    return true; // Found exact match, no need to check other filters
                }
            }
            // If exact filenames specified but no match, file doesn't match
            return false;
        }

        // Check file extensions
        if !self.extensions.is_empty() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if !self.extensions.contains(&ext_str) {
                    return false;
                }
            } else {
                return false; // No extension, but extension filter specified
            }
        }

        // Check file patterns (glob matching)
        if !self.file_patterns.is_empty() {
            let path_str = path.to_string_lossy();
            let matches_pattern = self.file_patterns.iter().any(|pattern| {
                match Pattern::new(pattern) {
                    Ok(glob_pattern) => glob_pattern.matches(&path_str),
                    Err(_) => {
                        // If pattern is invalid, fall back to simple substring matching
                        path_str.contains(pattern)
                    }
                }
            });
            if !matches_pattern {
                return false;
            }
        }

        // Check directory patterns
        if !self.dir_patterns.is_empty() {
            let _path_str = path.to_string_lossy();
            let matches_dir = self.dir_patterns.iter().any(|pattern| {
                // Check if any parent directory matches the pattern
                if let Some(parent) = path.parent() {
                    let parent_str = parent.to_string_lossy();
                    match Pattern::new(pattern) {
                        Ok(glob_pattern) => {
                            glob_pattern.matches(&parent_str) || parent_str.contains(pattern)
                        }
                        Err(_) => parent_str.contains(pattern),
                    }
                } else {
                    false
                }
            });
            if !matches_dir {
                return false;
            }
        }

        // Check file types (map to extensions)
        if !self.file_types.is_empty() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                let matches_type = self.file_types.iter().any(|file_type| {
                    match get_extensions_for_type(file_type) {
                        Some(extensions) => extensions.contains(&ext_str),
                        None => false,
                    }
                });
                if !matches_type {
                    return false;
                }
            } else {
                return false; // No extension, but file type filter specified
            }
        }

        // Check languages (map to extensions)
        if !self.languages.is_empty() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                let matches_lang =
                    self.languages
                        .iter()
                        .any(|lang| match get_extensions_for_language(lang) {
                            Some(extensions) => extensions.contains(&ext_str),
                            None => false,
                        });
                if !matches_lang {
                    return false;
                }
            } else {
                return false; // No extension, but language filter specified
            }
        }

        true
    }

    /// Extract filters from an AST and return a simplified AST without filter terms
    pub fn extract_and_simplify(
        ast: crate::search::elastic_query::Expr,
    ) -> (Self, Option<crate::search::elastic_query::Expr>) {
        let mut filters = SearchFilters::new();
        let simplified_ast = simplify_ast(ast, &mut filters);
        (filters, simplified_ast)
    }

    /// Extract filters with auto-detection of filename-like terms
    pub fn extract_and_simplify_with_autodetect(
        ast: crate::search::elastic_query::Expr,
    ) -> (Self, Option<crate::search::elastic_query::Expr>) {
        let mut filters = SearchFilters::new();
        let simplified_ast = simplify_ast_with_autodetect(ast, &mut filters);
        (filters, simplified_ast)
    }
}

/// Simplify AST by extracting filter terms and removing them
fn simplify_ast(
    expr: crate::search::elastic_query::Expr,
    filters: &mut SearchFilters,
) -> Option<crate::search::elastic_query::Expr> {
    use crate::search::elastic_query::Expr;

    match expr {
        Expr::Term {
            field: Some(field_name),
            keywords,
            required: _,
            excluded: _,
            exact: _,
            ..
        } if is_filter_field(&field_name) => {
            // This is a filter term - extract it and return None to remove from AST
            filters.add_filter(&field_name, keywords);
            None
        }
        Expr::Term { .. } => {
            // Regular search term - keep it
            Some(expr)
        }
        Expr::And(left, right) => {
            let left_simplified = simplify_ast(*left, filters);
            let right_simplified = simplify_ast(*right, filters);

            match (left_simplified, right_simplified) {
                (Some(l), Some(r)) => Some(Expr::And(Box::new(l), Box::new(r))),
                (Some(expr), None) | (None, Some(expr)) => Some(expr),
                (None, None) => None,
            }
        }
        Expr::Or(left, right) => {
            let left_simplified = simplify_ast(*left, filters);
            let right_simplified = simplify_ast(*right, filters);

            match (left_simplified, right_simplified) {
                (Some(l), Some(r)) => Some(Expr::Or(Box::new(l), Box::new(r))),
                (Some(expr), None) | (None, Some(expr)) => Some(expr),
                (None, None) => None,
            }
        }
    }
}

/// Simplify AST with auto-detection of filename-like terms
fn simplify_ast_with_autodetect(
    expr: crate::search::elastic_query::Expr,
    filters: &mut SearchFilters,
) -> Option<crate::search::elastic_query::Expr> {
    use crate::search::elastic_query::Expr;

    match expr {
        Expr::Term {
            field: Some(field_name),
            keywords,
            required,
            excluded,
            exact,
            ..
        } => {
            if is_filter_field(&field_name) {
                // This is a filter term - extract it and return None to remove from AST
                filters.add_filter(&field_name, keywords);
                None
            } else {
                // Not a recognized filter field - keep it
                Some(Expr::Term {
                    lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
                    field: Some(field_name),
                    keywords,
                    required,
                    excluded,
                    exact,
                })
            }
        }
        Expr::Term {
            field: None,
            keywords,
            required,
            excluded,
            exact,
            ..
        } => {
            // Check if all keywords look like filenames
            let all_filename_like =
                !keywords.is_empty() && keywords.iter().all(|kw| is_filename_like(kw));

            if all_filename_like && !excluded && !required {
                // Auto-detect as filename filter
                filters.add_filter("filename", keywords);
                None
            } else {
                // Regular search term - keep it
                Some(Expr::Term {
                    lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
                    field: None,
                    keywords,
                    required,
                    excluded,
                    exact,
                })
            }
        }
        Expr::And(left, right) => {
            let left_simplified = simplify_ast_with_autodetect(*left, filters);
            let right_simplified = simplify_ast_with_autodetect(*right, filters);

            match (left_simplified, right_simplified) {
                (Some(l), Some(r)) => Some(Expr::And(Box::new(l), Box::new(r))),
                (Some(expr), None) | (None, Some(expr)) => Some(expr),
                (None, None) => None,
            }
        }
        Expr::Or(left, right) => {
            let left_simplified = simplify_ast_with_autodetect(*left, filters);
            let right_simplified = simplify_ast_with_autodetect(*right, filters);

            match (left_simplified, right_simplified) {
                (Some(l), Some(r)) => Some(Expr::Or(Box::new(l), Box::new(r))),
                (Some(expr), None) | (None, Some(expr)) => Some(expr),
                (None, None) => None,
            }
        }
    }
}

/// Check if a field name is a recognized filter field
fn is_filter_field(field_name: &str) -> bool {
    matches!(
        field_name.to_lowercase().as_str(),
        "file"
            | "path"
            | "filename"
            | "ext"
            | "extension"
            | "type"
            | "dir"
            | "directory"
            | "lang"
            | "language"
    )
}

/// Check if a term looks like a filename
/// A term is considered filename-like if it:
/// - Contains a file extension (e.g., .txt, .rs, .json)
/// - Doesn't contain spaces (unless quoted)
/// - Has a reasonable filename structure
pub fn is_filename_like(term: &str) -> bool {
    // Empty or whitespace-only strings are not filenames
    if term.trim().is_empty() {
        return false;
    }

    // Check for common filename extensions
    let common_extensions = [
        ".txt",
        ".md",
        ".rs",
        ".js",
        ".ts",
        ".py",
        ".java",
        ".c",
        ".cpp",
        ".h",
        ".go",
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".xml",
        ".html",
        ".css",
        ".scss",
        ".sass",
        ".sh",
        ".bash",
        ".zsh",
        ".fish",
        ".rb",
        ".php",
        ".swift",
        ".kt",
        ".scala",
        ".sql",
        ".csv",
        ".log",
        ".conf",
        ".config",
        ".env",
        ".gitignore",
        ".dockerfile",
        ".makefile",
        ".cmake",
        ".gradle",
        ".properties",
        ".ini",
        ".cfg",
    ];

    // Check if term has a recognized file extension
    let term_lower = term.to_lowercase();
    if common_extensions
        .iter()
        .any(|ext| term_lower.ends_with(ext))
    {
        return true;
    }

    // Check for dotfiles (e.g., .gitignore, .env)
    if term.starts_with('.') && !term.contains('/') && term.len() > 1 {
        return true;
    }

    // Check for common filename patterns without extension
    // (e.g., Makefile, Dockerfile, README)
    let common_files = [
        "makefile",
        "dockerfile",
        "readme",
        "license",
        "changelog",
        "contributing",
        "codeowners",
        "authors",
        "notice",
        "cargo.toml",
        "package.json",
    ];
    if common_files
        .iter()
        .any(|f| term_lower == *f || term_lower.starts_with(f))
    {
        return true;
    }

    false
}

/// Normalize language names to standard forms
fn normalize_language_name(lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "rs" => "rust".to_string(),
        "js" | "jsx" => "javascript".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "py" => "python".to_string(),
        "rb" => "ruby".to_string(),
        "cs" => "csharp".to_string(),
        "cpp" | "cc" | "cxx" => "cpp".to_string(),
        "h" | "hpp" | "hxx" => "c".to_string(),
        other => other.to_string(),
    }
}

/// Get file extensions for a given file type (ripgrep-style types)
fn get_extensions_for_type(file_type: &str) -> Option<HashSet<String>> {
    let mut extensions = HashSet::new();

    match file_type.to_lowercase().as_str() {
        "rust" => {
            extensions.insert("rs".to_string());
        }
        "js" | "javascript" => {
            extensions.insert("js".to_string());
            extensions.insert("jsx".to_string());
            extensions.insert("mjs".to_string());
            extensions.insert("cjs".to_string());
        }
        "ts" | "typescript" => {
            extensions.insert("ts".to_string());
            extensions.insert("tsx".to_string());
        }
        "python" | "py" => {
            extensions.insert("py".to_string());
            extensions.insert("pyi".to_string());
            extensions.insert("pyw".to_string());
        }
        "java" => {
            extensions.insert("java".to_string());
        }
        "c" => {
            extensions.insert("c".to_string());
            extensions.insert("h".to_string());
        }
        "cpp" | "cxx" => {
            extensions.insert("cpp".to_string());
            extensions.insert("cxx".to_string());
            extensions.insert("cc".to_string());
            extensions.insert("hpp".to_string());
            extensions.insert("hxx".to_string());
        }
        "go" => {
            extensions.insert("go".to_string());
        }
        "ruby" | "rb" => {
            extensions.insert("rb".to_string());
            extensions.insert("rake".to_string());
        }
        "php" => {
            extensions.insert("php".to_string());
        }
        "swift" => {
            extensions.insert("swift".to_string());
        }
        "kotlin" => {
            extensions.insert("kt".to_string());
            extensions.insert("kts".to_string());
        }
        "scala" => {
            extensions.insert("scala".to_string());
        }
        "html" => {
            extensions.insert("html".to_string());
            extensions.insert("htm".to_string());
        }
        "css" => {
            extensions.insert("css".to_string());
        }
        "json" => {
            extensions.insert("json".to_string());
        }
        "yaml" | "yml" => {
            extensions.insert("yaml".to_string());
            extensions.insert("yml".to_string());
        }
        "toml" => {
            extensions.insert("toml".to_string());
        }
        "xml" => {
            extensions.insert("xml".to_string());
        }
        "md" | "markdown" => {
            extensions.insert("md".to_string());
            extensions.insert("markdown".to_string());
        }
        _ => return None,
    }

    Some(extensions)
}

/// Get file extensions for a given programming language
fn get_extensions_for_language(lang: &str) -> Option<HashSet<String>> {
    // For simplicity, map languages to types
    get_extensions_for_type(lang)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_empty_filters() {
        let filters = SearchFilters::new();
        assert!(filters.is_empty());

        let path = PathBuf::from("src/main.rs");
        assert!(filters.matches_file(&path));
    }

    #[test]
    fn test_extension_filter() {
        let mut filters = SearchFilters::new();
        filters.add_filter("ext", vec!["rs".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(!filters.matches_file(&PathBuf::from("src/main.js")));
        assert!(!filters.matches_file(&PathBuf::from("README")));
    }

    #[test]
    fn test_multiple_extensions() {
        let mut filters = SearchFilters::new();
        filters.add_filter("ext", vec!["rs,js,ts".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(filters.matches_file(&PathBuf::from("src/main.js")));
        assert!(filters.matches_file(&PathBuf::from("src/main.ts")));
        assert!(!filters.matches_file(&PathBuf::from("src/main.py")));
    }

    #[test]
    fn test_file_pattern_filter() {
        let mut filters = SearchFilters::new();
        filters.add_filter("file", vec!["src/**/*.rs".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(filters.matches_file(&PathBuf::from("src/lib/helper.rs")));
        assert!(!filters.matches_file(&PathBuf::from("tests/main.rs")));
    }

    #[test]
    fn test_type_filter() {
        let mut filters = SearchFilters::new();
        filters.add_filter("type", vec!["rust".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(!filters.matches_file(&PathBuf::from("src/main.js")));
    }

    #[test]
    fn test_language_filter() {
        let mut filters = SearchFilters::new();
        filters.add_filter("lang", vec!["rust".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(!filters.matches_file(&PathBuf::from("src/main.js")));
    }

    #[test]
    fn test_directory_filter() {
        let mut filters = SearchFilters::new();
        filters.add_filter("dir", vec!["src".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(filters.matches_file(&PathBuf::from("src/lib/helper.rs")));
        assert!(!filters.matches_file(&PathBuf::from("tests/main.rs")));
    }

    #[test]
    fn test_combined_filters() {
        let mut filters = SearchFilters::new();
        filters.add_filter("ext", vec!["rs".to_string()]);
        filters.add_filter("dir", vec!["src".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("src/main.rs")));
        assert!(!filters.matches_file(&PathBuf::from("src/main.js"))); // Wrong extension
        assert!(!filters.matches_file(&PathBuf::from("tests/main.rs"))); // Wrong directory
    }

    #[test]
    fn test_normalize_language_names() {
        assert_eq!(normalize_language_name("rs"), "rust");
        assert_eq!(normalize_language_name("js"), "javascript");
        assert_eq!(normalize_language_name("ts"), "typescript");
        assert_eq!(normalize_language_name("py"), "python");
    }

    #[test]
    fn test_is_filter_field() {
        assert!(is_filter_field("file"));
        assert!(is_filter_field("ext"));
        assert!(is_filter_field("type"));
        assert!(is_filter_field("lang"));
        assert!(is_filter_field("dir"));
        assert!(is_filter_field("filename"));
        assert!(!is_filter_field("content"));
        assert!(!is_filter_field("random"));
    }

    #[test]
    fn test_exact_filename_filter() {
        let mut filters = SearchFilters::new();
        filters.add_filter("filename", vec!["SWE_TASK.txt".to_string()]);

        assert!(filters.matches_file(&PathBuf::from("SWE_TASK.txt")));
        assert!(filters.matches_file(&PathBuf::from("src/SWE_TASK.txt")));
        assert!(filters.matches_file(&PathBuf::from("./SWE_TASK.txt")));
        // Case insensitive matching
        assert!(filters.matches_file(&PathBuf::from("swe_task.txt")));
        assert!(!filters.matches_file(&PathBuf::from("OTHER_FILE.txt")));
        assert!(!filters.matches_file(&PathBuf::from("SWE_TASK.md")));
    }

    #[test]
    fn test_multiple_exact_filenames() {
        let mut filters = SearchFilters::new();
        filters.add_filter(
            "filename",
            vec![
                "SWE_TASK.txt".to_string(),
                "swebench_problem.json".to_string(),
            ],
        );

        assert!(filters.matches_file(&PathBuf::from("SWE_TASK.txt")));
        assert!(filters.matches_file(&PathBuf::from("swebench_problem.json")));
        assert!(filters.matches_file(&PathBuf::from("src/SWE_TASK.txt")));
        assert!(filters.matches_file(&PathBuf::from("data/swebench_problem.json")));
        assert!(!filters.matches_file(&PathBuf::from("other.txt")));
    }

    #[test]
    fn test_is_filename_like() {
        // Files with common extensions
        assert!(is_filename_like("SWE_TASK.txt"));
        assert!(is_filename_like("swebench_problem.json"));
        assert!(is_filename_like("main.rs"));
        assert!(is_filename_like("index.js"));
        assert!(is_filename_like("data.csv"));
        assert!(is_filename_like("config.yaml"));
        assert!(is_filename_like("README.md"));

        // Dotfiles
        assert!(is_filename_like(".gitignore"));
        assert!(is_filename_like(".env"));

        // Common files without extensions
        assert!(is_filename_like("Makefile"));
        assert!(is_filename_like("Dockerfile"));
        assert!(is_filename_like("README"));

        // Not filenames
        assert!(!is_filename_like("error"));
        assert!(!is_filename_like("function"));
        assert!(!is_filename_like("search term"));
        assert!(!is_filename_like(""));
        assert!(!is_filename_like("   "));
    }

    #[test]
    fn test_exact_filename_takes_precedence() {
        let mut filters = SearchFilters::new();
        filters.add_filter("filename", vec!["main.rs".to_string()]);
        // Also add extension filter that would normally reject this
        filters.add_filter("ext", vec!["js".to_string()]);

        // Exact filename should match even though extension filter says .js only
        assert!(filters.matches_file(&PathBuf::from("main.rs")));
    }

    #[test]
    fn test_filename_filter_integration_with_ast() {
        use crate::search::elastic_query::parse_query;

        // Test explicit filename: directive with quoted term to prevent tokenization
        let ast = parse_query("filename:\"SWE_TASK.txt\"", false).unwrap();
        let (filters, simplified) = SearchFilters::extract_and_simplify_with_autodetect(ast);

        // The filename directive should extract the filename
        // Note: Without quotes, parser splits on _ into multiple keywords
        assert!(!filters.exact_filenames.is_empty());
        assert!(simplified.is_none()); // Filter was extracted, no content search

        // Test auto-detection with quoted term
        let ast2 = parse_query("\"SWE_TASK.txt\"", false).unwrap();
        let (filters2, simplified2) = SearchFilters::extract_and_simplify_with_autodetect(ast2);

        // Should detect as filename
        assert_eq!(filters2.exact_filenames.len(), 1);
        assert_eq!(filters2.exact_filenames[0], "SWE_TASK.txt");
        assert!(simplified2.is_none());
    }

    #[test]
    fn test_filename_with_or_query() {
        use crate::search::elastic_query::parse_query;

        // "SWE_TASK.txt OR swebench_problem.json" - use quotes to preserve filenames
        let ast = parse_query("\"SWE_TASK.txt\" OR \"swebench_problem.json\"", false).unwrap();
        let (filters, simplified) = SearchFilters::extract_and_simplify_with_autodetect(ast);

        // Both should be detected as filenames
        assert_eq!(filters.exact_filenames.len(), 2);
        assert!(filters
            .exact_filenames
            .contains(&"SWE_TASK.txt".to_string()));
        assert!(filters
            .exact_filenames
            .contains(&"swebench_problem.json".to_string()));
        assert!(simplified.is_none());
    }

    #[test]
    fn test_filename_with_and_content_query() {
        use crate::search::elastic_query::parse_query;

        // "SWE_TASK.txt AND error" - should search for "error" in SWE_TASK.txt
        let ast = parse_query("\"SWE_TASK.txt\" AND error", false).unwrap();
        let (filters, simplified) = SearchFilters::extract_and_simplify_with_autodetect(ast);

        // Filename should be extracted as filter
        assert_eq!(filters.exact_filenames.len(), 1);
        assert_eq!(filters.exact_filenames[0], "SWE_TASK.txt");

        // "error" should remain in the AST for content search
        assert!(simplified.is_some());
        use crate::search::elastic_query::Expr;
        if let Some(Expr::Term { keywords, .. }) = simplified {
            assert_eq!(keywords.len(), 1);
            assert_eq!(keywords[0], "error");
        } else {
            panic!("Expected Term node with 'error' keyword");
        }
    }

    #[test]
    fn test_mixed_filename_and_regular_terms() {
        use crate::search::elastic_query::parse_query;

        // Test with explicit directive (needs quotes to prevent tokenization)
        let ast = parse_query("filename:\"main.rs\" OR function", false).unwrap();
        let (filters, simplified) = SearchFilters::extract_and_simplify_with_autodetect(ast);

        assert_eq!(filters.exact_filenames.len(), 1);
        assert_eq!(filters.exact_filenames[0], "main.rs");
        assert!(simplified.is_some()); // "function" remains for content search

        // Test auto-detection with simple filename (no dots/underscores to avoid tokenization)
        let ast2 = parse_query("config.json OR error", false).unwrap();
        let (_filters2, simplified2) = SearchFilters::extract_and_simplify_with_autodetect(ast2);

        // Both terms are processed independently in OR
        assert!(simplified2.is_some());
    }
}
