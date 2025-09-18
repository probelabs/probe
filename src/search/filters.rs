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
    }

    /// Add a filter based on field name and values
    pub fn add_filter(&mut self, field_name: &str, values: Vec<String>) {
        match field_name.to_lowercase().as_str() {
            "file" | "path" => {
                self.file_patterns.extend(values);
            }
            "ext" | "extension" => {
                // Split comma-separated values and normalize extensions
                for value in values {
                    for ext in value.split(',') {
                        let ext = ext.trim();
                        if !ext.is_empty() {
                            // Remove leading dot if present
                            let normalized = if ext.starts_with('.') {
                                ext[1..].to_string()
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

/// Check if a field name is a recognized filter field
fn is_filter_field(field_name: &str) -> bool {
    matches!(
        field_name.to_lowercase().as_str(),
        "file" | "path" | "ext" | "extension" | "type" | "dir" | "directory" | "lang" | "language"
    )
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
        assert!(!is_filter_field("content"));
        assert!(!is_filter_field("random"));
    }
}
