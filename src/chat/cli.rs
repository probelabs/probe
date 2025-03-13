use anyhow::Result;
use colored::*;
use rig::completion::{Chat, Message};
use rig::message::{AssistantContent, Text, UserContent};
use rig::one_or_many::OneOrMany;
use rig::providers::{anthropic::CLAUDE_3_5_SONNET, openai::GPT_4O};
use std::io::Write;

use super::history::{
    EXTRACT_RESULTS_REGEX, MULTILINE_TOOL_OUTPUT_REGEX, QUERY_RESULTS_REGEX, SEARCH_RESULTS_REGEX,
};
use super::models::ModelType;
use super::probe_chat::ProbeChat;

#[allow(dead_code)]
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
        format!(
            "Session ID: {} (used for caching search results)",
            chat.get_session_id().bright_yellow()
        )
        .dimmed()
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
                            response.to_string()
                        }
                    }
                    ModelType::OpenAI(_) => {
                        // For OpenAI, look for the search results, query results, or extract results section with our new format
                        if SEARCH_RESULTS_REGEX.is_match(&response) {
                            SEARCH_RESULTS_REGEX
                                .replace_all(
                                    &response,
                                    "I have analyzed the search results. Provide a detailed",
                                )
                                .to_string()
                        } else if EXTRACT_RESULTS_REGEX.is_match(&response) {
                            EXTRACT_RESULTS_REGEX
                                .replace_all(
                                    &response,
                                    "I have analyzed the extracted code. Provide a detailed",
                                )
                                .to_string()
                        } else if QUERY_RESULTS_REGEX.is_match(&response) {
                            QUERY_RESULTS_REGEX
                                .replace_all(
                                    &response,
                                    "I have analyzed the query results. Provide a detailed",
                                )
                                .to_string()
                        } else {
                            // If no match found, just use the original response
                            response.to_string()
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
                    content: OneOrMany::one(AssistantContent::Text(Text {
                        text: response.to_string(),
                    })),
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

#[allow(dead_code)]
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
    println!(
        "{}",
        "  \"Extract the main function from src/main.rs\"".dimmed()
    );
    println!(
        "{}",
        "  \"Show me the implementation of the extract tool\"".dimmed()
    );
}
