use anyhow::{Context, Result};
use colored::*;
use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

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

/// Represents a single line in a file with match status
#[derive(Debug, Clone)]
struct MatchedLine {
    line_number: usize,
    content: String,
    is_match: bool,
}

/// Result of processing a single file
struct FileMatchResult {
    has_match: bool,
    match_count: usize,
    matched_lines: Vec<MatchedLine>,
}

/// Processes a single file and returns match results
struct FileProcessor<'a> {
    config: &'a GrepConfig,
}

impl<'a> FileProcessor<'a> {
    fn new(config: &'a GrepConfig) -> Self {
        Self { config }
    }

    fn process_file(&self, file_path: &Path) -> Result<FileMatchResult> {
        let file = fs::File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let reader = io::BufReader::new(file);
        let mut match_count = 0;
        let mut matched_lines = Vec::new();
        let mut has_match = false;

        for (line_index, line_result) in reader.lines().enumerate() {
            let line_number = line_index + 1; // Convert 0-based to 1-based
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
                    if match_count > max {
                        break;
                    }
                }
            }

            matched_lines.push(MatchedLine {
                line_number,
                content,
                is_match,
            });
        }

        Ok(FileMatchResult {
            has_match,
            match_count,
            matched_lines,
        })
    }
}

/// Output mode for grep results
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

/// Handles output formatting for grep results
struct OutputFormatter<'a> {
    config: &'a GrepConfig,
    mode: OutputMode,
}

impl<'a> OutputFormatter<'a> {
    fn new(config: &'a GrepConfig, mode: OutputMode) -> Self {
        Self { config, mode }
    }

    fn output_result(&self, file_path: &Path, result: &FileMatchResult) {
        match self.mode {
            OutputMode::FilesWithMatches | OutputMode::FilesWithoutMatch => {
                println!("{}", file_path.display());
            }
            OutputMode::Count => {
                if self.config.show_line_numbers {
                    println!("{}:{}", file_path.display(), result.match_count);
                } else {
                    println!("{}", result.match_count);
                }
            }
            OutputMode::FullWithContext => {
                self.output_with_context(file_path, &result.matched_lines);
            }
        }
    }

    fn output_with_context(&self, file_path: &Path, matched_lines: &[MatchedLine]) {
        let mut i = 0;
        while i < matched_lines.len() {
            let line = &matched_lines[i];

            if line.is_match {
                // Print before context
                let context_start = i.saturating_sub(self.config.before_context);
                for ctx_line in matched_lines.iter().take(i).skip(context_start) {
                    self.print_line(file_path, ctx_line, false);
                }

                // Print matching line
                self.print_line(file_path, line, true);

                // Print after context
                let context_end = (i + self.config.after_context + 1).min(matched_lines.len());
                for ctx_line in matched_lines.iter().take(context_end).skip(i + 1) {
                    if !ctx_line.is_match {
                        self.print_line(file_path, ctx_line, false);
                    }
                }

                // Skip ahead past the context we just printed
                i = (i + self.config.after_context + 1).min(matched_lines.len());
            } else {
                i += 1;
            }
        }
    }

    fn print_line(&self, file_path: &Path, line: &MatchedLine, is_match: bool) {
        let file_str = file_path.display().to_string();

        if self.config.use_color {
            self.print_colored_line(&file_str, line, is_match);
        } else {
            self.print_plain_line(&file_str, line, is_match);
        }
    }

    fn print_colored_line(&self, file_str: &str, line: &MatchedLine, is_match: bool) {
        if is_match {
            let highlighted = self.highlight_matches(&line.content);

            if self.config.show_line_numbers {
                println!(
                    "{}:{}:{}",
                    file_str.green(),
                    line.line_number.to_string().green(),
                    highlighted
                );
            } else {
                println!("{}:{}", file_str.green(), highlighted);
            }
        } else {
            // Context line
            if self.config.show_line_numbers {
                println!(
                    "{}-{}-{}",
                    file_str.green(),
                    line.line_number.to_string().cyan(),
                    line.content
                );
            } else {
                println!("{}-{}", file_str.green(), line.content);
            }
        }
    }

    fn print_plain_line(&self, file_str: &str, line: &MatchedLine, is_match: bool) {
        if self.config.show_line_numbers {
            let separator = if is_match { ":" } else { "-" };
            println!(
                "{}{}{}{}{}",
                file_str, separator, line.line_number, separator, line.content
            );
        } else {
            let separator = if is_match { ":" } else { "-" };
            println!("{}{}{}", file_str, separator, line.content);
        }
    }

    fn highlight_matches(&self, line: &str) -> String {
        let mut last_end = 0;
        let mut highlighted = String::new();

        for mat in self.config.regex.find_iter(line) {
            highlighted.push_str(&line[last_end..mat.start()]);
            highlighted.push_str(&mat.as_str().red().bold().to_string());
            last_end = mat.end();
        }
        highlighted.push_str(&line[last_end..]);
        highlighted
    }
}

/// Main entry point for grep functionality
pub fn handle_grep(params: GrepParams) -> Result<()> {
    let config = GrepConfig::from_params(&params)?;
    let output_mode = OutputMode::from_params(&params);
    let file_processor = FileProcessor::new(&config);
    let output_formatter = OutputFormatter::new(&config, output_mode);

    for path in &params.paths {
        let walker = build_walker(path, &params.ignore, params.no_gitignore);

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Skip directories
            if entry.file_type().is_none_or(|ft| ft.is_dir()) {
                continue;
            }

            let file_path = entry.path();

            // Process the file
            let result = match file_processor.process_file(file_path) {
                Ok(r) => r,
                Err(_) => continue, // Skip files we can't read
            };

            // Apply filtering based on output mode
            if should_skip_file(&result, &params) {
                continue;
            }

            // Output the results
            output_formatter.output_result(file_path, &result);
        }
    }

    Ok(())
}

/// Build a file walker with the given parameters
fn build_walker(path: &Path, ignore_patterns: &[String], no_gitignore: bool) -> ignore::Walk {
    let mut walker_builder = WalkBuilder::new(path);
    walker_builder
        .hidden(false)
        .git_ignore(!no_gitignore)
        .git_global(!no_gitignore)
        .git_exclude(!no_gitignore);

    for pattern in ignore_patterns {
        walker_builder.add_custom_ignore_filename(pattern);
    }

    walker_builder.build()
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
            is_match: true,
        };

        assert_eq!(line.line_number, 42);
        assert_eq!(line.content, "test content");
        assert!(line.is_match);
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
            matched_lines: vec![],
        };

        // Should skip files without matches when not looking for files without matches
        assert!(should_skip_file(&result, &params));

        // Should not skip files with matches
        let result_with_match = FileMatchResult {
            has_match: true,
            match_count: 1,
            matched_lines: vec![],
        };
        assert!(!should_skip_file(&result_with_match, &params));
    }
}
