use anyhow::{Context, Result};
use colored::*;
use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};
use std::collections::VecDeque;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct GrepParams {
    pub pattern: String,
    pub paths: Vec<PathBuf>,
    pub ignore_case: bool,
    pub line_number: bool,
    pub count: bool,
    pub files_with_matches: bool,
    pub files_without_match: bool,
    pub invert_match: bool,
    pub before_context: Option<usize>,
    pub after_context: Option<usize>,
    pub context: Option<usize>,
    pub ignore: Vec<String>,
    pub no_gitignore: bool,
    pub color: String,
    pub max_count: Option<usize>,
}

/// Configuration for grep operations
struct GrepConfig {
    regex: Regex,
    before_context: usize,
    after_context: usize,
    use_color: bool,
    show_line_numbers: bool,
    invert_match: bool,
    max_count: Option<usize>,
}

impl GrepConfig {
    fn from_params(params: &GrepParams) -> Result<Self> {
        let regex = RegexBuilder::new(&params.pattern)
            .case_insensitive(params.ignore_case)
            .build()
            .context("Failed to compile regex pattern")?;

        let before_context = params.context.or(params.before_context).unwrap_or(0);
        let after_context = params.context.or(params.after_context).unwrap_or(0);

        let use_color = match params.color.as_str() {
            "always" => true,
            "never" => false,
            _ => atty::is(atty::Stream::Stdout),
        };

        Ok(Self {
            regex,
            before_context,
            after_context,
            use_color,
            show_line_numbers: params.line_number,
            invert_match: params.invert_match,
            max_count: params.max_count,
        })
    }
}

/// Represents a single line in a file
#[derive(Debug, Clone)]
struct MatchedLine {
    line_number: usize,
    content: String,
}

/// Result of processing a single file (for simple modes)
struct FileMatchResult {
    has_match: bool,
    match_count: usize,
}

/// Processes a single file with streaming approach
struct FileProcessor<'a> {
    config: &'a GrepConfig,
}

impl<'a> FileProcessor<'a> {
    fn new(config: &'a GrepConfig) -> Self {
        Self { config }
    }

    /// Process file and return basic match info (for count/files-only modes)
    fn count_matches(&self, file_path: &Path) -> Result<FileMatchResult> {
        let file = fs::File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let reader = io::BufReader::new(file);
        let mut match_count = 0;
        let mut has_match = false;

        for line_result in reader.lines() {
            let content = match line_result {
                Ok(l) => l,
                Err(_) => continue,
            };

            let is_match = self.config.regex.is_match(&content) != self.config.invert_match;

            if is_match {
                has_match = true;
                match_count += 1;

                // Check max count
                if let Some(max) = self.config.max_count {
                    if match_count >= max {
                        break;
                    }
                }
            }
        }

        Ok(FileMatchResult {
            has_match,
            match_count,
        })
    }

    /// Process file with streaming output (for full context mode)
    fn process_with_output<F>(&self, file_path: &Path, mut output_fn: F) -> Result<FileMatchResult>
    where
        F: FnMut(&MatchedLine, bool),
    {
        let file = fs::File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let reader = io::BufReader::new(file);
        let mut match_count = 0;
        let mut has_match = false;

        // Ring buffer for before-context lines
        let mut before_buffer: VecDeque<MatchedLine> =
            VecDeque::with_capacity(self.config.before_context);

        // Track lines we need to print after a match
        let mut after_remaining = 0;

        for (line_index, line_result) in reader.lines().enumerate() {
            let line_number = line_index + 1;
            let content = match line_result {
                Ok(l) => l,
                Err(_) => continue,
            };

            let is_match = self.config.regex.is_match(&content) != self.config.invert_match;

            let current_line = MatchedLine {
                line_number,
                content,
            };

            if is_match {
                has_match = true;
                match_count += 1;

                // Check max count
                if let Some(max) = self.config.max_count {
                    if match_count > max {
                        break;
                    }
                }

                // Print before-context lines from buffer
                for ctx_line in &before_buffer {
                    output_fn(ctx_line, false);
                }
                before_buffer.clear();

                // Print the matching line
                output_fn(&current_line, true);

                // Set up after-context printing
                after_remaining = self.config.after_context;
            } else if after_remaining > 0 {
                // Print this line as after-context
                output_fn(&current_line, false);
                after_remaining -= 1;
            } else {
                // Add to before-context buffer
                if self.config.before_context > 0 {
                    before_buffer.push_back(current_line);
                    // Keep buffer size limited
                    if before_buffer.len() > self.config.before_context {
                        before_buffer.pop_front();
                    }
                }
            }
        }

        Ok(FileMatchResult {
            has_match,
            match_count,
        })
    }
}

/// Output mode for grep results
#[derive(Debug, Clone, Copy)]
enum OutputMode {
    FilesWithMatches,
    FilesWithoutMatch,
    Count,
    FullWithContext,
}

impl OutputMode {
    fn from_params(params: &GrepParams) -> Self {
        if params.files_without_match {
            Self::FilesWithoutMatch
        } else if params.files_with_matches {
            Self::FilesWithMatches
        } else if params.count {
            Self::Count
        } else {
            Self::FullWithContext
        }
    }
}

/// Main entry point for grep functionality
pub fn handle_grep(params: GrepParams) -> Result<()> {
    let config = GrepConfig::from_params(&params)?;
    let output_mode = OutputMode::from_params(&params);

    // Use Arc to share config across threads
    let config = std::sync::Arc::new(config);
    let params = std::sync::Arc::new(params);

    // Mutex for synchronized output to prevent interleaved results
    let stdout = Mutex::new(io::stdout());

    for path in params.paths.iter() {
        let walker = build_walker_parallel(path, &params.ignore, params.no_gitignore);

        let config = config.clone();
        let params = params.clone();
        let stdout_ref = &stdout;

        walker.run(|| {
            let config = config.clone();
            let params = params.clone();

            Box::new(move |entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return ignore::WalkState::Continue,
                };

                // Skip directories
                if entry.file_type().is_none_or(|ft| ft.is_dir()) {
                    return ignore::WalkState::Continue;
                }

                let file_path = entry.path();
                let file_processor = FileProcessor::new(&config);

                match output_mode {
                    OutputMode::FullWithContext => {
                        // For streaming mode, collect output in a buffer first
                        let mut buffer = Vec::new();

                        let result = file_processor.process_with_output(
                            file_path,
                            |line, is_match| {
                                // Format line into buffer
                                let formatted = format_line(&config, file_path, line, is_match);
                                buffer.push(formatted);
                            },
                        );

                        let result = match result {
                            Ok(r) => r,
                            Err(_) => return ignore::WalkState::Continue,
                        };

                        // Skip files based on match status
                        if should_skip_file(&result, &params) {
                            return ignore::WalkState::Continue;
                        }

                        // Write entire buffer atomically
                        if !buffer.is_empty() {
                            if let Ok(mut out) = stdout_ref.lock() {
                                for line in buffer {
                                    let _ = writeln!(out, "{}", line);
                                }
                            }
                        }
                    }
                    _ => {
                        // Simple modes: count matches only
                        let result = match file_processor.count_matches(file_path) {
                            Ok(r) => r,
                            Err(_) => return ignore::WalkState::Continue,
                        };

                        // Apply filtering based on output mode
                        if should_skip_file(&result, &params) {
                            return ignore::WalkState::Continue;
                        }

                        // Format and write output atomically
                        if let Ok(mut out) = stdout_ref.lock() {
                            match output_mode {
                                OutputMode::FilesWithMatches | OutputMode::FilesWithoutMatch => {
                                    let _ = writeln!(out, "{}", file_path.display());
                                }
                                OutputMode::Count => {
                                    if config.show_line_numbers {
                                        let _ = writeln!(out, "{}:{}", file_path.display(), result.match_count);
                                    } else {
                                        let _ = writeln!(out, "{}", result.match_count);
                                    }
                                }
                                OutputMode::FullWithContext => unreachable!(),
                            }
                        }
                    }
                }

                ignore::WalkState::Continue
            })
        });
    }

    Ok(())
}

/// Format a single line for output
fn format_line(config: &GrepConfig, file_path: &Path, line: &MatchedLine, is_match: bool) -> String {
    let file_str = file_path.display().to_string();

    if config.use_color {
        format_colored_line(config, &file_str, line, is_match)
    } else {
        format_plain_line(config, &file_str, line, is_match)
    }
}

/// Format a colored line
fn format_colored_line(config: &GrepConfig, file_str: &str, line: &MatchedLine, is_match: bool) -> String {
    if is_match {
        let highlighted = highlight_matches(config, &line.content);

        if config.show_line_numbers {
            format!(
                "{}:{}:{}",
                file_str.green(),
                line.line_number.to_string().green(),
                highlighted
            )
        } else {
            format!("{}:{}", file_str.green(), highlighted)
        }
    } else {
        // Context line
        if config.show_line_numbers {
            format!(
                "{}-{}-{}",
                file_str.green(),
                line.line_number.to_string().cyan(),
                line.content
            )
        } else {
            format!("{}-{}", file_str.green(), line.content)
        }
    }
}

/// Format a plain (non-colored) line
fn format_plain_line(config: &GrepConfig, file_str: &str, line: &MatchedLine, is_match: bool) -> String {
    if config.show_line_numbers {
        let separator = if is_match { ":" } else { "-" };
        format!(
            "{}{}{}{}{}",
            file_str, separator, line.line_number, separator, line.content
        )
    } else {
        let separator = if is_match { ":" } else { "-" };
        format!("{}{}{}", file_str, separator, line.content)
    }
}

/// Highlight regex matches in a line
fn highlight_matches(config: &GrepConfig, line: &str) -> String {
    let mut last_end = 0;
    let mut highlighted = String::new();

    for mat in config.regex.find_iter(line) {
        highlighted.push_str(&line[last_end..mat.start()]);
        highlighted.push_str(&mat.as_str().red().bold().to_string());
        last_end = mat.end();
    }
    highlighted.push_str(&line[last_end..]);
    highlighted
}

/// Build a parallel file walker with the given parameters
fn build_walker_parallel(
    path: &Path,
    ignore_patterns: &[String],
    no_gitignore: bool,
) -> ignore::WalkParallel {
    let mut walker_builder = WalkBuilder::new(path);
    walker_builder
        .hidden(false)
        .git_ignore(!no_gitignore)
        .git_global(!no_gitignore)
        .git_exclude(!no_gitignore)
        .threads(num_cpus::get()); // Use all available CPU cores

    for pattern in ignore_patterns {
        walker_builder.add_custom_ignore_filename(pattern);
    }

    walker_builder.build_parallel()
}

/// Determine if a file should be skipped based on match status and params
fn should_skip_file(result: &FileMatchResult, params: &GrepParams) -> bool {
    if !result.has_match && !params.files_without_match {
        return true;
    }

    if result.has_match && params.files_without_match {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matched_line_creation() {
        let line = MatchedLine {
            line_number: 42,
            content: "test content".to_string(),
        };

        assert_eq!(line.line_number, 42);
        assert_eq!(line.content, "test content");
    }

    #[test]
    fn test_grep_config_from_params() {
        let params = GrepParams {
            pattern: "test".to_string(),
            paths: vec![PathBuf::from(".")],
            ignore_case: true,
            line_number: true,
            count: false,
            files_with_matches: false,
            files_without_match: false,
            invert_match: false,
            before_context: Some(2),
            after_context: Some(3),
            context: None,
            ignore: vec![],
            no_gitignore: false,
            color: "never".to_string(),
            max_count: Some(10),
        };

        let config = GrepConfig::from_params(&params).unwrap();
        assert_eq!(config.before_context, 2);
        assert_eq!(config.after_context, 3);
        assert!(!config.use_color);
        assert!(config.show_line_numbers);
        assert_eq!(config.max_count, Some(10));
    }

    #[test]
    fn test_output_mode_from_params() {
        let mut params = GrepParams {
            pattern: "test".to_string(),
            paths: vec![PathBuf::from(".")],
            ignore_case: false,
            line_number: true,
            count: false,
            files_with_matches: true,
            files_without_match: false,
            invert_match: false,
            before_context: None,
            after_context: None,
            context: None,
            ignore: vec![],
            no_gitignore: false,
            color: "auto".to_string(),
            max_count: None,
        };

        matches!(
            OutputMode::from_params(&params),
            OutputMode::FilesWithMatches
        );

        params.files_with_matches = false;
        params.count = true;
        matches!(OutputMode::from_params(&params), OutputMode::Count);
    }

    #[test]
    fn test_should_skip_file_logic() {
        let params = GrepParams {
            pattern: "test".to_string(),
            paths: vec![PathBuf::from(".")],
            ignore_case: false,
            line_number: true,
            count: false,
            files_with_matches: false,
            files_without_match: false,
            invert_match: false,
            before_context: None,
            after_context: None,
            context: None,
            ignore: vec![],
            no_gitignore: false,
            color: "auto".to_string(),
            max_count: None,
        };

        let result = FileMatchResult {
            has_match: false,
            match_count: 0,
        };

        // Should skip files without matches when not looking for files without matches
        assert!(should_skip_file(&result, &params));

        // Should not skip files with matches
        let result_with_match = FileMatchResult {
            has_match: true,
            match_count: 1,
        };
        assert!(!should_skip_file(&result_with_match, &params));
    }

    #[test]
    fn test_streaming_context_handling() {
        // Test that the ring buffer correctly handles before-context
        let config = GrepConfig {
            regex: regex::Regex::new("match").unwrap(),
            before_context: 2,
            after_context: 1,
            use_color: false,
            show_line_numbers: true,
            invert_match: false,
            max_count: None,
        };

        let processor = FileProcessor::new(&config);

        // Create a temp file with test content
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        writeln!(file, "match here").unwrap(); // Should trigger output
        writeln!(file, "line 5").unwrap();
        writeln!(file, "line 6").unwrap();

        let mut output_lines = Vec::new();
        let result = processor
            .process_with_output(&file_path, |line, _is_match| {
                output_lines.push(line.line_number);
            })
            .unwrap();

        // Should have found 1 match
        assert_eq!(result.match_count, 1);
        assert!(result.has_match);

        // Should output: line 2 (before), line 3 (before), line 4 (match), line 5 (after)
        assert_eq!(output_lines, vec![2, 3, 4, 5]);
    }
}
