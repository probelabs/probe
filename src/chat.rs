use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use rig::{
    agent::Agent,
    completion::{Chat as RigChat, CompletionError, Message, Prompt, PromptError, ToolDefinition},
    message::{AssistantContent, Text, UserContent},
    one_or_many::OneOrMany,
    providers::{
        anthropic::{completion::CompletionModel, CLAUDE_3_5_SONNET},
        openai::GPT_4O,
    },
    tool::Tool,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{io::Write, path::PathBuf};
use tiktoken_rs::cl100k_base;

use crate::query::{handle_query, QueryOptions};
use crate::search::{perform_probe, SearchOptions};

#[derive(Debug, thiserror::Error)]
#[error("Search error: {0}")]
pub struct SearchError(String);

#[derive(Debug, thiserror::Error)]
#[error("Query error: {0}")]
pub struct QueryError(String);

#[derive(Deserialize, Serialize)]
pub struct ProbeSearchArgs {
    query: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    files_only: bool,
    #[serde(default)]
    exclude_filenames: bool,
    #[serde(default = "default_reranker")]
    reranker: String,
    #[serde(default = "default_true")]
    frequency_search: bool,
    #[serde(default)]
    exact: bool,
    #[serde(default)]
    allow_tests: bool,
}

#[derive(Deserialize, Serialize)]
pub struct AstGrepQueryArgs {
    pattern: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default)]
    allow_tests: bool,
}

fn default_language() -> String {
    "rust".to_string()
}

#[derive(Serialize)]
pub struct SearchResult {
    result: String,
}

fn default_path() -> String {
    ".".to_string()
}

fn default_reranker() -> String {
    "hybrid".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
pub struct ProbeSearch;

#[derive(Serialize, Deserialize)]
pub struct AstGrepQuery;

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

        println!("\nDoing ast-grep query: \"{}\" in {}", args.pattern, args.path);

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
use std::sync::atomic::{AtomicUsize, Ordering};

// Enum to represent the different model types we support
pub enum ModelType {
    Anthropic(Agent<CompletionModel>),
    OpenAI(Agent<rig::providers::openai::CompletionModel>),
}

pub struct ProbeChat {
    chat: ModelType,
    // Track token usage for the current session using atomic counters for thread safety
    request_tokens: AtomicUsize,
    response_tokens: AtomicUsize,
    // Tokenizer for counting tokens
    tokenizer: tiktoken_rs::CoreBPE,
}

impl ProbeChat {
    pub fn new() -> Result<Self> {
        // Check for environment variables to determine which API to use
        let anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();
        let openai_key = std::env::var("OPENAI_API_KEY").ok();

        // Get model override if provided
        let model_override = std::env::var("MODEL_NAME").ok();

        // Get API URL overrides if provided
        let anthropic_api_url = std::env::var("ANTHROPIC_API_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        let openai_api_url = std::env::var("OPENAI_API_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        // Debug mode check
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

        if debug_mode {
            println!("[DEBUG] API URLs:");
            println!("[DEBUG] - Anthropic: {}", anthropic_api_url);
            println!("[DEBUG] - OpenAI: {}", openai_api_url);
        }

        // Initialize the tokenizer with cl100k_base encoding (works for both Claude and GPT models)
        let tokenizer = cl100k_base().unwrap();

        // Common preamble for both models
        let preamble = r#"You are a helpful assistant that can search code repositories, and answer user questions in detail.

<search_tool>
You have access to a powerful search tool that you can use to answer questions about the codebase.
Use the search tool when you need to find specific text, keywords, or patterns in the code.
Also output the main code structure related to query, with file names and line numbers.

To use the search tool, you MUST format your response exactly like this:
tool: search {"query": "your search query", "path": "."}

For example, if the user asks about chat functionality, your response should be:
tool: search {"query": "chat", "path": "."}

The search tool supports Elasticsearch-like query syntax with the following features:
- Basic term searching: "config" or "search"
- Field-specific searching: "field:value" (e.g., "function:parse")
- Required terms with + prefix: "+required"
- Excluded terms with - prefix: "-excluded"
- Logical operators: "term1 AND term2", "term1 OR term2"
- Grouping with parentheses: "(term1 OR term2) AND term3"

Examples:
- Simple search: "config"
- Required and excluded terms: "+parse -test"
- Field-specific: "function:evaluate"
- Complex query: "(parse OR tokenize) AND query"

When using search tool:
- Try simpler queries (e.g. use 'rpc' instead of 'rpc layer implementation')
- This tool knows how to do the stemming by itself, put only unique keywords to query
- Focus on keywords that would appear in code
- Use multiple search tool calls if needed
- If you can't find what you want after multiple attempts, ask the user for more context
- While doing multiple calls, do not repeat the same queries
</search_tool>

<query_tool>
You also have access to an ast-grep query tool that can search for structural patterns in code.
Use the query tool when you need to find specific code structures like functions, classes, or methods.

To use the query tool, you MUST format your response exactly like this:
tool: query {"pattern": "your pattern here", "language": "language_name", "path": "."}

For example, if the user asks about function definitions in Rust code, your response should be:
tool: query {"pattern": "fn $NAME($$$PARAMS) $$$BODY", "language": "rust", "path": "."}

The query tool supports the following pattern syntax:
- $NAME: Matches a single identifier (variable name, function name, etc.)
- $$$PARAMS: Matches multiple parameters in a parameter list
- $$$BODY: Matches a function or method body
- $$$FIELDS: Matches struct or class fields
- $$$METHODS: Matches class methods

Examples:
- Find Rust functions: {"pattern": "fn $NAME($$$PARAMS) $$$BODY", "language": "rust"}
- Find JavaScript functions: {"pattern": "function $NAME($$$PARAMS) $$$BODY", "language": "javascript"}
- Find JavaScript arrow functions: {"pattern": "const $NAME = ($$$PARAMS) => $$$BODY", "language": "javascript"}
- Find classes: {"pattern": "class $CLASS { $$$METHODS }", "language": "javascript"}
- Find structs: {"pattern": "struct $NAME { $$$FIELDS }", "language": "rust"}

Supported languages: rust, javascript, typescript, python, go, c, cpp, java, ruby, php, swift, csharp

When using query tool:
- Use the appropriate language parameter for the code you're searching
- Use specific patterns that match the code structure you're looking for
- If a pattern doesn't return results, try a simpler pattern
- For complex structures, break them down into smaller patterns
</query_tool>

ALWAYS use the search tool first before attempting to answer questions about the code.
If you need to find specific code structures, use the query tool after using the search tool.

Always base your knowledge only on results from the tools.
When you do not know something, do one more request, and if it failed to answer, acknowledge the issue and ask for more context.

After receiving search results, you can provide a detailed answer based on the code you found.
Where relevant, include diagrams in nice ASCII format."#;

        // Create the appropriate chat based on available API keys
        let model_type = if let Some(key) = anthropic_key {
            if debug_mode {
                println!("[DEBUG] Using Anthropic API");
            }

            let client = rig::providers::anthropic::ClientBuilder::new(&key)
                .anthropic_version(rig::providers::anthropic::ANTHROPIC_VERSION_LATEST)
                .anthropic_beta("prompt-caching-2024-07-31")
                .base_url(&anthropic_api_url)
                .build();

            if debug_mode {
                println!("[DEBUG] Using Anthropic API URL: {}", anthropic_api_url);
            }

            // Use model override if provided, otherwise use default
            let model = if let Some(model_name) = &model_override {
                if debug_mode {
                    println!("[DEBUG] Using custom Anthropic model: {}", model_name);
                }
                model_name.as_str()
            } else {
                CLAUDE_3_5_SONNET
            };

            let anthropic_chat = client
                .agent(model)
                .preamble(preamble)
                .max_tokens(8096)
                .temperature(0.1)
                .tool(ProbeSearch)
                .tool(AstGrepQuery)
                .build();

            ModelType::Anthropic(anthropic_chat)
        } else if let Some(key) = openai_key {
            if debug_mode {
                println!("[DEBUG] Using OpenAI API");
            }

            // For OpenAI, use from_url method instead of ClientBuilder
            let client =
                rig::providers::openai::Client::from_url(&key, &openai_api_url.to_string());

            if debug_mode {
                println!("[DEBUG] Using OpenAI API URL: {}", openai_api_url);
            }

            // Use model override if provided, otherwise use default
            let model = if let Some(model_name) = &model_override {
                if debug_mode {
                    println!("[DEBUG] Using custom OpenAI model: {}", model_name);
                }
                model_name.as_str()
            } else {
                GPT_4O
            };

            // For OpenAI, configure the chat
            let openai_chat = client
                .agent(model)
                .preamble(preamble)
                .max_tokens(4096) // OpenAI typically has lower token limits
                .temperature(0.1)
                .tool(ProbeSearch)
                .tool(AstGrepQuery)
                .build();

            if debug_mode {
                println!("[DEBUG] OpenAI chat configured with default settings");
            }

            ModelType::OpenAI(openai_chat)
        } else {
            // No API keys found, print helpful message and exit
            return Err(anyhow!("No API keys found. Please set either ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable."));
        };

        Ok(Self {
            chat: model_type,
            request_tokens: AtomicUsize::new(0),
            response_tokens: AtomicUsize::new(0),
            tokenizer,
        })
    }
}

// Add a method to ModelType to handle prompting for both model types
impl ModelType {
    async fn prompt(&self, prompt_text: String) -> Result<String, PromptError> {
        match self {
            ModelType::Anthropic(agent) => agent.prompt(prompt_text).await,
            ModelType::OpenAI(agent) => {
                // For OpenAI, we need to handle the prompt differently
                // This is to ensure that tool outputs are properly processed
                let result = agent.prompt(prompt_text).await?;

                // Debug output to help diagnose issues
                if std::env::var("DEBUG").unwrap_or_default() != "" {
                    println!("[DEBUG] OpenAI raw response: {}", result);
                }

                Ok(result)
            }
        }
    }
}

impl ProbeChat {
    // Count tokens in a string using tiktoken
    fn count_tokens(&self, text: &str) -> usize {
        self.tokenizer.encode_ordinary(text).len()
    }

    // Add to request token count (thread-safe)
    fn add_request_tokens(&self, text: &str) {
        let token_count = self.count_tokens(text);
        self.request_tokens.fetch_add(token_count, Ordering::SeqCst);

        if std::env::var("DEBUG").unwrap_or_default() != "" {
            println!(
                "[DEBUG] Added {} request tokens for text of length {}",
                token_count,
                text.len()
            );
        }
    }

    // Add to response token count (thread-safe)
    fn add_response_tokens(&self, text: &str) {
        let token_count = self.count_tokens(text);
        self.response_tokens
            .fetch_add(token_count, Ordering::SeqCst);

        if std::env::var("DEBUG").unwrap_or_default() != "" {
            println!(
                "[DEBUG] Added {} response tokens for text of length {}",
                token_count,
                text.len()
            );
        }
    }

    // Get the current token usage (thread-safe)
    pub fn get_token_usage(&self) -> (usize, usize) {
        (
            self.request_tokens.load(Ordering::SeqCst),
            self.response_tokens.load(Ordering::SeqCst),
        )
    }
}

// Maximum number of previous messages to include in context
const MAX_HISTORY_MESSAGES: usize = 4;
// Maximum length of tool result to include in prompt (in characters)
const MAX_TOOL_RESULT_LENGTH: usize = 999999999;

// Define static regex patterns - more permissive to handle formatting variations
static TOOL_OUTPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<tool_output>\s*.*?\s*</tool_output>").unwrap());

static MULTILINE_TOOL_OUTPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<tool_output>\s*.*?\s*</tool_output>").unwrap());

static SEARCH_RESULTS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)Here are the search results for query '.*?':\n.*?\n\nProvide a detailed")
        .unwrap()
});

impl ProbeChat {
    // Helper function to simplify a search query by removing special operators
    fn simplify_query(query: &str) -> String {
        // Extract the actual query from the JSON string if present
        let query_text = if query.contains("\"query\":") {
            if let Some(start) = query.find("\"query\":") {
                let start_idx = start + 9; // Length of "\"query\":"
                if let Some(quote_start) = query[start_idx..].find('"') {
                    let content_start = start_idx + quote_start + 1;
                    if let Some(quote_end) = query[content_start..].find('"') {
                        query[content_start..(content_start + quote_end)].trim()
                    } else {
                        query
                    }
                } else {
                    query
                }
            } else {
                query
            }
        } else {
            query
        };

        // Remove special operators and simplify the query
        query_text
            .split_whitespace()
            .filter(|&word| {
                !word.starts_with('+')
                    && !word.starts_with('-')
                    && !word.starts_with('(')
                    && !word.ends_with(')')
                    && !word.contains("AND")
                    && !word.contains("OR")
            })
            .take(2) // Only take the first two terms for a broader search
            .collect::<Vec<_>>()
            .join(" ")
    }

    // Helper function to generate a summary of chat history
    fn generate_history_summary(chat_history: &[Message]) -> String {
        chat_history
            .iter()
            .map(|msg| match msg {
                Message::User { content } => {
                    let text = content
                        .iter()
                        .filter_map(|c| {
                            if let UserContent::Text(t) = c {
                                Some(t.text.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<&str>>()
                        .join(" ");
                    format!("User: {}", text)
                }
                Message::Assistant { content } => {
                    // Filter out tool output from assistant messages to save tokens
                    let text = content
                        .iter()
                        .filter_map(|c| {
                            if let AssistantContent::Text(t) = c {
                                // Remove tool output sections with fallback for unmatched cases
                                let filtered = if TOOL_OUTPUT_REGEX.is_match(&t.text) {
                                    TOOL_OUTPUT_REGEX
                                        .replace_all(&t.text, "[Tool results omitted]")
                                        .to_string()
                                } else {
                                    // If no match found, just use the original text
                                    t.text.clone()
                                };
                                Some(filtered)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("Assistant: {}", text)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl RigChat for ProbeChat {
    fn chat(
        &self,
        prompt: impl Into<Message> + Send,
        mut chat_history: Vec<Message>,
    ) -> impl std::future::Future<Output = Result<String, PromptError>> + Send {
        // Append the latest user message to chat history
        let new_message = prompt.into();
        chat_history.push(new_message);

        let self_ref = self; // Use immutable reference since our methods are now thread-safe
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

        async move {
            // Extract the latest user message text
            let latest_user_text = if let Some(Message::User { content }) = chat_history.last() {
                // Iterate through the OneOrMany content items
                let mut text = String::new();
                for item in content.iter() {
                    if let UserContent::Text(t) = item {
                        text = t.text.clone();
                        break;
                    }
                }

                if text.is_empty() {
                    return Err(PromptError::CompletionError(CompletionError::RequestError(
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "No text content found",
                        )),
                    )));
                }

                text
            } else {
                return Err(PromptError::CompletionError(CompletionError::RequestError(
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "No user message found",
                    )),
                )));
            };

            // Clone the user text for later use in continuation prompts
            let latest_user_text_for_continuation = latest_user_text.clone();

            // Build context from limited previous conversation
            // Only include the last MAX_HISTORY_MESSAGES messages to prevent context explosion
            let mut context = String::new();
            let history_start = if chat_history.len() > MAX_HISTORY_MESSAGES + 1 {
                chat_history.len() - MAX_HISTORY_MESSAGES - 1
            } else {
                0
            };

            for (i, msg) in chat_history.iter().enumerate().skip(history_start) {
                // Skip the last message as we'll send it separately
                if i == chat_history.len() - 1 {
                    break;
                }

                match msg {
                    Message::User { content } => {
                        for user_item in content.iter() {
                            if let UserContent::Text(t) = user_item {
                                context.push_str(&format!("User: {}\n", t.text));
                            }
                        }
                    }
                    Message::Assistant { content } => {
                        for ai_item in content.iter() {
                            if let AssistantContent::Text(t) = ai_item {
                                // Filter out tool output from history to save tokens with fallback
                                let filtered_text = if TOOL_OUTPUT_REGEX.is_match(&t.text) {
                                    TOOL_OUTPUT_REGEX
                                        .replace_all(
                                            &t.text,
                                            "[Tool results omitted to save context]\n\n",
                                        )
                                        .to_string()
                                } else {
                                    // If no match found, just use the original text
                                    t.text.clone()
                                };
                                context.push_str(&format!("Assistant: {}\n", filtered_text));
                            }
                        }
                    }
                }
            }

            // Combine context with the latest user message
            let prompt_text = if context.is_empty() {
                latest_user_text
            } else {
                format!("{}\nUser: {}", context, latest_user_text)
            };

            // Count tokens in the prompt
            self_ref.add_request_tokens(&prompt_text);

            // Send the prompt to the LLM based on the model type
            let mut current_result = self_ref.chat.prompt(prompt_text.clone()).await?;

            // Count tokens in the response
            self_ref.add_response_tokens(&current_result);

            // Get current token usage
            let (request_tokens, response_tokens) = self_ref.get_token_usage();

            // Debug: Print the raw response and token usage if DEBUG env var is set
            if debug_mode {
                println!("\n[DEBUG] Raw model response:");
                println!("{}", current_result);
                println!(
                    "\n[DEBUG] Token usage - Request: {}, Response: {}, Total: {}",
                    request_tokens,
                    response_tokens,
                    request_tokens + response_tokens
                );
            }

            // We'll store the final text we want to display here
            let mut response_accumulator = String::new();

            // Process the initial response
            if !current_result.contains("tool:") {
                response_accumulator.push_str(&current_result);
            }

            // Look for "tool:" calls inside the LLM's text
            let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

            if debug_mode {
                println!("\n[DEBUG] Checking for tool calls in response");
                println!(
                    "[DEBUG] Response contains 'tool:': {}",
                    current_result.contains("tool:")
                );
            }

            // Add a maximum limit on tool calls to prevent infinite loops
            const MAX_TOOL_CALLS: usize = 5;
            let mut tool_call_count = 0;

            while current_result.contains("tool:") && tool_call_count < MAX_TOOL_CALLS {
                tool_call_count += 1;
                if debug_mode {
                    println!("[DEBUG] Found 'tool:' in response");
                }

                if let Some(tool_start) = current_result.find("tool:") {
                    if debug_mode {
                        println!("[DEBUG] Tool call starts at position: {}", tool_start);
                    }
                    // Add any text before the tool call to the response
                    if tool_start > 0 {
                        response_accumulator.push_str(&current_result[..tool_start]);
                    }

                    let rest = &current_result[tool_start + 5..];
                    if let Some(end) = rest.find('\n') {
                        let tool_call = &rest[..end].trim();
                        if debug_mode {
                            println!("\n[DEBUG] Processing tool call: {}", tool_call);
                        }

                        // Manual parsing approach instead of regex
                        let parts: Vec<&str> = tool_call.split_whitespace().collect();
                        if parts.len() >= 2 && (parts[0] == "search" || parts[0] == "query") {
                            let tool_name = parts[0];

                            // Find the start of the JSON object (the first '{')
                            if let Some(json_start) = tool_call.find('{') {
                                // Extract everything from the first '{' to the end of the string
                                let raw_json = &tool_call[json_start..];

                                if debug_mode {
                                    println!("[DEBUG] Extracted tool name: {}", tool_name);
                                    println!("[DEBUG] Raw JSON: '{}'", raw_json);
                                }

                                // Validate JSON parameters
                                let tool_params =
                                    match serde_json::from_str::<serde_json::Value>(raw_json) {
                                        Ok(json_value) => {
                                            // JSON is valid, use the raw string
                                            if debug_mode {
                                                println!(
                                                    "[DEBUG] Valid JSON parameters: {}",
                                                    json_value
                                                );
                                            }
                                            raw_json.to_string()
                                        }
                                        Err(e) => {
                                            if debug_mode {
                                                println!("[DEBUG] Invalid JSON parameters: {}", e);
                                                println!("[DEBUG] Raw JSON: '{}'", raw_json);
                                            }

                                            // Add error to response and skip this tool call
                                            response_accumulator.push_str(&format!(
                                            "tool: {} {}\n\nTool parse error: invalid syntax.\n\n",
                                            tool_name, raw_json
                                        ));

                                            // Skip to the next part of the response
                                            if let Some(end_idx) = rest.find('\n') {
                                                current_result = rest[end_idx + 1..].to_string();
                                            } else {
                                                current_result = String::new();
                                            }
                                            continue;
                                        }
                                    };

                                let tool_params_clone = tool_params.clone();

                                // Call the tool based on the model type
                                let tool_result = match &self_ref.chat {
                                    ModelType::Anthropic(agent) => {
                                        agent.tools.call(tool_name, tool_params.clone()).await
                                    }
                                    ModelType::OpenAI(agent) => {
                                        agent.tools.call(tool_name, tool_params.clone()).await
                                    }
                                };

                                match tool_result {
                                    Ok(tool_result) => {
                                        if debug_mode {
                                            println!("\n[DEBUG] Tool result: {}", tool_result);
                                        }

                                        // Check if the search returned no results and retry with a simplified query
                                        if tool_name == "search"
                                            && tool_result.contains("No results found")
                                        {
                                            if debug_mode {
                                                println!("[DEBUG] No results found. Attempting a broader search...");
                                            }

                                            // Add a message about retrying
                                            response_accumulator.push_str(&format!(
                                                "<tool_output>\n{}\n</tool_output>\n\n",
                                                tool_result
                                            ));
                                            response_accumulator.push_str("\nNo results found. Attempting a broader search...\n\n");

                                            // Simplify the query and retry
                                            let simplified_query =
                                                ProbeChat::simplify_query(&tool_params_clone);

                                            if debug_mode {
                                                println!(
                                                    "[DEBUG] Simplified query: '{}'",
                                                    simplified_query
                                                );
                                            }

                                            // Only retry if the simplified query is different and not empty
                                            if !simplified_query.is_empty()
                                                && simplified_query != tool_params_clone
                                            {
                                                let retry_prompt = format!("tool: search {{\"query\": \"{}\", \"path\": \".\"}}", simplified_query);
                                                current_result = retry_prompt;
                                                continue;
                                            }
                                        }

                                        // Add the tool call and result to the response
                                        if debug_mode {
                                            response_accumulator.push_str(&format!(
                                                "tool: {} {}\n\nTool result:\n{}\n\n",
                                                tool_name, tool_params_clone, tool_result
                                            ));
                                        } else {
                                            // In non-debug mode, wrap the tool result in XML tags
                                            response_accumulator.push_str(&format!(
                                                "<tool_output>\n{}\n</tool_output>\n\n",
                                                tool_result
                                            ));
                                        }

                                        // Truncate tool result if it's too long to save tokens
                                        let truncated_result = if tool_result.len()
                                            > MAX_TOOL_RESULT_LENGTH
                                        {
                                            let truncated = &tool_result[..MAX_TOOL_RESULT_LENGTH];
                                            format!(
                                                "{}\n[Result truncated to save context]",
                                                truncated
                                            )
                                        } else {
                                            tool_result.clone()
                                        };

                                        // Generate a summary of the chat history to provide context
                                        let history_summary =
                                            ProbeChat::generate_history_summary(&chat_history);

                                        // Continue the conversation with chat history, latest user message, and tool result
                                        let continuation_prompt = match &self_ref.chat {
                                            ModelType::Anthropic(_) => format!(
                                                "Previous conversation:\n{}\n\nUser: {}\n\ntool: {} {}\n\nTool result:\n<tool_output>\n{}\n</tool_output>\n\nPlease continue based on this information.",
                                                history_summary, latest_user_text_for_continuation, tool_name, tool_params_clone, truncated_result
                                            ),
                                            ModelType::OpenAI(_) => {
                                                // For OpenAI, use a direct format similar to the example
                                                if debug_mode {
                                                    println!("[DEBUG] Creating OpenAI continuation prompt with direct format");
                                                    println!("[DEBUG] Tool result length: {} characters", truncated_result.len());
                                                }

                                                // Format similar to the example, but include chat history
                                                format!(
                                                    "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nHere are the search results for query '{}':\n{}\n\nProvide a detailed answer based on these search results.",
                                                    history_summary,
                                                    latest_user_text_for_continuation,
                                                    tool_params_clone,
                                                    truncated_result
                                                )
                                            },
                                        };

                                        // Count tokens in the continuation prompt
                                        self_ref.add_request_tokens(&continuation_prompt);
                                        // Send the continuation prompt based on the model type
                                        let continuation_result = match &self_ref.chat {
                                            ModelType::Anthropic(_) => {
                                                self_ref.chat.prompt(continuation_prompt).await
                                            }
                                            ModelType::OpenAI(agent) => {
                                                // For OpenAI, we need to ensure the tool result is properly processed
                                                if debug_mode {
                                                    println!("[DEBUG] Directly calling OpenAI agent with tool result");
                                                }

                                                // Create a new prompt that includes chat history, original question, and search results
                                                let direct_prompt = format!(
                                                    "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI searched the codebase and found:\n{}\n\nBased on these search results, provide a detailed answer about the user's question.",
                                                    history_summary,
                                                    latest_user_text_for_continuation,
                                                    truncated_result
                                                );

                                                if debug_mode {
                                                    println!(
                                                        "[DEBUG] Direct OpenAI prompt:\n{}",
                                                        direct_prompt
                                                    );
                                                }

                                                // Call the OpenAI agent directly
                                                agent.prompt(direct_prompt).await
                                            }
                                        };

                                        match continuation_result {
                                            Ok(next_result) => {
                                                current_result = next_result;

                                                // Count tokens in the response
                                                self_ref.add_response_tokens(&current_result);

                                                // Get current token usage
                                                let (request_tokens, response_tokens) =
                                                    self_ref.get_token_usage();

                                                // Debug the continuation response
                                                if debug_mode {
                                                    println!("\n[DEBUG] Continuation response:");
                                                    println!("{}", current_result);
                                                    println!("\n[DEBUG] Updated token usage - Request: {}, Response: {}, Total: {}",
                                                        request_tokens, response_tokens, request_tokens + response_tokens);
                                                }

                                                // If there are no more tool calls, add the result to the response
                                                if !current_result.contains("tool:") {
                                                    response_accumulator.push_str(&current_result);
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                if debug_mode {
                                                    println!(
                                                        "\n[DEBUG] Error getting continuation: {}",
                                                        e
                                                    );
                                                }
                                                response_accumulator
                                                    .push_str(&format!("\nError: {}\n", e));
                                                break;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        if debug_mode {
                                            println!("\n[DEBUG] Tool error: {}", e);
                                        }
                                        if debug_mode {
                                            response_accumulator.push_str(&format!(
                                                "tool: {} {}\n\nTool error: {}\n\n",
                                                tool_name, tool_params_clone, e
                                            ));
                                        } else {
                                            // In non-debug mode, wrap the error in XML tags
                                            response_accumulator.push_str(&format!(
                                                "<tool_output>\nError: {}\n</tool_output>\n\n",
                                                e
                                            ));
                                        }

                                        // Generate a summary of the chat history for error handling
                                        let history_summary =
                                            ProbeChat::generate_history_summary(&chat_history);

                                        // Continue with the error information including chat history
                                        let error_prompt = match &self_ref.chat {
                                            ModelType::Anthropic(_) => format!(
                                                "Previous conversation:\n{}\n\nUser: {}\n\ntool: {} {}\n\nTool error: <tool_output>\n{}\n</tool_output>\n\nPlease continue based on this information.",
                                                history_summary, latest_user_text_for_continuation, tool_name, tool_params_clone, e
                                            ),
                                            ModelType::OpenAI(_) => {
                                                // For OpenAI, use a direct format similar to the example
                                                if debug_mode {
                                                    println!("[DEBUG] Creating OpenAI error prompt with direct format");
                                                }

                                                // Format similar to the example, but include chat history
                                                format!(
                                                    "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nI tried to search with query '{}' but encountered this error: {}\n\nPlease apologize to the user and suggest alternative search terms.",
                                                    history_summary,
                                                    latest_user_text_for_continuation,
                                                    tool_params_clone,
                                                    e
                                                )
                                            },
                                        };

                                        // Count tokens in the error prompt
                                        self_ref.add_request_tokens(&error_prompt);
                                        // Send the error prompt based on the model type
                                        let error_result = match &self_ref.chat {
                                            ModelType::Anthropic(_) => {
                                                self_ref.chat.prompt(error_prompt).await
                                            }
                                            ModelType::OpenAI(agent) => {
                                                // For OpenAI, we need to ensure the error is properly processed
                                                if debug_mode {
                                                    println!("[DEBUG] Directly calling OpenAI agent with error");
                                                }

                                                // Create a new prompt that includes chat history, original question, and the error
                                                let direct_error_prompt = format!(
                                                    "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI tried to search the codebase with the query but encountered an error: {}\n\nPlease apologize to the user and suggest alternative search terms.",
                                                    history_summary,
                                                    latest_user_text_for_continuation,
                                                    e
                                                );

                                                if debug_mode {
                                                    println!(
                                                        "[DEBUG] Direct OpenAI error prompt:\n{}",
                                                        direct_error_prompt
                                                    );
                                                }

                                                // Call the OpenAI agent directly
                                                agent.prompt(direct_error_prompt).await
                                            }
                                        };

                                        match error_result {
                                            Ok(next_result) => {
                                                current_result = next_result;

                                                // Count tokens in the response
                                                self_ref.add_response_tokens(&current_result);

                                                // Get current token usage
                                                let (request_tokens, response_tokens) =
                                                    self_ref.get_token_usage();

                                                if debug_mode {
                                                    println!(
                                                        "\n[DEBUG] Error continuation response:"
                                                    );
                                                    println!("{}", current_result);
                                                    println!("\n[DEBUG] Updated token usage - Request: {}, Response: {}, Total: {}",
                                                        request_tokens, response_tokens, request_tokens + response_tokens);
                                                }

                                                // If there are no more tool calls, add the result to the response
                                                if !current_result.contains("tool:") {
                                                    response_accumulator.push_str(&current_result);
                                                    break;
                                                }
                                            }
                                            Err(e) => {
                                                if debug_mode {
                                                    println!(
                                                        "\n[DEBUG] Error getting continuation: {}",
                                                        e
                                                    );
                                                }
                                                response_accumulator
                                                    .push_str(&format!("\nError: {}\n", e));
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // No JSON object found
                                if debug_mode {
                                    println!(
                                        "[DEBUG] No JSON object found in tool call: {}",
                                        tool_call
                                    );
                                }
                                response_accumulator.push_str(&format!(
                                    "tool: {}\n\nTool parse error: invalid syntax.\n\n",
                                    tool_call
                                ));

                                // Skip to the next part of the response
                                if let Some(end_idx) = rest.find('\n') {
                                    current_result = rest[end_idx + 1..].to_string();
                                } else {
                                    current_result = String::new();
                                }
                                continue;
                            }
                        } else {
                            // Invalid tool call format
                            if debug_mode {
                                println!("[DEBUG] Invalid tool call format: {}", tool_call);
                                response_accumulator.push_str(&format!(
                                    "tool: {}\n\nTool parse error: invalid syntax.\n\n",
                                    tool_call
                                ));
                            } else {
                                // In non-debug mode, just add a generic error message
                                response_accumulator
                                    .push_str("Error: Invalid tool call syntax.\n\n");
                            }

                            // Skip this tool call and continue with the rest of the response
                            if let Some(end_idx) = rest.find('\n') {
                                current_result = rest[end_idx + 1..].to_string();
                            } else {
                                current_result = String::new();
                            }

                            // If there are no more tool calls, add the result to the response
                            if !current_result.contains("tool:") {
                                response_accumulator.push_str(&current_result);
                                break;
                            }
                        }
                    } else {
                        // No newline found after tool:, just add the rest and break
                        response_accumulator.push_str(&current_result);
                        break;
                    }
                }
            }

            // Add a message if the maximum tool calls limit was reached
            if tool_call_count >= MAX_TOOL_CALLS {
                response_accumulator
                    .push_str("\nMaximum tool calls reached. Please refine your query.\n");
            }

            if debug_mode {
                println!("[DEBUG] Got AI response");
            }
            Ok(response_accumulator)
        }
    }
}

use colored::*;

pub async fn handle_chat() -> Result<()> {
    let chat = ProbeChat::new()?;
    let mut input = String::new();
    let mut conversation_history = Vec::new();
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

    // Get model information for display (prefixed with underscore to indicate intentional non-use)
    let (api_name, model_name) = match &chat.chat {
        ModelType::Anthropic(_) => {
            let model =
                std::env::var("MODEL_NAME").unwrap_or_else(|_| CLAUDE_3_5_SONNET.to_string());
            ("Anthropic".bright_purple(), model)
        }
        ModelType::OpenAI(_) => {
            let model = std::env::var("MODEL_NAME").unwrap_or_else(|_| GPT_4O.to_string());
            ("OpenAI".bright_green(), model)
        }
    };

    println!("{}", "Welcome to Probe AI Assistant".bold().bright_blue());
    println!(
        "{}",
        format!("Using {} API with model: {}", api_name, model_name.dimmed()).dimmed()
    );
    println!(
        "{}",
        "Ask questions about your codebase or search for code patterns".dimmed()
    );
    println!(
        "{}",
        "Type 'quit' to exit, 'help' for command list".dimmed()
    );
    println!(
        "{}",
        "───────────────────────────────────────────────────────────────────".cyan()
    );

    loop {
        input.clear();
        print!("{} ", "❯".bright_green().bold());
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut input)?;

        let trimmed = input.trim();
        if trimmed.eq_ignore_ascii_case("quit") {
            println!("{}", "Goodbye! 👋".bright_blue());
            break;
        }

        if trimmed.eq_ignore_ascii_case("help") {
            display_help();
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Show a thinking indicator
        println!("{}", "Thinking...".italic().dimmed());

        // Construct a user message
        let user_msg = Message::User {
            content: OneOrMany::one(UserContent::Text(Text {
                text: trimmed.to_string(),
            })),
        };

        // Chat with the updated history (don't add the user message to history yet)
        match chat
            .chat(user_msg.clone(), conversation_history.clone())
            .await
        {
            Ok(response) => {
                // Clear the "Thinking..." line
                print!("\r{}\r", " ".repeat(11));

                // Filter out tool output before printing to console
                // Use a more robust regex that handles multiline tool outputs
                let filtered_response = match &chat.chat {
                    ModelType::Anthropic(_) => {
                        // For Anthropic, look for the <tool_output> tags with fallback
                        if MULTILINE_TOOL_OUTPUT_REGEX.is_match(&response) {
                            MULTILINE_TOOL_OUTPUT_REGEX
                                .replace_all(
                                    &response,
                                    format!(
                                        "{}\n\n",
                                        "Tool results processed (hidden from output)"
                                            .italic()
                                            .dimmed()
                                    )
                                    .as_str(),
                                )
                                .to_string()
                        } else {
                            // If no match found, just use the original response
                            response.clone()
                        }
                    }
                    ModelType::OpenAI(_) => {
                        // For OpenAI, look for the search results section with our new format with fallback
                        if SEARCH_RESULTS_REGEX.is_match(&response) {
                            SEARCH_RESULTS_REGEX
                                .replace_all(
                                    &response,
                                    "I have analyzed the search results. Provide a detailed",
                                )
                                .to_string()
                        } else {
                            // If no match found, just use the original response
                            response.clone()
                        }
                    }
                };

                // Get token usage
                let (request_tokens, response_tokens) = chat.get_token_usage();
                let total_tokens = request_tokens + response_tokens;

                // Print the response with a nice separator
                println!(
                    "{} {}",
                    "─ Response ".bright_blue(),
                    "─".repeat(65).bright_blue()
                );

                // For OpenAI, check if the response contains raw JSON tool output and suppress it
                let display_response = match &chat.chat {
                    ModelType::OpenAI(_) => {
                        if response.contains("[{\"result\":\"File:") {
                            // If we detect raw JSON output, don't display it
                            if debug_mode {
                                println!("[DEBUG] Detected raw JSON output, suppressing display");
                            }
                            "Processing search results...".italic().dimmed().to_string()
                        } else {
                            filtered_response.clone()
                        }
                    }
                    _ => filtered_response.clone(),
                };

                println!("{}", display_response);
                println!("{}", "─".repeat(65).bright_blue());

                // Print token usage information with more details
                println!(
                    "{} {} {} {}",
                    "Token Usage:".yellow().bold(),
                    format!("Request: {}", request_tokens).yellow(),
                    format!("Response: {}", response_tokens).yellow(),
                    format!(
                        "(Current message only: ~{})",
                        chat.count_tokens(&filtered_response)
                    )
                    .yellow()
                    .dimmed()
                );
                println!(
                    "{} {} {}",
                    "Total:".yellow().bold(),
                    format!("{} tokens", total_tokens).yellow(),
                    "(Cumulative for entire session)".yellow().dimmed()
                );
                println!("{}", "─".repeat(65).bright_blue());

                // Now add both the user message and assistant response to history
                conversation_history.push(user_msg);
                conversation_history.push(Message::Assistant {
                    content: OneOrMany::one(AssistantContent::Text(Text { text: response })),
                });
            }
            Err(e) => {
                println!("{} {}", "Error:".bold().red(), e);
                break;
            }
        }
    }

    Ok(())
}

fn display_help() {
    println!("{}", "Probe AI Assistant Commands".bold().bright_blue());
    println!("{}", "  help    - Display this help message".dimmed());
    println!("{}", "  quit    - Exit the assistant".dimmed());
    println!();
    println!("{}", "Environment Variables:".bold());
    println!(
        "{}",
        "  ANTHROPIC_API_KEY - Set to use Anthropic's Claude models".dimmed()
    );
    println!(
        "{}",
        "  OPENAI_API_KEY    - Set to use OpenAI's GPT models".dimmed()
    );
    println!(
        "{}",
        "  MODEL_NAME        - Override the default model name".dimmed()
    );
    println!(
        "{}",
        "  ANTHROPIC_API_URL - Override the default Anthropic API URL".dimmed()
    );
    println!(
        "{}",
        "  OPENAI_API_URL    - Override the default OpenAI API URL".dimmed()
    );
    println!(
        "{}",
        "  DEBUG             - Set to any value to enable debug output".dimmed()
    );
    println!();
    println!("{}", "Example Queries:".bold());
    println!(
        "{}",
        "  \"Find all implementations of the search function\"".dimmed()
    );
    println!("{}", "  \"How does the ranking algorithm work?\"".dimmed());
    println!(
        "{}",
        "  \"Show me the main entry point of the application\"".dimmed()
    );
    println!(
        "{}",
        "  \"Find all Rust functions that return a Result\"".dimmed()
    );
    println!(
        "{}",
        "  \"Show me all JavaScript classes in the codebase\"".dimmed()
    );
}
