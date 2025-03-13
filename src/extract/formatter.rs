//! Functions for formatting and printing extraction results.
//!
//! This module provides functions for formatting and printing extraction results
//! in various formats (terminal, markdown, plain, json, xml, color).

use crate::models::SearchResult;
use crate::search::search_tokens::count_tokens;
use anyhow::Result;
use std::path::Path;

/// Format the extraction results in the specified format and return as a string
///
/// # Arguments
///
/// * `results` - The search results to format
/// * `format` - The output format (terminal, markdown, plain, json, or color)
pub fn format_extraction_results(results: &[SearchResult], format: &str) -> Result<String> {
    use std::fmt::Write;
    let mut output = String::new();

    match format {
        "markdown" => {
            format_markdown_results(&mut output, results);
        }
        "plain" => {
            format_plain_results(&mut output, results);
        }
        "json" => {
            format_json_results(&mut output, results)?;
        }
        "xml" => {
            format_xml_results(&mut output, results)?;
        }
        "color" => {
            format_color_results(&mut output, results);
        }
        _ => {
            format_terminal_results(&mut output, results);
        }
    }

    // Add summary (only for non-JSON/XML formats)
    if format != "json" && format != "xml" {
        use colored::*;
        writeln!(output)?;
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

        // Calculate and add total bytes and tokens
        let total_bytes: usize = results.iter().map(|r| r.code.len()).sum();
        let total_tokens: usize = results.iter().map(|r| count_tokens(&r.code)).sum();
        writeln!(output, "Total bytes returned: {}", total_bytes)?;
        writeln!(output, "Total tokens returned: {}", total_tokens)?;
    }

    Ok(output)
}

/// Format and print the extraction results in the specified format
///
/// # Arguments
///
/// * `results` - The search results to format and print
/// * `format` - The output format (terminal, markdown, plain, json, or color)
#[allow(dead_code)]
pub fn format_and_print_extraction_results(results: &[SearchResult], format: &str) -> Result<()> {
    let output = format_extraction_results(results, format)?;
    println!("{}", output);
    Ok(())
}

/// Format results in terminal format with colors and write to a string buffer
pub fn format_terminal_results(output: &mut String, results: &[SearchResult]) {
    use colored::*;
    use std::fmt::Write;

    if results.is_empty() {
        writeln!(output, "{}", "No results found.".yellow().bold()).unwrap();
        return;
    }

    for result in results {
        // Get file extension
        let file_path = Path::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Write file info
        writeln!(output, "File: {}", result.file.yellow()).unwrap();

        // Write lines if not a full file
        if result.node_type != "file" {
            writeln!(output, "Lines: {}-{}", result.lines.0, result.lines.1).unwrap();
        }

        // Write node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            writeln!(output, "Type: {}", result.node_type.cyan()).unwrap();
        }

        // Determine the language for syntax highlighting
        let language = get_language_from_extension(extension);

        // Write the code with syntax highlighting
        if !language.is_empty() {
            writeln!(output, "```{}", language).unwrap();
        } else {
            writeln!(output, "```").unwrap();
        }

        writeln!(output, "{}", result.code).unwrap();
        writeln!(output, "```").unwrap();
        writeln!(output).unwrap();
    }
}

/// Format and print results in terminal format with colors
#[allow(dead_code)]
pub fn format_and_print_terminal_results(results: &[SearchResult]) {
    let mut output = String::new();
    format_terminal_results(&mut output, results);
    print!("{}", output);
}

/// Format results in markdown format and write to a string buffer
pub fn format_markdown_results(output: &mut String, results: &[SearchResult]) {
    use std::fmt::Write;

    if results.is_empty() {
        writeln!(output, "No results found.").unwrap();
        return;
    }

    for result in results {
        // Get file extension
        let file_path = Path::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Write file info
        writeln!(output, "## File: {}", result.file).unwrap();

        // Write lines if not a full file
        if result.node_type != "file" {
            writeln!(output, "Lines: {}-{}", result.lines.0, result.lines.1).unwrap();
        }

        // Write node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            writeln!(output, "Type: {}", result.node_type).unwrap();
        }

        // Determine the language for syntax highlighting
        let language = get_language_from_extension(extension);

        // Write the code with syntax highlighting
        if !language.is_empty() {
            writeln!(output, "```{}", language).unwrap();
        } else {
            writeln!(output, "```").unwrap();
        }

        writeln!(output, "{}", result.code).unwrap();
        writeln!(output, "```").unwrap();
        writeln!(output).unwrap();
    }
}

/// Format and print results in markdown format
#[allow(dead_code)]
pub fn format_and_print_markdown_results(results: &[SearchResult]) {
    let mut output = String::new();
    format_markdown_results(&mut output, results);
    print!("{}", output);
}

/// Format results in plain text format and write to a string buffer
pub fn format_plain_results(output: &mut String, results: &[SearchResult]) {
    use std::fmt::Write;

    if results.is_empty() {
        writeln!(output, "No results found.").unwrap();
        return;
    }

    for result in results {
        // Write file info
        writeln!(output, "File: {}", result.file).unwrap();

        // Write lines if not a full file
        if result.node_type != "file" {
            writeln!(output, "Lines: {}-{}", result.lines.0, result.lines.1).unwrap();
        }

        // Write node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            writeln!(output, "Type: {}", result.node_type).unwrap();
        }

        writeln!(output).unwrap();
        writeln!(output, "{}", result.code).unwrap();
        writeln!(output).unwrap();
        writeln!(output, "----------------------------------------").unwrap();
        writeln!(output).unwrap();
    }
}

/// Format and print results in plain text format
#[allow(dead_code)]
pub fn format_and_print_plain_results(results: &[SearchResult]) {
    let mut output = String::new();
    format_plain_results(&mut output, results);
    print!("{}", output);
}

/// Format results in XML format and write to a string buffer
pub fn format_xml_results(output: &mut String, results: &[SearchResult]) -> Result<()> {
    use std::fmt::Write;

    writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").unwrap();
    writeln!(output, "<extraction_results>").unwrap();

    for result in results {
        writeln!(output, "  <result>").unwrap();
        writeln!(output, "    <file>{}</file>", escape_xml(&result.file)).unwrap();

        if result.node_type != "file" {
            writeln!(
                output,
                "    <lines>{}-{}</lines>",
                result.lines.0, result.lines.1
            )
            .unwrap();
        }

        if result.node_type != "file" && result.node_type != "context" {
            writeln!(
                output,
                "    <node_type>{}</node_type>",
                escape_xml(&result.node_type)
            )
            .unwrap();
        }

        writeln!(output, "    <code><![CDATA[{}]]></code>", result.code).unwrap();
        writeln!(output, "  </result>").unwrap();
    }

    // Add summary section
    writeln!(output, "  <summary>").unwrap();
    writeln!(output, "    <count>{}</count>", results.len()).unwrap();
    writeln!(
        output,
        "    <total_bytes>{}</total_bytes>",
        results.iter().map(|r| r.code.len()).sum::<usize>()
    )
    .unwrap();
    writeln!(
        output,
        "    <total_tokens>{}</total_tokens>",
        results.iter().map(|r| count_tokens(&r.code)).sum::<usize>()
    )
    .unwrap();
    writeln!(output, "  </summary>").unwrap();

    writeln!(output, "</extraction_results>").unwrap();
    Ok(())
}

/// Format and print results in XML format
#[allow(dead_code)]
pub fn format_and_print_xml_results(results: &[SearchResult]) -> Result<()> {
    let mut output = String::new();
    format_xml_results(&mut output, results)?;
    print!("{}", output);
    Ok(())
}

/// Format results in JSON format and write to a string buffer
pub fn format_json_results(output: &mut String, results: &[SearchResult]) -> Result<()> {
    use std::fmt::Write;

    // Create a simplified version of the results for JSON output
    #[derive(serde::Serialize)]
    struct JsonResult<'a> {
        file: &'a str,
        lines: (usize, usize),
        node_type: &'a str,
        code: &'a str,
    }

    let json_results: Vec<JsonResult> = results
        .iter()
        .map(|r| JsonResult {
            file: &r.file,
            lines: r.lines,
            node_type: &r.node_type,
            code: &r.code,
        })
        .collect();

    // Create a wrapper object with results and summary
    let wrapper = serde_json::json!({
        "results": json_results,
        "summary": {
            "count": results.len(),
            "total_bytes": results.iter().map(|r| r.code.len()).sum::<usize>(),
            "total_tokens": results.iter().map(|r| count_tokens(&r.code)).sum::<usize>(),
        }
    });

    write!(output, "{}", serde_json::to_string_pretty(&wrapper)?)?;
    Ok(())
}

/// Format and print results in JSON format
#[allow(dead_code)]
pub fn format_and_print_json_results(results: &[SearchResult]) -> Result<()> {
    let mut output = String::new();
    format_json_results(&mut output, results)?;
    print!("{}", output);
    Ok(())
}

/// Format results with color highlighting and write to a string buffer
pub fn format_color_results(output: &mut String, results: &[SearchResult]) {
    use colored::*;
    use regex::Regex;
    use std::collections::HashSet;
    use std::fmt::Write;

    if results.is_empty() {
        writeln!(output, "No results found.").unwrap();
        return;
    }

    // Extract search terms from the results
    // We'll use the unique terms from the results if available
    let mut search_terms = HashSet::new();
    for result in results {
        if let Some(terms) = &result.file_unique_terms {
            if *terms > 0 {
                // If we have unique terms data, we can try to extract terms from the code
                // This is a simple approach - in a real implementation, you might want to
                // get the actual search terms from the search query
                let words: Vec<&str> = result.code.split_whitespace().collect();
                for word in words {
                    // Clean up the word (remove punctuation, etc.)
                    let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric());
                    if !clean_word.is_empty() {
                        search_terms.insert(clean_word.to_lowercase());
                    }
                }
            }
        }
    }

    // Use the search terms we extracted, or an empty list if none were found
    // This removes the default highlighting of common programming terms
    let default_terms: Vec<String> = search_terms.into_iter().collect();

    // Create regex patterns for the terms
    let mut patterns = Vec::new();
    for term in &default_terms {
        // Create a case-insensitive regex for the term
        // We use word boundaries to match whole words
        if let Ok(regex) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(term))) {
            patterns.push(regex);
        }
    }

    for result in results {
        // Get file extension
        let file_path = Path::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Write file info
        writeln!(output, "## File: {}", result.file).unwrap();

        // Write lines if not a full file
        if result.node_type != "file" {
            writeln!(output, "Lines: {}-{}", result.lines.0, result.lines.1).unwrap();
        }

        // Write node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            writeln!(output, "Type: {}", result.node_type).unwrap();
        }

        // Determine the language for syntax highlighting
        let language = get_language_from_extension(extension);

        // Write the code with syntax highlighting
        if !language.is_empty() {
            writeln!(output, "```{}", language).unwrap();
        } else {
            writeln!(output, "```").unwrap();
        }

        // Process the code line by line to highlight matching terms
        for line in result.code.lines() {
            let mut highlighted_line = line.to_string();

            // Apply highlighting for each pattern
            for pattern in &patterns {
                // Use a temporary string to build the highlighted line
                let mut temp_line = String::new();
                let mut last_end = 0;

                // Find all matches in the line
                for mat in pattern.find_iter(&highlighted_line) {
                    // Add the text before the match
                    temp_line.push_str(&highlighted_line[last_end..mat.start()]);

                    // Add the highlighted match
                    temp_line.push_str(&mat.as_str().yellow().bold().to_string());

                    last_end = mat.end();
                }

                // Add the remaining text
                temp_line.push_str(&highlighted_line[last_end..]);

                highlighted_line = temp_line;
            }

            writeln!(output, "{}", highlighted_line).unwrap();
        }

        writeln!(output, "```").unwrap();
        writeln!(output).unwrap();
    }
}

/// Format and print results with color highlighting
#[allow(dead_code)]
pub fn format_and_print_color_results(results: &[SearchResult]) {
    let mut output = String::new();
    format_color_results(&mut output, results);
    print!("{}", output);
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
