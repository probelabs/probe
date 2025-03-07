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

use crate::search::{perform_probe, SearchOptions};

#[derive(Debug, thiserror::Error)]
#[error("Search error: {0}")]
pub struct SearchError(String);

#[derive(Deserialize, Serialize)]
pub struct ProbeSearchArgs {
    pattern: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    files_only: bool,
    #[serde(default)]
    include_filenames: bool,
    #[serde(default = "default_reranker")]
    reranker: String,
    #[serde(default = "default_true")]
    frequency_search: bool,
    #[serde(default)]
    exact: bool,
    #[serde(default)]
    allow_tests: bool,
    #[serde(default)]
    any_term: bool,
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

impl Tool for ProbeSearch {
    const NAME: &'static str = "search";

    type Error = SearchError;
    type Args = ProbeSearchArgs;
    type Output = Vec<SearchResult>;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "search".to_string(),
            description: "Search code in the repository using patterns".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern. Try to use simpler queries and focus on keywords that can appear in code",
                        "examples": ["hybrid", "Config", "RPC"]
                    },
                    "path": {
                        "type": "string",
                        "description": "Path to search in",
                        "default": "."
                    },
                    "exact": {
                        "type": "boolean",
                        "description": "Use exact match when you explicitly want to match specific search pattern, without stemming. Used when you exaclty know function or Struct name",
                        "default": false
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

        println!("\nDoing code search: \"{}\" in {}", args.pattern, args.path);

        if debug_mode {
            println!("\n[DEBUG] ===== Search Tool Called =====");
            println!("[DEBUG] Raw pattern: '{}'", args.pattern);
            println!("[DEBUG] Search path: '{}'", args.path);
        }

        let pattern = args.pattern.trim().to_lowercase();
        if debug_mode {
            println!("[DEBUG] Normalized pattern: '{}'", pattern);
        }

        let query = vec![pattern];
        let path = PathBuf::from(args.path);

        if debug_mode {
            println!("\n[DEBUG] Search configuration:");
            println!("[DEBUG] - Files only: {}", args.files_only);
            println!("[DEBUG] - Include filenames: {}", args.include_filenames);
            println!("[DEBUG] - Frequency search: {}", args.frequency_search);
            println!("[DEBUG] - Exact match: {}", args.exact);
            println!("[DEBUG] - Allow tests: {}", args.allow_tests);
            println!("[DEBUG] - Any term: {}", args.any_term);

            println!("[DEBUG] Query vector: {:?}", query);
            println!("[DEBUG] Search path exists: {}", path.exists());
        }

        let search_options = SearchOptions {
            path: &path,
            queries: &query,
            files_only: args.files_only,
            custom_ignores: &[],
            include_filenames: args.include_filenames,
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
            any_term: args.any_term,
            exact: args.exact,
            no_merge: false,
            merge_threshold: None,
        };

        if debug_mode {
            println!("\n[DEBUG] Executing search with options:");
            println!("[DEBUG] - Path: {:?}", search_options.path);
            println!("[DEBUG] - Queries: {:?}", search_options.queries);
            println!("[DEBUG] - Files only: {}", search_options.files_only);
            println!(
                "[DEBUG] - Include filenames: {}",
                search_options.include_filenames
            );
            println!("[DEBUG] - Reranker: {}", search_options.reranker);
            println!(
                "[DEBUG] - Frequency search: {}",
                search_options.frequency_search
            );
            println!("[DEBUG] - Any term: {}", search_options.any_term);
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
                println!("[DEBUG] No results found for pattern: '{}'", args.pattern);
                println!("[DEBUG] ===== End Search =====\n");
            }
            Ok(vec![])
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

You have access to a powerful search tool that you MUST use to answer questions about the codebase.
ALWAYS use the search tool first before attempting to answer questions about the code.
Also output the main code structure related to query, with file names and line numbers.

To use the search tool, you MUST format your response exactly like this:
tool: search {"pattern": "your search pattern", "path": "."}

For example, if the user asks about chat functionality, your response should be:
tool: search {"pattern": "chat", "path": "."}

Always based your knowledge only based on results from the search tool.
When you do not know smth, do one more request, and if it failed to answer, acknowledge issue and ask for more context.

When using search tool:
- Try simpler queries (e.g. use 'rpc' instead of 'rpc layer implementation')
- Focus on keywords that would appear in code
- Split distinct terms into separate searches, unless they should be search together, e.g. how they connect.
- Use multiple search tool calls if needed
- If you can't find what you want after multiple attempts, ask the user for more context
- While doing multiple calls, do not repeat the same queries

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

// Define static regex patterns
static TOOL_OUTPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<tool_output>\n.*?\n</tool_output>\n\n").unwrap());

static MULTILINE_TOOL_OUTPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)<tool_output>\n.*?\n</tool_output>\n\n").unwrap());

static SEARCH_RESULTS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)Here are the search results for '.*?':\n.*?\n\nProvide a detailed").unwrap()
});

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
                                // Filter out tool output from history to save tokens
                                let filtered_text = TOOL_OUTPUT_REGEX
                                    .replace_all(
                                        &t.text,
                                        "[Tool results omitted to save context]\n\n",
                                    )
                                    .to_string();
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

            while current_result.contains("tool:") {
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

                        // Attempt to split into name and params
                        let parts: Vec<&str> = tool_call.splitn(2, char::is_whitespace).collect();
                        if parts.len() == 2 {
                            // Call the tool
                            let tool_name = parts[0];
                            let tool_params = parts[1].to_string();
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
                                        format!("{}\n[Result truncated to save context]", truncated)
                                    } else {
                                        tool_result.clone()
                                    };

                                    // Continue the conversation with just the latest user message and tool result
                                    // This prevents context explosion by not including the entire history again
                                    let continuation_prompt = match &self_ref.chat {
                                        ModelType::Anthropic(_) => format!(
                                            "User: {}\n\ntool: {} {}\n\nTool result:\n<tool_output>\n{}\n</tool_output>\n\nPlease continue based on this information.",
                                            latest_user_text_for_continuation, tool_name, tool_params_clone, truncated_result
                                        ),
                                        ModelType::OpenAI(_) => {
                                            // For OpenAI, use a direct format similar to the example
                                            if debug_mode {
                                                println!("[DEBUG] Creating OpenAI continuation prompt with direct format");
                                                println!("[DEBUG] Tool result length: {} characters", truncated_result.len());
                                            }

                                            // Format similar to the example, very direct
                                            format!(
                                                "You are a code search assistant. The user asked: {}\n\nHere are the search results for '{}':\n{}\n\nProvide a detailed answer based on these search results.",
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

                                            // Create a new prompt that includes the original question and the search results
                                            let direct_prompt = format!(
                                                "The user asked: '{}'\n\nI searched the codebase and found:\n{}\n\nBased on these search results, provide a detailed answer about the user's question.",
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

                                    // Continue with the error information - simplified prompt
                                    let error_prompt = match &self_ref.chat {
                                        ModelType::Anthropic(_) => format!(
                                            "User: {}\n\ntool: {} {}\n\nTool error: <tool_output>\n{}\n</tool_output>\n\nPlease continue based on this information.",
                                            latest_user_text_for_continuation, tool_name, tool_params_clone, e
                                        ),
                                        ModelType::OpenAI(_) => {
                                            // For OpenAI, use a direct format similar to the example
                                            if debug_mode {
                                                println!("[DEBUG] Creating OpenAI error prompt with direct format");
                                            }

                                            // Format similar to the example, very direct
                                            format!(
                                                "You are a code search assistant. The user asked: {}\n\nI tried to search for '{}' but encountered this error: {}\n\nPlease apologize to the user and suggest alternative search terms.",
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

                                            // Create a new prompt that includes the original question and the error
                                            let direct_error_prompt = format!(
                                                "The user asked: '{}'\n\nI tried to search the codebase but encountered an error: {}\n\nPlease apologize to the user and suggest alternative search terms.",
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
                                                println!("\n[DEBUG] Error continuation response:");
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
                            // Invalid tool call format
                            if debug_mode {
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
                            let remaining_text = rest[end..].trim().to_string();
                            current_result = remaining_text;

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

    // Get model information for display
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

    // Display a nice welcome message
    println!(
        "{}",
        "╭───────────────────────────────────────────────────────────────────╮".cyan()
    );
    println!(
        "{} {:<65} {}",
        "│".cyan(),
        "Welcome to Probe AI Assistant".bold().bright_blue(),
        "│".cyan()
    );
    println!(
        "{} {:<65} {}",
        "│".cyan(),
        format!("Using {} API with model: {}", api_name, model_name.dimmed()).dimmed(),
        "│".cyan()
    );
    println!(
        "{} {:<65} {}",
        "│".cyan(),
        "Ask questions about your codebase or search for code patterns".dimmed(),
        "│".cyan()
    );
    println!(
        "{} {:<65} {}",
        "│".cyan(),
        "Type 'quit' to exit, 'help' for command list".dimmed(),
        "│".cyan()
    );
    println!(
        "{}",
        "╰───────────────────────────────────────────────────────────────────╯".cyan()
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
                        // For Anthropic, look for the <tool_output> tags
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
                    }
                    ModelType::OpenAI(_) => {
                        // For OpenAI, look for the search results section with our new format
                        SEARCH_RESULTS_REGEX
                            .replace_all(
                                &response,
                                "I have analyzed the search results. Provide a detailed",
                            )
                            .to_string()
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
    println!("{}", "Example queries:".bold());
    println!(
        "{}",
        "  \"Find all implementations of the search function\"".dimmed()
    );
    println!("{}", "  \"How does the ranking algorithm work?\"".dimmed());
    println!(
        "{}",
        "  \"Show me the main entry point of the application\"".dimmed()
    );
}
