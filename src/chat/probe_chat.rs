use anyhow::Result;
use rig::completion::{Chat as RigChat, CompletionError, Message, Prompt, PromptError};
use rig::message::{AssistantContent, UserContent};
use std::sync::atomic::{AtomicUsize, Ordering};
use tiktoken_rs::cl100k_base;

use super::history::{
    generate_history_summary, simplify_query, MAX_HISTORY_MESSAGES, MAX_TOOL_RESULT_LENGTH,
    TOOL_OUTPUT_REGEX,
};
use super::models::ModelType;
use crate::search::cache;

pub struct ProbeChat {
    pub chat: ModelType,
    // Track token usage for the current session using atomic counters for thread safety
    request_tokens: AtomicUsize,
    response_tokens: AtomicUsize,
    // Tokenizer for counting tokens
    tokenizer: tiktoken_rs::CoreBPE,
    // Session ID for caching search results
    session_id: String,
}

impl ProbeChat {
    #[allow(dead_code)]
    pub fn new() -> Result<Self> {
        // Initialize the tokenizer with cl100k_base encoding (works for both Claude and GPT models)
        let tokenizer = cl100k_base().unwrap();

        // Generate a unique session ID for this chat instance
        let (session_id, _) = cache::generate_session_id()?;
        let session_id = session_id.to_string();

        // Print the session ID for debugging
        let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";
        if debug_mode {
            println!("[DEBUG] Generated session ID for chat: {}", session_id);
        }

        // Store the session ID in an environment variable for tools to access
        std::env::set_var("PROBE_SESSION_ID", &session_id);

        // Common preamble for both models
        let preamble = r#"You are code intelligence assistant powered by the Probe. 
        Always use the given tools for searching the code. 
        Rather then guessing, start with using `search` tool, with exact keywords, and extend your search deeper. 
        AVOID reading full files, unless absolutelly necessary. 
        Use this tools as a scalpel, not a hammer. 
        Use 'exact' parameter if you looking for something specific. 
        Avoid searching with too common keywords, like 'if', 'for', 'while', etc. 
        If you need to read a full file or extract a specific code block, use `extract` tool. 
        If you need to find a specific code structure, use `query` tool. 
        If you are unsure about the results, refine your query or ask for clarification.
        
        Leverage given tools effectively:
            1. `search`:
                - Use simple, unique keywords, avoid general terms (e.g., 'rpc' over 'rpc layer')\n
                - Use ElasticSearch query language: ALWAYS use + for required terms, ".." for exact match, and omit for general and optional words, - for excluded terms, and AND/OR for logic. Prefer explicit searches, with this syntax.
            2. `query` 
                - Craft tree-sitter patterns (e.g., 'fn $NAME($$$PARAMS) $$$BODY') for specific structures
                - Match patterns to the language (e.g., Rust, Python)
                - Use sparingly for precise structural queries
            3. `extract`
                - Extract blocks by line number (e.g., '/file.rs:42') or full files for context
                - Include `contextLines` only if AST parsing fails
            
        **Approach**:
            - Start with a clear search strategy
            - Interpret results concisely, tying them to the user’s question
            - If unsure, refine queries or ask for clarification.
        "#;

        // Create the model
        let model_type = super::models::create_model(preamble)?;

        Ok(Self {
            chat: model_type,
            request_tokens: AtomicUsize::new(0),
            response_tokens: AtomicUsize::new(0),
            tokenizer,
            session_id,
        })
    }

    // Count tokens in a string using tiktoken
    pub fn count_tokens(&self, text: &str) -> usize {
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

    // Get the session ID for this chat instance
    pub fn get_session_id(&self) -> &str {
        &self.session_id
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
                        let parts: Vec<&str> = tool_call.splitn(2, char::is_whitespace).collect();
                        if parts.len() == 2
                            && (parts[0] == "search"
                                || parts[0] == "query"
                                || parts[0] == "extract")
                        {
                            let tool_name = parts[0];
                            // The entire JSON, including internal spaces, is now in parts[1]
                            let maybe_json = parts[1].trim();

                            // Find the start of the JSON object (the first '{')
                            if let Some(json_start) = maybe_json.find('{') {
                                // Extract everything from the first '{' to the end of the string
                                let raw_json = &maybe_json[json_start..];

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
                                                simplify_query(&tool_params_clone);

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
                                            generate_history_summary(&chat_history);

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
                                                if tool_name == "extract" {
                                                    format!(
                                                        "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nHere is the extracted code:\n{}\n\nProvide a detailed answer based on this code.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        truncated_result
                                                    )
                                                } else if tool_name == "query" {
                                                    format!(
                                                        "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nHere are the query results for pattern '{}':\n{}\n\nProvide a detailed answer based on these query results.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        tool_params_clone,
                                                        truncated_result
                                                    )
                                                } else {
                                                    format!(
                                                        "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nHere are the search results for query '{}':\n{}\n\nProvide a detailed answer based on these search results.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        tool_params_clone,
                                                        truncated_result
                                                    )
                                                }
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

                                                // Create a new prompt that includes chat history, original question, and tool results
                                                let direct_prompt = if tool_name == "extract" {
                                                    format!(
                                                        "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI extracted the following code:\n{}\n\nBased on this code, provide a detailed answer about the user's question.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        truncated_result
                                                    )
                                                } else if tool_name == "query" {
                                                    format!(
                                                        "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI queried the codebase with a pattern and found:\n{}\n\nBased on these query results, provide a detailed answer about the user's question.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        truncated_result
                                                    )
                                                } else {
                                                    format!(
                                                        "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI searched the codebase and found:\n{}\n\nBased on these search results, provide a detailed answer about the user's question.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        truncated_result
                                                    )
                                                };

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
                                            generate_history_summary(&chat_history);

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
                                                if tool_name == "extract" {
                                                    format!(
                                                        "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nI tried to extract code from '{}' but encountered this error: {}\n\nPlease apologize to the user and suggest alternatives.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        tool_params_clone,
                                                        e
                                                    )
                                                } else if tool_name == "query" {
                                                    format!(
                                                        "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nI tried to query the codebase with pattern '{}' but encountered this error: {}\n\nPlease apologize to the user and suggest alternative patterns.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        tool_params_clone,
                                                        e
                                                    )
                                                } else {
                                                    format!(
                                                        "You are a code search assistant. Previous conversation:\n{}\n\nThe user asked: {}\n\nI tried to search with query '{}' but encountered this error: {}\n\nPlease apologize to the user and suggest alternative search terms.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        tool_params_clone,
                                                        e
                                                    )
                                                }
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
                                                let direct_error_prompt = if tool_name == "extract"
                                                {
                                                    format!(
                                                        "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI tried to extract code from the file but encountered an error: {}\n\nPlease apologize to the user and suggest alternatives.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        e
                                                    )
                                                } else if tool_name == "query" {
                                                    format!(
                                                        "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI tried to query the codebase with a pattern but encountered an error: {}\n\nPlease apologize to the user and suggest alternative patterns.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        e
                                                    )
                                                } else {
                                                    format!(
                                                        "Previous conversation:\n{}\n\nThe user asked: '{}'\n\nI tried to search the codebase with the query but encountered an error: {}\n\nPlease apologize to the user and suggest alternative search terms.",
                                                        history_summary,
                                                        latest_user_text_for_continuation,
                                                        e
                                                    )
                                                };

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
