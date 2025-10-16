//! Outline-diff format: Semantically enhanced git diff output
//!
//! This module provides functionality to take git diff output and enhance it by:
//! 1. Parsing the diff to identify changed lines
//! 2. Using AST to find semantic contexts (functions/classes) containing changes
//! 3. Outputting in unified diff format with expanded semantic context
//! 4. Adding +/- prefixes to show additions and deletions

use anyhow::Result;
use probe_code::models::SearchResult;
use probe_code::search::search_output::{
    collect_outline_lines, create_file_content_cache, OutlineLineType,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::sync::Arc;

/// Type of line in a diff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffKind {
    Context,
    Add,
    Remove,
}

/// A single line from the diff with its line numbers and content
#[derive(Debug, Clone)]
struct DiffLine {
    kind: DiffKind,
    old_no: Option<usize>, // present for Context & Remove
    new_no: Option<usize>, // present for Context & Add
    text: String,          // line content without the +/-/space prefix
}

/// Rendering operation for a single logical change
#[derive(Debug)]
enum RenderOp<'a> {
    Context(&'a DiffLine),
    Add(&'a DiffLine),
    Remove(&'a DiffLine),
    Replace {
        old: &'a DiffLine,
        new: &'a DiffLine,
    },
    Gap,
}

/// Coalesce consecutive removes followed by adds into Replace operations
/// This makes the output much more readable by showing "old -> new" on one line
fn coalesce_replacements<'a>(lines: &'a [DiffLine]) -> Vec<RenderOp<'a>> {
    let mut ops = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        match lines[i].kind {
            DiffKind::Remove => {
                // Collect contiguous removes
                let r0 = i;
                while i < lines.len() && lines[i].kind == DiffKind::Remove {
                    i += 1;
                }

                // Collect contiguous adds right after
                let a0 = i;
                while i < lines.len() && lines[i].kind == DiffKind::Add {
                    i += 1;
                }

                let removes = &lines[r0..a0];
                let adds = &lines[a0..i];
                let n = removes.len().min(adds.len());

                // Pair up what we can
                for k in 0..n {
                    ops.push(RenderOp::Replace {
                        old: &removes[k],
                        new: &adds[k],
                    });
                }

                // Leftover removes (if removes > adds)
                for r in removes.iter().skip(n) {
                    ops.push(RenderOp::Remove(r));
                }

                // Leftover adds (if adds > removes)
                for a in adds.iter().skip(n) {
                    ops.push(RenderOp::Add(a));
                }
            }
            DiffKind::Add => {
                ops.push(RenderOp::Add(&lines[i]));
                i += 1;
            }
            DiffKind::Context => {
                ops.push(RenderOp::Context(&lines[i]));
                i += 1;
            }
        }
    }

    ops
}

/// Format extraction results in outline-diff format
///
/// This function takes SearchResults (which contain file paths and line ranges with matched_lines)
/// and the original raw diff text, then formats them as semantically-enhanced git diff output
/// using the outline rendering logic with proper dual-number gutters for additions and deletions.
pub fn format_outline_diff(results: &[SearchResult], raw_diff: Option<&str>) -> Result<String> {
    let mut output = String::new();

    if results.is_empty() {
        return Ok("No results found.\n".to_string());
    }

    // Parse the raw diff to get DiffLine structures with old/new line numbers
    let diff_lines_by_file = if let Some(diff_text) = raw_diff {
        parse_diff(diff_text)?
    } else {
        HashMap::new()
    };

    // Convert to refs for outline functions
    let result_refs: Vec<&SearchResult> = results.iter().collect();

    // Create file content cache (reuse from outline logic)
    let file_cache = create_file_content_cache(&result_refs);

    // Group results by file
    let mut results_by_file: HashMap<PathBuf, Vec<&SearchResult>> = HashMap::new();
    for result in &result_refs {
        let path = PathBuf::from(&result.file);
        results_by_file
            .entry(path.clone())
            .or_default()
            .push(result);
    }

    // Sort files alphabetically for consistent output
    let mut sorted_files: Vec<_> = results_by_file.keys().collect();
    sorted_files.sort();

    // Process each file
    for file_path in sorted_files {
        let file_results = &results_by_file[file_path];
        let diff_lines = diff_lines_by_file.get(file_path);
        format_file_outline_diff(
            &mut output,
            file_path,
            file_results,
            &file_cache,
            diff_lines,
        )?;
    }

    Ok(output)
}

/// Format a single file's results in outline-diff format using outline rendering logic
fn format_file_outline_diff(
    output: &mut String,
    file_path: &PathBuf,
    results: &[&SearchResult],
    file_cache: &HashMap<PathBuf, Arc<String>>,
    diff_lines: Option<&Vec<DiffLine>>,
) -> Result<()> {
    // Write diff header
    writeln!(
        output,
        "diff --git a/{} b/{}",
        file_path.display(),
        file_path.display()
    )?;
    writeln!(output, "index 00000000..11111111 100644")?;
    writeln!(output, "--- a/{}", file_path.display())?;
    writeln!(output, "+++ b/{}", file_path.display())?;

    // Get source lines from cache
    let source = match file_cache.get(file_path) {
        Some(content) => content,
        None => return Err(anyhow::anyhow!("File not found in cache: {:?}", file_path)),
    };
    let source_lines: Vec<&str> = source.lines().collect();

    // For each result, use the outline logic to collect lines to display
    for result in results {
        let file_path_str = file_path.to_string_lossy();

        // Use the outline logic to collect lines with their types
        // Trust the outline logic - it provides semantic context
        let (lines_with_types, _closing_brace_contexts) =
            collect_outline_lines(result, &file_path_str, file_cache);

        // Get the set of matched (changed) lines from the result
        // matched_lines contains relative line numbers within the extracted range
        let matched_lines_absolute: HashSet<usize> = result
            .matched_lines
            .as_ref()
            .map(|v| {
                v.iter()
                    .map(|&rel_line| result.lines.0 + rel_line - 1)
                    .collect()
            })
            .unwrap_or_default();

        // Debug output
        if std::env::var("DEBUG").unwrap_or_default() == "1" {
            eprintln!(
                "[DEBUG outline-diff] matched_lines (relative): {:?}",
                result.matched_lines
            );
            eprintln!(
                "[DEBUG outline-diff] matched_lines (absolute): {:?}",
                matched_lines_absolute
            );
            eprintln!(
                "[DEBUG outline-diff] outline collected {} lines",
                lines_with_types.len()
            );
        }

        // Write hunk header with semantic context (first line of code)
        // Extract the first meaningful line as context
        let context = result
            .code
            .lines()
            .next()
            .unwrap_or(&result.node_type)
            .trim();

        writeln!(
            output,
            "@@ -{},{} +{},{} @@ {}",
            result.lines.0,
            result.lines.1 - result.lines.0 + 1,
            result.lines.0,
            result.lines.1 - result.lines.0 + 1,
            context
        )?;

        // Render lines with smart gap handling and dual-number gutters
        render_outline_lines_as_diff(
            output,
            &lines_with_types,
            &matched_lines_absolute,
            &source_lines,
            diff_lines,
        )?;
    }

    Ok(())
}

/// Render a single DiffLine with prefix after the line number
/// Format: "123+ code" or "123- code" or "123  code"
/// hide_numbers: if true, skip line numbers (for replacements)
/// width: column width for line numbers
fn render_line(
    dl: &DiffLine,
    prefix: char,
    hide_numbers: bool,
    width: usize,
    output: &mut String,
) -> Result<()> {
    if hide_numbers {
        // No line numbers - use spaces matching the width
        let padding = " ".repeat(width + 1);
        writeln!(output, "{} {}", padding, dl.text)?;
    } else {
        match (dl.old_no, dl.new_no) {
            (Some(num), Some(_)) | (Some(num), None) | (None, Some(num)) => {
                // Show line number with prefix after: "123+" or "123-" or "123 "
                let display_num = dl.new_no.or(dl.old_no).unwrap_or(num);
                writeln!(output, "{:>width$}{} {}", display_num, prefix, dl.text)?;
            }
            (None, None) => {
                // Gap or other
                writeln!(output, "{}", dl.text)?;
            }
        }
    }
    Ok(())
}

/// Render a single RenderOp
fn render_op(op: &RenderOp, width: usize, output: &mut String) -> Result<()> {
    match op {
        RenderOp::Context(dl) => render_line(dl, ' ', false, width, output)?,
        RenderOp::Add(dl) => render_line(dl, '+', false, width, output)?,
        RenderOp::Remove(dl) => render_line(dl, '-', false, width, output)?,
        RenderOp::Replace { old, new } => {
            // Always show line numbers for replacements
            render_line(old, '-', false, width, output)?;
            render_line(new, '+', false, width, output)?;
        }
        RenderOp::Gap => {
            writeln!(output, "...")?;
        }
    }
    Ok(())
}

/// Render outline lines in diff format with dual-number gutters and +/- prefixes
/// Now using coalesced RenderOps to show replacements as "old -> new" on one line
fn render_outline_lines_as_diff(
    output: &mut String,
    lines: &[(usize, OutlineLineType)],
    _matched_lines: &HashSet<usize>,
    source_lines: &[&str],
    diff_lines: Option<&Vec<DiffLine>>,
) -> Result<()> {
    // If we have diff_lines, coalesce them into RenderOps
    let ops = if let Some(dlines) = diff_lines {
        coalesce_replacements(dlines)
    } else {
        Vec::new()
    };

    // Calculate the width for line numbers (based on max line number)
    let width = if let Some(dlines) = diff_lines {
        let max_old = dlines.iter().filter_map(|dl| dl.old_no).max().unwrap_or(0);
        let max_new = dlines.iter().filter_map(|dl| dl.new_no).max().unwrap_or(0);
        max_old.max(max_new).to_string().len()
    } else {
        lines
            .iter()
            .map(|&(line_num, _)| line_num)
            .max()
            .unwrap_or(0)
            .to_string()
            .len()
    };

    // Build a map from new line numbers to RenderOps for quick lookup
    let mut op_map: HashMap<usize, &RenderOp> = HashMap::new();
    for op in &ops {
        match op {
            RenderOp::Add(dl) => {
                if let Some(n) = dl.new_no {
                    op_map.insert(n, op);
                }
            }
            RenderOp::Replace { new, .. } => {
                if let Some(n) = new.new_no {
                    op_map.insert(n, op);
                }
            }
            RenderOp::Context(dl) => {
                if let Some(n) = dl.new_no {
                    op_map.insert(n, op);
                }
            }
            _ => {}
        }
    }

    // Also track removed lines by their old line number (for proper ordering)
    let mut removed_ops: Vec<(usize, &RenderOp)> = Vec::new();
    for op in &ops {
        if let RenderOp::Remove(dl) = op {
            if let Some(old_no) = dl.old_no {
                removed_ops.push((old_no, op));
            }
        }
    }
    removed_ops.sort_by_key(|(old_no, _)| *old_no);

    let mut last_displayed_line = 0;
    let mut removed_index = 0;

    for &(line_num, _line_type) in lines {
        // Before showing this outline line, show any removed lines that come before it
        while removed_index < removed_ops.len() {
            let (removed_old_no, removed_op) = removed_ops[removed_index];
            // Show removed lines that come before the current line
            if removed_old_no <= last_displayed_line || (line_num > 0 && removed_old_no >= line_num)
            {
                removed_index += 1;
                if removed_old_no <= last_displayed_line {
                    continue; // Already shown or too early
                }
                break;
            }
            render_op(removed_op, width, output)?;
            removed_index += 1;
        }

        // Handle gap from last displayed line
        if last_displayed_line > 0 && line_num > last_displayed_line + 1 {
            let gap_size = line_num - last_displayed_line - 1;

            if gap_size >= 5 {
                // Show ellipsis for larger gaps
                render_op(&RenderOp::Gap, width, output)?;
            } else {
                // Show actual lines for small gaps (as context)
                for gap_line in (last_displayed_line + 1)..line_num {
                    if gap_line > 0 && gap_line <= source_lines.len() {
                        // Check if this gap line has a diff operation
                        if let Some(op) = op_map.get(&gap_line) {
                            render_op(op, width, output)?;
                        } else {
                            // No diff for this line, show as context
                            let ctx_line = DiffLine {
                                kind: DiffKind::Context,
                                old_no: Some(gap_line),
                                new_no: Some(gap_line),
                                text: source_lines[gap_line - 1].to_string(),
                            };
                            render_op(&RenderOp::Context(&ctx_line), width, output)?;
                        }
                    }
                }
            }
        }

        // Display this outline line - either as a diff op or as context
        if line_num > 0 && line_num <= source_lines.len() {
            if let Some(op) = op_map.get(&line_num) {
                // This line has a diff operation (Add, Replace, or Context from diff)
                render_op(op, width, output)?;
            } else {
                // No diff for this line, show as context
                let ctx_line = DiffLine {
                    kind: DiffKind::Context,
                    old_no: Some(line_num),
                    new_no: Some(line_num),
                    text: source_lines[line_num - 1].to_string(),
                };
                render_op(&RenderOp::Context(&ctx_line), width, output)?;
            }
            last_displayed_line = line_num;
        }
    }

    Ok(())
}

/// Parse raw unified diff text to extract DiffLine structures with old/new line numbers
///
/// Returns a HashMap mapping file paths to vectors of DiffLines
fn parse_diff(diff_text: &str) -> Result<HashMap<PathBuf, Vec<DiffLine>>> {
    let mut result: HashMap<PathBuf, Vec<DiffLine>> = HashMap::new();
    let lines: Vec<&str> = diff_text.lines().collect();

    let diff_header_regex = Regex::new(r"^diff --git a/(.*) b/(.*)$").unwrap();
    let hunk_header_regex = Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap();

    let mut current_file: Option<PathBuf> = None;
    let mut current_diff_lines: Vec<DiffLine> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Check for diff header
        if let Some(cap) = diff_header_regex.captures(line) {
            // Save previous file's diff lines
            if let Some(file_path) = &current_file {
                result.insert(file_path.clone(), current_diff_lines.clone());
                current_diff_lines.clear();
            }

            // Use the 'b' path (new file) as current file
            let file_path = cap.get(2).unwrap().as_str();
            current_file = Some(PathBuf::from(file_path));
            i += 1;
            continue;
        }

        // Check for hunk header
        if let Some(cap) = hunk_header_regex.captures(line) {
            let old_start: usize = cap.get(1).unwrap().as_str().parse().unwrap_or(1);
            let new_start: usize = cap.get(3).unwrap().as_str().parse().unwrap_or(1);

            let mut old_line = old_start;
            let mut new_line = new_start;

            i += 1;

            // Process lines within this hunk
            while i < lines.len() {
                let hunk_line = lines[i];

                // Stop at next hunk or next diff
                if hunk_line.starts_with("@@") || hunk_line.starts_with("diff --git") {
                    break;
                }

                // Skip file headers (---, +++)
                if hunk_line.starts_with("---") || hunk_line.starts_with("+++") {
                    i += 1;
                    continue;
                }

                // Parse diff line based on prefix
                if hunk_line.starts_with('+') && !hunk_line.starts_with("+++") {
                    // Addition
                    current_diff_lines.push(DiffLine {
                        kind: DiffKind::Add,
                        old_no: None,
                        new_no: Some(new_line),
                        text: hunk_line[1..].to_string(),
                    });
                    new_line += 1;
                } else if hunk_line.starts_with('-') && !hunk_line.starts_with("---") {
                    // Deletion
                    current_diff_lines.push(DiffLine {
                        kind: DiffKind::Remove,
                        old_no: Some(old_line),
                        new_no: None,
                        text: hunk_line[1..].to_string(),
                    });
                    old_line += 1;
                } else if let Some(stripped) = hunk_line.strip_prefix(' ') {
                    // Context line
                    current_diff_lines.push(DiffLine {
                        kind: DiffKind::Context,
                        old_no: Some(old_line),
                        new_no: Some(new_line),
                        text: stripped.to_string(),
                    });
                    old_line += 1;
                    new_line += 1;
                }

                i += 1;
            }
            continue;
        }

        i += 1;
    }

    // Save the last file's diff lines
    if let Some(file_path) = &current_file {
        result.insert(file_path.clone(), current_diff_lines);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outline_diff_with_no_diff() {
        // Test outline-diff without raw diff input (should work with empty results)
        let results = vec![];
        let output = format_outline_diff(&results, None);

        // Should handle empty results gracefully
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "No results found.\n");
    }
}

// Note: The following unused helper functions have been removed since we now use
// the outline logic from probe_code::search::search_output instead.
// Previously had: extract_semantic_context, find_context_for_line, find_node_at_line, is_acceptable_context
