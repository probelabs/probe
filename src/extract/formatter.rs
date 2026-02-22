//! Functions for formatting and printing extraction results.
//!
//! This module provides functions for formatting and printing extraction results
//! in various formats (terminal, markdown, plain, json, xml, color).

use anyhow::Result;
use probe_code::models::SearchResult;
use probe_code::search::search_tokens::sum_tokens_with_deduplication;
use serde::Serialize;
use std::fmt::Write as FmtWrite;
use std::path::Path;

use super::outline_diff_formatter;

/// A single internal function that handles both dry-run and non-dry-run formatting.
///
/// # Arguments
///
/// * `results` - The search results to format
/// * `format` - The output format (terminal, markdown, plain, json, or color)
/// * `original_input` - Optional original user input
/// * `system_prompt` - Optional system prompt for LLM models
/// * `user_instructions` - Optional user instructions for LLM models
/// * `is_dry_run` - Whether this is a dry-run request (only file names/line numbers)
/// * `symbols` - Whether to show symbol signatures instead of full code
fn format_extraction_internal(
    results: &[SearchResult],
    format: &str,
    original_input: Option<&str>,
    system_prompt: Option<&str>,
    user_instructions: Option<&str>,
    is_dry_run: bool,
    symbols: bool,
) -> Result<String> {
    let mut output = String::new();

    // Handle outline-diff format separately
    if format == "outline-diff" {
        return outline_diff_formatter::format_outline_diff(results, original_input);
    }

    match format {
        // ---------------------------------------
        // JSON output
        // ---------------------------------------
        "json" => {
            if is_dry_run {
                // DRY-RUN JSON structure
                #[derive(Serialize)]
                struct JsonDryRunResult<'a> {
                    file: &'a str,
                    #[serde(serialize_with = "serialize_lines_as_array")]
                    lines: (usize, usize),
                    node_type: &'a str,
                }

                // Helper function to serialize lines as an array
                fn serialize_lines_as_array<S>(
                    lines: &(usize, usize),
                    serializer: S,
                ) -> std::result::Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    use serde::ser::SerializeSeq;
                    let mut seq = serializer.serialize_seq(Some(2))?;
                    seq.serialize_element(&lines.0)?;
                    seq.serialize_element(&lines.1)?;
                    seq.end()
                }

                let json_results: Vec<JsonDryRunResult> = results
                    .iter()
                    .map(|r| JsonDryRunResult {
                        file: &r.file,
                        lines: r.lines,
                        node_type: &r.node_type,
                    })
                    .collect();

                // Create a wrapper object with results and summary
                let mut wrapper = serde_json::json!({
                    "results": json_results,
                    "summary": {
                        "count": results.len(),
                    },
                    "version": probe_code::version::get_version()
                });

                // Add system prompt, user instructions, and original_input if provided
                if let Some(prompt) = system_prompt {
                    wrapper["system_prompt"] = serde_json::Value::String(prompt.to_string());
                }

                if let Some(instructions) = user_instructions {
                    wrapper["user_instructions"] =
                        serde_json::Value::String(instructions.to_string());
                }

                if let Some(input) = original_input {
                    wrapper["original_input"] = serde_json::Value::String(input.to_string());
                }

                write!(output, "{}", serde_json::to_string_pretty(&wrapper)?)?;
            } else {
                // NON-DRY-RUN JSON structure
                #[derive(Serialize)]
                struct JsonResult<'a> {
                    file: &'a str,
                    #[serde(serialize_with = "serialize_lines_as_array")]
                    lines: (usize, usize),
                    node_type: &'a str,
                    code: &'a str,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    symbol_signature: Option<&'a String>,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    original_input: Option<&'a str>,
                }

                // Helper function to serialize lines as an array
                fn serialize_lines_as_array<S>(
                    lines: &(usize, usize),
                    serializer: S,
                ) -> std::result::Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    use serde::ser::SerializeSeq;
                    let mut seq = serializer.serialize_seq(Some(2))?;
                    seq.serialize_element(&lines.0)?;
                    seq.serialize_element(&lines.1)?;
                    seq.end()
                }

                let json_results: Vec<JsonResult> = results
                    .iter()
                    .map(|r| JsonResult {
                        file: &r.file,
                        lines: r.lines,
                        node_type: &r.node_type,
                        code: &r.code,
                        symbol_signature: r.symbol_signature.as_ref(),
                        // We no longer put original_input per result. If you truly need it,
                        // you can uncomment the line below, but it's typically at the root.
                        // original_input: r.original_input.as_deref(),
                        original_input: None,
                    })
                    .collect();

                // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for extract JSON output:
                // Process all code blocks in batch to leverage content deduplication
                let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
                let total_tokens = if symbols {
                    // In symbols mode, count tokens from symbol signatures instead of full code
                    let symbol_blocks: Vec<&str> = results
                        .iter()
                        .filter_map(|r| r.symbol_signature.as_deref())
                        .collect();
                    sum_tokens_with_deduplication(&symbol_blocks)
                } else {
                    sum_tokens_with_deduplication(&code_blocks)
                };

                // Create a wrapper object with results and summary
                let mut wrapper = serde_json::json!({
                    "results": json_results,
                    "summary": {
                        "count": results.len(),
                        "total_bytes": if symbols {
                            results.iter().map(|r| {
                                r.symbol_signature.as_ref().map(|s| s.len()).unwrap_or(0)
                            }).sum::<usize>()
                        } else {
                            results.iter().map(|r| r.code.len()).sum::<usize>()
                        },
                        "total_tokens": total_tokens,
                    },
                    "version": probe_code::version::get_version()
                });

                // Add system prompt, user instructions, and original_input if provided
                if let Some(input) = original_input {
                    wrapper["original_input"] = serde_json::Value::String(input.to_string());
                }

                if let Some(prompt) = system_prompt {
                    wrapper["system_prompt"] = serde_json::Value::String(prompt.to_string());
                }

                if let Some(instructions) = user_instructions {
                    wrapper["user_instructions"] =
                        serde_json::Value::String(instructions.to_string());
                }

                write!(output, "{}", serde_json::to_string_pretty(&wrapper)?)?;
            }
        }

        // ---------------------------------------
        // XML output
        // ---------------------------------------
        "xml" => {
            // XML declaration
            writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
            // Open the root tag
            writeln!(output, "<probe_results>")?;

            if is_dry_run {
                // DRY-RUN: no code, just file/lines/node_type
                for result in results {
                    writeln!(output, "  <result>")?;
                    writeln!(output, "    <file>{}</file>", escape_xml(&result.file))?;

                    if result.node_type != "file" {
                        writeln!(output, "    <lines>")?;
                        writeln!(output, "      <start>{}</start>", result.lines.0)?;
                        writeln!(output, "      <end>{}</end>", result.lines.1)?;
                        writeln!(output, "    </lines>")?;
                    }

                    if result.node_type != "file" && result.node_type != "context" {
                        writeln!(
                            output,
                            "    <node_type>{}</node_type>",
                            escape_xml(&result.node_type)
                        )?;
                    }

                    writeln!(output, "  </result>")?;
                }
                // Summary
                writeln!(output, "  <summary>")?;
                writeln!(output, "    <count>{}</count>", results.len())?;
                writeln!(output, "  </summary>")?;
                writeln!(
                    output,
                    "  <version>{}</version>",
                    probe_code::version::get_version()
                )?;
            } else {
                // NON-DRY-RUN: includes code
                for result in results {
                    writeln!(output, "  <result>")?;
                    writeln!(output, "    <file>{}</file>", escape_xml(&result.file))?;

                    if result.node_type != "file" {
                        writeln!(output, "    <lines>")?;
                        writeln!(output, "      <start>{}</start>", result.lines.0)?;
                        writeln!(output, "      <end>{}</end>", result.lines.1)?;
                        writeln!(output, "    </lines>")?;
                    }

                    if result.node_type != "file" && result.node_type != "context" {
                        writeln!(output, "    <node_type>{}</node_type>", &result.node_type)?;
                    }

                    // Include symbol signature if available
                    if let Some(symbol_signature) = &result.symbol_signature {
                        writeln!(
                            output,
                            "    <symbol_signature><![CDATA[{}]]></symbol_signature>",
                            symbol_signature
                        )?;
                    }

                    // Use CDATA to preserve formatting and special characters
                    writeln!(output, "    <code><![CDATA[{}]]></code>", &result.code)?;

                    writeln!(output, "  </result>")?;
                }

                // Summary
                writeln!(output, "  <summary>")?;
                writeln!(output, "    <count>{}</count>", results.len())?;
                writeln!(
                    output,
                    "    <total_bytes>{}</total_bytes>",
                    if symbols {
                        results
                            .iter()
                            .map(|r| r.symbol_signature.as_ref().map(|s| s.len()).unwrap_or(0))
                            .sum::<usize>()
                    } else {
                        results.iter().map(|r| r.code.len()).sum::<usize>()
                    }
                )?;
                // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for extract XML output:
                // Process all code blocks in batch to leverage content deduplication
                let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
                let total_tokens = if symbols {
                    // In symbols mode, count tokens from symbol signatures instead of full code
                    let symbol_blocks: Vec<&str> = results
                        .iter()
                        .filter_map(|r| r.symbol_signature.as_deref())
                        .collect();
                    sum_tokens_with_deduplication(&symbol_blocks)
                } else {
                    sum_tokens_with_deduplication(&code_blocks)
                };

                writeln!(output, "    <total_tokens>{total_tokens}</total_tokens>")?;
                writeln!(output, "  </summary>")?;
                writeln!(
                    output,
                    "  <version>{}</version>",
                    probe_code::version::get_version()
                )?;
            }

            // Add original_input, system_prompt, and user_instructions inside the root element
            if let Some(input) = original_input {
                writeln!(
                    output,
                    "  <original_input><![CDATA[{input}]]></original_input>"
                )?;
            }

            if let Some(prompt) = system_prompt {
                writeln!(
                    output,
                    "  <system_prompt><![CDATA[{prompt}]]></system_prompt>"
                )?;
            }

            if let Some(instructions) = user_instructions {
                writeln!(
                    output,
                    "  <user_instructions><![CDATA[{instructions}]]></user_instructions>"
                )?;
            }

            // Close the root tag
            writeln!(output, "</probe_results>")?;
        }

        // ---------------------------------------
        // All other formats (terminal, markdown, plain, color)
        // ---------------------------------------
        _ => {
            use colored::*;

            // If there are no results
            if results.is_empty() {
                writeln!(output, "{}", "No results found.".yellow().bold())?;
            } else {
                // Disambiguation hint: when multiple results from the same file share
                // the same base symbol name, print a note with qualified selectors.
                if results.len() > 1 {
                    // Group by file to detect same-file duplicates
                    use std::collections::HashMap;
                    let mut by_file: HashMap<&str, Vec<&SearchResult>> = HashMap::new();
                    for r in results {
                        by_file.entry(&r.file).or_default().push(r);
                    }
                    for file_results in by_file.values() {
                        // Only show hint if multiple results in the same file have symbol_signature
                        let with_sig: Vec<_> = file_results
                            .iter()
                            .filter(|r| r.symbol_signature.is_some())
                            .collect();
                        if with_sig.len() > 1 {
                            // Extract the base symbol name from the first signature
                            let base_name = with_sig[0]
                                .symbol_signature
                                .as_ref()
                                .map(|s| s.rsplit('.').next().unwrap_or(s.as_str()))
                                .unwrap_or("unknown");
                            writeln!(
                                output,
                                "{}",
                                format!(
                                    "Note: Found {} symbols named \"{}\". Use qualified names to disambiguate:",
                                    with_sig.len(),
                                    base_name
                                )
                                .yellow()
                                .bold()
                            )?;
                            for r in &with_sig {
                                writeln!(
                                    output,
                                    "  - {} ({}, line {})",
                                    r.symbol_signature.as_deref().unwrap_or("?"),
                                    r.node_type,
                                    r.lines.0
                                )?;
                            }
                            writeln!(output)?;
                        }
                    }
                }

                // For each result, we either skip the code if is_dry_run, or include it otherwise.
                for result in results {
                    // Common: show file (with format-specific prefix)
                    if format == "markdown" {
                        writeln!(output, "## File: {}", result.file.yellow())?;
                    } else {
                        writeln!(output, "File: {}", result.file.yellow())?;
                    }

                    // Show lines if not a full file
                    if result.node_type != "file" {
                        if format == "markdown" {
                            writeln!(output, "### Lines: {}-{}", result.lines.0, result.lines.1)?;
                        } else {
                            writeln!(output, "Lines: {}-{}", result.lines.0, result.lines.1)?;
                        }
                    }

                    // Show node type if not file/context
                    if result.node_type != "file" && result.node_type != "context" {
                        if format == "markdown" {
                            writeln!(output, "### Type: {}", result.node_type.cyan())?;
                        } else {
                            writeln!(output, "Type: {}", result.node_type.cyan())?;
                        }
                    }

                    // In dry-run, we do NOT print the code or symbols
                    if !is_dry_run {
                        // Check if we should display symbols instead of code
                        if symbols {
                            if let Some(symbol_signature) = &result.symbol_signature {
                                if format == "markdown" {
                                    writeln!(output, "### Symbol: {}", symbol_signature)?;
                                } else {
                                    writeln!(output, "Symbol: {}", symbol_signature)?;
                                }
                            } else if format == "markdown" {
                                writeln!(output, "### Symbol: *not available*")?;
                            } else {
                                writeln!(output, "Symbol: <not available>")?;
                            }
                        } else {
                            // Show full code (existing behavior)
                            // Attempt a basic "highlight" approach by checking file extension
                            let extension = Path::new(&result.file)
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .unwrap_or("");
                            let language = get_language_from_extension(extension);

                            match format {
                                "markdown" => {
                                    if !language.is_empty() {
                                        writeln!(output, "```{language}")?;
                                    } else {
                                        writeln!(output, "```")?;
                                    }
                                    writeln!(output, "{}", result.code)?;
                                    writeln!(output, "```")?;
                                }
                                "plain" => {
                                    writeln!(output)?;
                                    writeln!(output, "{}", result.code)?;
                                    writeln!(output)?;
                                    writeln!(output, "----------------------------------------")?;
                                    writeln!(output)?;
                                }
                                "color" => {
                                    if !language.is_empty() {
                                        writeln!(output, "```{language}")?;
                                    } else {
                                        writeln!(output, "```")?;
                                    }
                                    writeln!(output, "{}", result.code)?;
                                    writeln!(output, "```")?;
                                }
                                // "terminal" or anything else not covered
                                _ => {
                                    if !language.is_empty() {
                                        writeln!(output, "```{language}")?;
                                    } else {
                                        writeln!(output, "```")?;
                                    }
                                    writeln!(output, "{}", result.code)?;
                                    writeln!(output, "```")?;
                                }
                            }
                        }
                    }

                    writeln!(output)?;
                }
            }

            // Now, print the root-level data (system prompt, user instructions, original input)
            if let Some(input) = original_input {
                writeln!(output, "{}", "Original Input:".yellow().bold())?;
                writeln!(output, "{input}")?;
            }
            if let Some(prompt) = system_prompt {
                writeln!(output)?;
                writeln!(output, "{}", "System Prompt:".yellow().bold())?;
                writeln!(output, "{prompt}")?;
            }
            if let Some(instructions) = user_instructions {
                writeln!(output)?;
                writeln!(output, "{}", "User Instructions:".yellow().bold())?;
                writeln!(output, "{instructions}")?;
            }

            // Summaries for non-JSON/XML:
            if !["json", "xml"].contains(&format) && !results.is_empty() {
                writeln!(output)?;
                if is_dry_run {
                    writeln!(
                        output,
                        "{} {} {}",
                        "Would extract".green().bold(),
                        results.len(),
                        if results.len() == 1 {
                            "result"
                        } else {
                            "results"
                        }
                    )?;
                } else {
                    writeln!(
                        output,
                        "{} {} {}",
                        "Extracted".green().bold(),
                        results.len(),
                        if results.len() == 1 {
                            "result"
                        } else {
                            "results"
                        }
                    )?;

                    let total_bytes: usize = if symbols {
                        results
                            .iter()
                            .map(|r| r.symbol_signature.as_ref().map(|s| s.len()).unwrap_or(0))
                            .sum()
                    } else {
                        results.iter().map(|r| r.code.len()).sum()
                    };

                    // BATCH TOKENIZATION WITH DEDUPLICATION OPTIMIZATION for extract terminal output:
                    // Process all code blocks in batch to leverage content deduplication
                    let code_blocks: Vec<&str> = results.iter().map(|r| r.code.as_str()).collect();
                    let total_tokens: usize = if symbols {
                        // In symbols mode, count tokens from symbol signatures instead of full code
                        let symbol_blocks: Vec<&str> = results
                            .iter()
                            .filter_map(|r| r.symbol_signature.as_deref())
                            .collect();
                        sum_tokens_with_deduplication(&symbol_blocks)
                    } else {
                        sum_tokens_with_deduplication(&code_blocks)
                    };
                    writeln!(output, "Total bytes returned: {total_bytes}")?;
                    writeln!(output, "Total tokens returned: {total_tokens}")?;
                }
            }
        }
    }

    Ok(output)
}

/// Format the extraction results for dry-run mode (only file names and line numbers)
///
/// # Arguments
///
/// * `results` - The search results to format
/// * `format` - The output format (terminal, markdown, plain, json, or color)
/// * `system_prompt` - Optional system prompt for LLM models
/// * `user_instructions` - Optional user instructions for LLM models
/// * `symbols` - Whether to show symbol signatures instead of full code
pub fn format_extraction_dry_run(
    results: &[SearchResult],
    format: &str,
    original_input: Option<&str>,
    system_prompt: Option<&str>,
    user_instructions: Option<&str>,
    symbols: bool,
) -> Result<String> {
    format_extraction_internal(
        results,
        format,
        original_input,
        system_prompt,
        user_instructions,
        true, // is_dry_run
        symbols,
    )
}

/// Format the extraction results in the specified format and return as a string
///
/// # Arguments
///
/// * `results` - The search results to format
/// * `format` - The output format (terminal, markdown, plain, json, or color)
/// * `system_prompt` - Optional system prompt for LLM models
/// * `user_instructions` - Optional user instructions for LLM models
/// * `symbols` - Whether to show symbol signatures instead of full code
pub fn format_extraction_results(
    results: &[SearchResult],
    format: &str,
    original_input: Option<&str>,
    system_prompt: Option<&str>,
    user_instructions: Option<&str>,
    symbols: bool,
) -> Result<String> {
    format_extraction_internal(
        results,
        format,
        original_input,
        system_prompt,
        user_instructions,
        false, // is_dry_run
        symbols,
    )
}

/// Format and print the extraction results in the specified format
///
/// # Arguments
///
/// * `results` - The search results to format and print
/// * `format` - The output format (terminal, markdown, plain, json, or color)
/// * `system_prompt` - Optional system prompt for LLM models
/// * `user_instructions` - Optional user instructions for LLM models
/// * `symbols` - Whether to show symbol signatures instead of full code
#[allow(dead_code)]
pub fn format_and_print_extraction_results(
    results: &[SearchResult],
    format: &str,
    original_input: Option<&str>,
    system_prompt: Option<&str>,
    user_instructions: Option<&str>,
    symbols: bool,
) -> Result<()> {
    let output = format_extraction_results(
        results,
        format,
        original_input,
        system_prompt,
        user_instructions,
        symbols,
    )?;
    println!("{output}");
    Ok(())
}

/// Helper function to escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&apos;")
}

/// Get the language name from a file extension for syntax highlighting
pub fn get_language_from_extension(extension: &str) -> &'static str {
    match extension {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "java" => "java",
        "rb" => "ruby",
        "php" => "php",
        "sh" => "bash",
        "md" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "cs" => "csharp",
        "scala" => "scala",
        "dart" => "dart",
        "ex" | "exs" => "elixir",
        "hs" => "haskell",
        "clj" => "clojure",
        "lua" => "lua",
        "r" => "r",
        "pl" | "pm" => "perl",
        "proto" => "protobuf",
        _ => "",
    }
}
