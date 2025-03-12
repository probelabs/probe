use once_cell::sync::Lazy;
use regex::Regex;
use rig::completion::Message;
use rig::message::{AssistantContent, UserContent};

// Maximum number of previous messages to include in context
pub const MAX_HISTORY_MESSAGES: usize = 4;
// Maximum length of tool result to include in prompt (in characters)
pub const MAX_TOOL_RESULT_LENGTH: usize = 999999999;

// Define static regex patterns - more permissive to handle formatting variations
pub static TOOL_OUTPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<tool_output>\s*.*?\s*</tool_output>").unwrap());

// These regex patterns are currently unused but kept for future use
#[allow(dead_code)]
pub static MULTILINE_TOOL_OUTPUT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<tool_output>\s*.*?\s*</tool_output>").unwrap());

#[allow(dead_code)]
pub static SEARCH_RESULTS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)Here are the search results for query '.*?':\n.*?\n\nProvide a detailed")
        .unwrap()
});

#[allow(dead_code)]
pub static EXTRACT_RESULTS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)Here is the extracted code:\n.*?\n\nProvide a detailed").unwrap()
});

#[allow(dead_code)]
pub static QUERY_RESULTS_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)Here are the query results for pattern '.*?':\n.*?\n\nProvide a detailed")
        .unwrap()
});

// Helper function to simplify a search query by removing special operators
pub fn simplify_query(query: &str) -> String {
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
pub fn generate_history_summary(chat_history: &[Message]) -> String {
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
