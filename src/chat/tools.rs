use anyhow::Result;
use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

// Import the extract module
use crate::query::{handle_query, QueryOptions};
use crate::search::{perform_probe, SearchOptions};
use probe::extract;

use super::errors::{ExtractError, QueryError, SearchError};

// Default functions for tool arguments
fn default_path() -> String {
    ".".to_string()
}

fn default_reranker() -> String {
    "hybrid".to_string()
}

fn default_true() -> bool {
    true
}

fn default_language() -> String {
    "rust".to_string()
}

fn default_context_lines() -> usize {
    10
}

fn default_format() -> String {
    "plain".to_string()
}

// Tool argument structs
#[derive(Deserialize, Serialize)]
pub struct ProbeSearchArgs {
    pub query: String,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default)]
    pub files_only: bool,
    #[serde(default)]
    pub exclude_filenames: bool,
    #[serde(default = "default_reranker")]
    pub reranker: String,
    #[serde(default = "default_true")]
    pub frequency_search: bool,
    #[serde(default)]
    pub exact: bool,
    #[serde(default)]
    pub allow_tests: bool,
}

#[derive(Deserialize, Serialize)]
pub struct AstGrepQueryArgs {
    pub pattern: String,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub allow_tests: bool,
}

#[derive(Deserialize, Serialize)]
pub struct ExtractArgs {
    pub file_path: String,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub end_line: Option<usize>,
    #[serde(default)]
    pub allow_tests: bool,
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub result: String,
}

// Tool implementations
#[derive(Serialize, Deserialize)]
pub struct ProbeSearch;

#[derive(Serialize, Deserialize)]
pub struct AstGrepQuery;

#[derive(Serialize, Deserialize)]
pub struct Extract;

impl Tool for Extract {
    const NAME: &'static str = "extract";

    type Error = ExtractError;
    type Args = ExtractArgs;
    type Output = Vec<SearchResult>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "extract".to_string(),
            description:
                "Extract code blocks from files based on file paths and optional line numbers"
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to extract from. Can include a line number (e.g., 'src/main.rs:42') or a line range (e.g., 'src/main.rs:1-60')"
                    },
                    "line": {
                        "type": "integer",
                        "description": "Start line number to extract a specific code block. If provided alone, the tool will find the closest suitable parent node (function, struct, class, etc.) for that line",
                        "default": null
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "End line number for extracting a range of lines. Used together with 'line' parameter to specify a range",
                        "default": null
                    },
                    "allow_tests": {
                        "type": "boolean",
                        "description": "Allow test files and test code blocks",
                        "default": false
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Number of context lines to include before and after the specified line (used when no suitable code block is found)",
                        "default": 10
                    },
                    "format": {
                        "type": "string",
                        "description": "Output format (plain, markdown, json, color)",
                        "enum": ["plain", "markdown", "json", "color"],
                        "default": "plain"
                    }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

        // Parse file_path to extract line number or range if present (e.g., "src/main.rs:42" or "src/main.rs:1-60")
        let (file_path, start_line, end_line) = if args.file_path.contains(':') {
            let parts: Vec<&str> = args.file_path.split(':').collect();
            if parts.len() >= 2 {
                let path = parts[0];
                let line_spec = parts[1];
                
                // Check if it's a range (contains a hyphen)
                if line_spec.contains('-') {
                    let range_parts: Vec<&str> = line_spec.split('-').collect();
                    if range_parts.len() == 2 {
                        let start = range_parts[0].parse::<usize>().ok();
                        let end = range_parts[1].parse::<usize>().ok();
                        
                        if let (Some(s), Some(e)) = (start, end) {
                            (path.to_string(), Some(s), Some(e))
                        } else {
                            (args.file_path.clone(), args.line, args.end_line)
                        }
                    } else {
                        (args.file_path.clone(), args.line, args.end_line)
                    }
                } else {
                    // Try to parse as a single line number
                    if let Ok(line) = line_spec.parse::<usize>() {
                        (path.to_string(), Some(line), args.end_line)
                    } else {
                        (args.file_path.clone(), args.line, args.end_line)
                    }
                }
            } else {
                (args.file_path.clone(), args.line, args.end_line)
            }
        } else {
            (args.file_path.clone(), args.line, args.end_line)
        };

        // Prepare message about what we're extracting
        let extraction_info = match (start_line, end_line) {
            (Some(start), Some(end)) => format!(" lines {}-{}", start, end),
            (Some(line), None) => format!(" at line {}", line),
            _ => String::new(),
        };

        println!(
            "\nExtracting code from file: {}{}",
            file_path,
            extraction_info
        );

        if debug_mode {
            println!("\n[DEBUG] ===== Extract Tool Called =====");
            println!("[DEBUG] File path: '{}'", file_path);
            println!("[DEBUG] Start line: {:?}", start_line);
            println!("[DEBUG] End line: {:?}", end_line);
            println!("[DEBUG] Allow tests: {}", args.allow_tests);
            println!("[DEBUG] Context lines: {}", args.context_lines);
            println!("[DEBUG] Format: {}", args.format);
        }

        let path = PathBuf::from(&file_path);

        if debug_mode {
            println!("[DEBUG] File path exists: {}", path.exists());
        }

        // Process the file for extraction
        let probe_result = extract::process_file_for_extraction(
            &path,
            start_line,
            end_line,
            args.allow_tests,
            args.context_lines,
        )
        .map_err(|e| ExtractError(e.to_string()))?;

        if debug_mode {
            println!("\n[DEBUG] ===== Extract Results =====");
            println!("[DEBUG] File: {}", probe_result.file);
            println!("[DEBUG] Lines: {:?}", probe_result.lines);
            println!("[DEBUG] Node type: {}", probe_result.node_type);
            println!("[DEBUG] Code length: {} chars", probe_result.code.len());
        }

        // Format the result for output
        let formatted = format!(
            "File: {}\nLines: {}-{}\nType: {}\n\nCode:\n{}",
            probe_result.file,
            probe_result.lines.0,
            probe_result.lines.1,
            probe_result.node_type,
            probe_result.code
        );

        if debug_mode {
            println!("[DEBUG] ===== End Extract =====\n");
        }

        Ok(vec![SearchResult { result: formatted }])
    }
}

impl Tool for AstGrepQuery {
    const NAME: &'static str = "query";

    type Error = QueryError;
    type Args = AstGrepQueryArgs;
    type Output = Vec<SearchResult>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "query".to_string(),
            description: "Search code using ast-grep structural pattern matching".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "AST pattern to search for. Use $NAME for variable names, $$$PARAMS for parameter lists, $$$BODY for function bodies, etc.",
                        "examples": [
                            "fn $NAME($$$PARAMS) $$$BODY",
                            "function $NAME($$$PARAMS) $$$BODY",
                            "class $CLASS { $$$METHODS }",
                            "struct $NAME { $$$FIELDS }",
                            "const $NAME = ($$$PARAMS) => $$$BODY"
                        ]
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to search in",
                        "default": "."
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language to use for parsing",
                        "enum": ["rust", "javascript", "typescript", "python", "go", "c", "cpp", "java", "ruby", "php", "swift", "csharp"],
                        "default": "rust"
                    },
                    "allow_tests": {
                        "type": "boolean",
                        "description": "Allow test files in search results",
                        "default": false
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

        println!(
            "\nDoing ast-grep query: \"{}\" in {}",
            args.pattern, args.path
        );

        if debug_mode {
            println!("\n[DEBUG] ===== Query Tool Called =====");
            println!("[DEBUG] Pattern: '{}'", args.pattern);
            println!("[DEBUG] Search path: '{}'", args.path);
            println!("[DEBUG] Language: '{}'", args.language);
        }

        let path = PathBuf::from(args.path);

        if debug_mode {
            println!("\n[DEBUG] Query configuration:");
            println!("[DEBUG] - Allow tests: {}", args.allow_tests);
            println!("[DEBUG] Search path exists: {}", path.exists());
        }

        let query_options = QueryOptions {
            path: &path,
            pattern: &args.pattern,
            language: Some(&args.language),
            ignore: &[],
            allow_tests: args.allow_tests,
            max_results: None,
            format: "plain",
        };

        // Use std::panic::catch_unwind to handle potential panics from ast-grep
        let result = std::panic::catch_unwind(|| {
            handle_query(
                &args.pattern,
                &path,
                Some(&args.language),
                &[],
                args.allow_tests,
                None,
                "plain",
            )
        });

        match result {
            Ok(Ok(_)) => {
                // Successfully executed query, now we need to perform the query again to get the results
                // This is because handle_query prints the results directly and doesn't return them
                let matches = crate::query::perform_query(&query_options)
                    .map_err(|e| QueryError(e.to_string()))?;

                if debug_mode {
                    println!("\n[DEBUG] ===== Query Results =====");
                    println!("[DEBUG] Found {} matches", matches.len());
                }

                if matches.is_empty() {
                    if debug_mode {
                        println!("[DEBUG] No results found for pattern: '{}'", args.pattern);
                        println!("[DEBUG] ===== End Query =====\n");
                    }
                    // Return a clear message instead of an empty vector
                    Ok(vec![SearchResult {
                        result: format!("No results found for the pattern: '{}'.", args.pattern),
                    }])
                } else {
                    let results: Vec<SearchResult> = matches
                        .iter()
                        .map(|m| {
                            if debug_mode {
                                println!(
                                    "\n[DEBUG] Processing match from file: {}",
                                    m.file_path.display()
                                );
                            }

                            let formatted = format!(
                                "File: {}:{}:{}\n\nCode:\n{}",
                                m.file_path.display(),
                                m.line_start,
                                m.column_start,
                                m.matched_text
                            );

                            if debug_mode {
                                println!(
                                    "[DEBUG] Formatted result length: {} chars",
                                    formatted.len()
                                );
                            }

                            SearchResult { result: formatted }
                        })
                        .collect();

                    let matches_text = match results.len() {
                        0 => "no matches".to_string(),
                        1 => "1 match".to_string(),
                        n => format!("{} matches", n),
                    };
                    println!("Found {}", matches_text);

                    if debug_mode {
                        println!("[DEBUG] ===== End Query =====\n");
                    }
                    Ok(results)
                }
            }
            Ok(Err(e)) => Err(QueryError(e.to_string())),
            Err(e) => {
                let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&'static str>() {
                    s.to_string()
                } else {
                    "Unknown error occurred during query execution".to_string()
                };
                Err(QueryError(error_msg))
            }
        }
    }
}

impl Tool for ProbeSearch {
    const NAME: &'static str = "search";

    type Error = SearchError;
    type Args = ProbeSearchArgs;
    type Output = Vec<SearchResult>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search".to_string(),
            description: "Search code in the repository using Elasticsearch-like query syntax"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query with Elasticsearch-like syntax support. Supports logical operators (AND, OR), required (+) and excluded (-) terms, and grouping with parentheses.",
                        "examples": [
                            "hybrid",
                            "Config",
                            "RPC",
                            "+required -excluded",
                            "(term1 OR term2) AND term3"
                        ]
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to search in",
                        "default": "."
                    },
                    "exact": {
                        "type": "boolean",
                        "description": "Use exact match when you explicitly want to match specific search query, without stemming. Used when you exactly know function or Struct name",
                        "default": false
                    },
                    "allow_tests": {
                        "type": "boolean",
                        "description": "Allow test files in search results",
                        "default": false
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

        println!("\nDoing code search: \"{}\" in {}", args.query, args.path);

        if debug_mode {
            println!("\n[DEBUG] ===== Search Tool Called =====");
            println!("[DEBUG] Raw query: '{}'", args.query);
            println!("[DEBUG] Search path: '{}'", args.path);
        }

        let query_text = args.query.trim().to_lowercase();
        if debug_mode {
            println!("[DEBUG] Normalized query: '{}'", query_text);
        }

        let query = vec![query_text];
        let path = PathBuf::from(args.path);

        if debug_mode {
            println!("\n[DEBUG] Search configuration:");
            println!("[DEBUG] - Files only: {}", args.files_only);
            println!("[DEBUG] - Exclude filenames: {}", args.exclude_filenames);
            println!("[DEBUG] - Frequency search: {}", args.frequency_search);
            println!("[DEBUG] - Exact match: {}", args.exact);
            println!("[DEBUG] - Allow tests: {}", args.allow_tests);

            println!("[DEBUG] Query vector: {:?}", query);
            println!("[DEBUG] Search path exists: {}", path.exists());
        }

        let search_options = SearchOptions {
            path: &path,
            queries: &query,
            files_only: args.files_only,
            custom_ignores: &[],
            exclude_filenames: args.exclude_filenames,
            reranker: &args.reranker,
            frequency_search: if args.exact {
                false
            } else {
                args.frequency_search
            },
            max_results: None,
            max_bytes: None,
            max_tokens: Some(40000),
            allow_tests: args.allow_tests,
            exact: args.exact,
            no_merge: false,
            merge_threshold: None,
            dry_run: false, // Chat mode doesn't use dry-run
        };

        if debug_mode {
            println!("\n[DEBUG] Executing search with options:");
            println!("[DEBUG] - Path: {:?}", search_options.path);
            println!("[DEBUG] - Queries: {:?}", search_options.queries);
            println!("[DEBUG] - Files only: {}", search_options.files_only);
            println!(
                "[DEBUG] - Exclude filenames: {}",
                search_options.exclude_filenames
            );
            println!("[DEBUG] - Reranker: {}", search_options.reranker);
            println!(
                "[DEBUG] - Frequency search: {}",
                search_options.frequency_search
            );
            println!("[DEBUG] - Exact: {}", search_options.exact);
        }

        let limited_results =
            perform_probe(&search_options).map_err(|e| SearchError(e.to_string()))?;

        if debug_mode {
            println!("\n[DEBUG] ===== Search Results =====");
            println!("[DEBUG] Found {} results", limited_results.results.len());
        }

        if limited_results.results.is_empty() {
            if debug_mode {
                println!("[DEBUG] No results found for query: '{}'", args.query);
                println!("[DEBUG] ===== End Search =====\n");
            }
            // Return a clear message instead of an empty vector
            Ok(vec![SearchResult {
                result: format!("No results found for the query: '{}'.", args.query),
            }])
        } else {
            let results: Vec<SearchResult> = limited_results
                .results
                .iter()
                .map(|result| {
                    if debug_mode {
                        println!("\n[DEBUG] Processing match from file: {}", result.file);

                        if result.code.trim().is_empty() {
                            println!("[DEBUG] WARNING: Empty code block found");
                        }
                    }

                    let formatted = format!("File: {}\n\nCode:\n{}", result.file, result.code);

                    if debug_mode {
                        println!("[DEBUG] Formatted result length: {} chars", formatted.len());
                    }

                    SearchResult { result: formatted }
                })
                .collect();

            let matches_text = match results.len() {
                0 => "no matches".to_string(),
                1 => "1 match".to_string(),
                n => format!("{} matches", n),
            };
            println!("Found {}", matches_text);

            if debug_mode {
                println!("[DEBUG] ===== End Search =====\n");
            }
            Ok(results)
        }
    }
}
