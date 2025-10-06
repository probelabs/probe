use anyhow::{Context, Result};
use colored::*;
use ignore::WalkBuilder;
use regex::RegexBuilder;
use std::collections::HashMap;
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

pub fn handle_grep(params: GrepParams) -> Result<()> {
    // Build the regex pattern
    let regex = RegexBuilder::new(&params.pattern)
        .case_insensitive(params.ignore_case)
        .build()
        .context("Failed to compile regex pattern")?;

    // Determine context lines
    let before_ctx = params.context.or(params.before_context).unwrap_or(0);
    let after_ctx = params.context.or(params.after_context).unwrap_or(0);

    // Determine if we should use colors
    let use_color = match params.color.as_str() {
        "always" => true,
        "never" => false,
        _ => atty::is(atty::Stream::Stdout), // auto: check if stdout is a tty
    };

    // Build the file walker
    let mut _file_match_counts: HashMap<PathBuf, usize> = HashMap::new();

    for path in &params.paths {
        let mut walker_builder = WalkBuilder::new(path);
        walker_builder
            .hidden(false)
            .git_ignore(!params.no_gitignore)
            .git_global(!params.no_gitignore)
            .git_exclude(!params.no_gitignore);

        // Add custom ignore patterns
        for pattern in &params.ignore {
            walker_builder.add_custom_ignore_filename(pattern);
        }

        let walker = walker_builder.build();

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

            // Try to read the file
            let file = match fs::File::open(file_path) {
                Ok(f) => f,
                Err(_) => continue, // Skip files we can't read
            };

            let reader = io::BufReader::new(file);
            let mut line_num = 0;
            let mut match_count = 0;
            let mut matched_lines: Vec<(usize, String, bool)> = Vec::new();
            let mut has_match = false;

            // Read file line by line
            for line_result in reader.lines() {
                line_num += 1;
                let line = match line_result {
                    Ok(l) => l,
                    Err(_) => continue,
                };

                let is_match = regex.is_match(&line) != params.invert_match;

                if is_match {
                    has_match = true;
                    match_count += 1;

                    // Check max count
                    if let Some(max) = params.max_count {
                        if match_count > max {
                            break;
                        }
                    }
                }

                // Store line for context processing
                matched_lines.push((line_num, line, is_match));
            }

            if !has_match && !params.files_without_match {
                continue;
            }

            if has_match && params.files_without_match {
                continue;
            }

            // Output based on mode
            if params.files_without_match || params.files_with_matches {
                println!("{}", file_path.display());
            } else if params.count {
                if params.line_number {
                    println!("{}:{}", file_path.display(), match_count);
                } else {
                    println!("{}", match_count);
                }
            } else {
                // Full output with context
                let mut i = 0;
                while i < matched_lines.len() {
                    let (line_num, line, is_match) = &matched_lines[i];

                    if *is_match {
                        // Print before context
                        let context_start = i.saturating_sub(before_ctx);
                        for (ctx_line_num, ctx_line, _) in
                            matched_lines.iter().take(i).skip(context_start)
                        {
                            print_grep_line(
                                file_path,
                                *ctx_line_num,
                                ctx_line,
                                false,
                                params.line_number,
                                use_color,
                                &regex,
                            );
                        }

                        // Print matching line
                        print_grep_line(
                            file_path,
                            *line_num,
                            line,
                            true,
                            params.line_number,
                            use_color,
                            &regex,
                        );

                        // Print after context
                        let context_end = (i + after_ctx + 1).min(matched_lines.len());
                        for (ctx_line_num, ctx_line, ctx_is_match) in
                            matched_lines.iter().take(context_end).skip(i + 1)
                        {
                            if !ctx_is_match {
                                print_grep_line(
                                    file_path,
                                    *ctx_line_num,
                                    ctx_line,
                                    false,
                                    params.line_number,
                                    use_color,
                                    &regex,
                                );
                            }
                        }

                        // Skip ahead past the context we just printed
                        i = (i + after_ctx + 1).min(matched_lines.len());
                    } else {
                        i += 1;
                    }
                }
            }

            _file_match_counts.insert(file_path.to_path_buf(), match_count);
        }
    }

    Ok(())
}

fn print_grep_line(
    file_path: &Path,
    line_num: usize,
    line: &str,
    is_match: bool,
    show_line_number: bool,
    use_color: bool,
    regex: &regex::Regex,
) {
    let file_str = file_path.display().to_string();

    if use_color {
        if is_match {
            // Highlight matches in the line
            let mut last_end = 0;
            let mut highlighted = String::new();

            for mat in regex.find_iter(line) {
                highlighted.push_str(&line[last_end..mat.start()]);
                highlighted.push_str(&mat.as_str().red().bold().to_string());
                last_end = mat.end();
            }
            highlighted.push_str(&line[last_end..]);

            if show_line_number {
                println!(
                    "{}:{}:{}",
                    file_str.green(),
                    line_num.to_string().green(),
                    highlighted
                );
            } else {
                println!("{}:{}", file_str.green(), highlighted);
            }
        } else {
            // Context line
            if show_line_number {
                println!(
                    "{}-{}-{}",
                    file_str.green(),
                    line_num.to_string().cyan(),
                    line
                );
            } else {
                println!("{}-{}", file_str.green(), line);
            }
        }
    } else if show_line_number {
        if is_match {
            println!("{}:{}:{}", file_str, line_num, line);
        } else {
            println!("{}-{}-{}", file_str, line_num, line);
        }
    } else if is_match {
        println!("{}:{}", file_str, line);
    } else {
        println!("{}-{}", file_str, line);
    }
}
