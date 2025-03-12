use anyhow::{Context, Result};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
// No need for term_exceptions import

/// Searches a file for a pattern and returns whether it matched and the matching line numbers
pub fn search_file_for_pattern(
    file_path: &Path,
    pattern: &str,
    exact: bool,
) -> Result<(bool, HashSet<usize>)> {
    let mut matched = false;
    let mut line_numbers = HashSet::new();
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Check if the pattern already has word boundaries or parentheses (indicating a grouped pattern)
    let has_word_boundaries = pattern.contains("\\b") || pattern.starts_with("(");

    // Use the pattern as-is if exact mode is specified, it already has word boundaries,
    // or it's a grouped pattern (starts with parenthesis)
    let adjusted_pattern = if exact || has_word_boundaries {
        pattern.to_string()
    } else {
        format!(r"\b{}\b", pattern)
    };

    // Create a case-insensitive regex matcher for the pattern
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(&adjusted_pattern)
        .context(format!(
            "Failed to create regex matcher for: {}",
            adjusted_pattern
        ))?;

    // Configure the searcher
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    // Search the file
    if let Err(err) = searcher.search_path(
        &matcher,
        file_path,
        UTF8(|line_number, line| {
            // Check if line is longer than 2000 characters
            if line.len() > 2000 {
                if debug_mode {
                    println!(
                        "  Skipping line {} in file {} - line too long ({} characters)",
                        line_number,
                        file_name,
                        line.len()
                    );
                }
                return Ok(true); // Skip this line but continue searching
            }

            if debug_mode {
                // Log every match
                println!(
                    "  Match found in file: {} (term: {}) at line {}",
                    file_name, adjusted_pattern, line_number
                );
            }
            matched = true; // Still need this for the return value
            let line_num = line_number as usize;
            line_numbers.insert(line_num);

            // Log raw search results if debug mode is enabled
            // if debug_mode {
            //     println!(
            //         "DEBUG: Match in file '{}' at line {}: '{}'",
            //         file_name,
            //         line_num,
            //         line.trim()
            //     );
            // }

            Ok(true) // Continue searching for all matches
        }),
    ) {
        // Just convert the error to anyhow::Error
        return Err(err.into());
    }

    Ok((matched, line_numbers))
}

/// Finds files containing a specific pattern
pub fn find_files_with_pattern(
    path: &Path,
    pattern: &str,
    custom_ignores: &[String],
    allow_tests: bool,
) -> Result<Vec<PathBuf>> {
    let mut matching_files = Vec::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("Running rgrep search with pattern: {}", pattern);
        println!("DEBUG: Starting rgrep search in path: {:?}", path);
        println!("DEBUG: Using pattern: {}", pattern);
        println!("DEBUG: Custom ignores: {:?}", custom_ignores);
    }

    // Create a case-insensitive regex matcher for the pattern
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)
        .context(format!("Failed to create regex matcher for: {}", pattern))?;

    // Configure the searcher
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    // Create a WalkBuilder that respects .gitignore files and common ignore patterns
    let mut builder = WalkBuilder::new(path);

    // Configure the builder
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);

    // Add common directories to ignore
    let mut common_ignores: Vec<String> = vec![
        "node_modules",
        "vendor",
        "target",
        "dist",
        "build",
        ".git",
        ".svn",
        ".hg",
        ".idea",
        ".vscode",
        "__pycache__",
        "*.pyc",
        "*.pyo",
        "*.class",
        "*.o",
        "*.obj",
        "*.a",
        "*.lib",
        "*.so",
        "*.dylib",
        "*.dll",
        "*.exe",
        "*.out",
        "*.app",
        "*.jar",
        "*.war",
        "*.ear",
        "*.zip",
        "*.tar.gz",
        "*.rar",
        "*.log",
        "*.tmp",
        "*.temp",
        "*.swp",
        "*.swo",
        "*.bak",
        "*.orig",
        "*.DS_Store",
        "Thumbs.db",
        "*.yml",
        "*.yaml",
        "*.json",
        "*.tconf",
        "*.conf",
        "go.sum",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    // Add test file patterns if allow_tests is false
    if !allow_tests {
        let test_patterns: Vec<String> = vec![
            "*_test.rs",
            "*_tests.rs",
            "test_*.rs",
            "tests.rs",
            "*.spec.js",
            "*.test.js",
            "*.spec.ts",
            "*.test.ts",
            "*.spec.jsx",
            "*.test.jsx",
            "*.spec.tsx",
            "*.test.tsx",
            "test_*.py",
            "*_test.go",
            "test_*.c",
            "*_test.c",
            "*_test.cpp",
            "*_test.cc",
            "*_test.cxx",
            "*Test.java",
            "*_test.rb",
            "test_*.rb",
            "*_spec.rb",
            "*Test.php",
            "test_*.php",
            "**/tests/**",
            "**/test/**",
            "**/__tests__/**",
            "**/__test__/**",
            "**/spec/**",
            "**/specs/**",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        common_ignores.extend(test_patterns);
    }

    // Add custom ignore patterns to the common ignores
    for pattern in custom_ignores {
        common_ignores.push(pattern.clone());
    }

    // Create a single override builder for all ignore patterns
    let mut override_builder = ignore::overrides::OverrideBuilder::new(path);

    // Add all ignore patterns to the override builder
    for pattern in &common_ignores {
        if let Err(err) = override_builder.add(&format!("!**/{}", pattern)) {
            eprintln!("Error adding ignore pattern {:?}: {}", pattern, err);
        }
    }

    // Build and apply the overrides
    match override_builder.build() {
        Ok(overrides) => {
            builder.overrides(overrides);
        }
        Err(err) => {
            eprintln!("Error building ignore overrides: {}", err);
        }
    }

    // Count how many files we're searching
    let mut total_files = 0;

    // Recursively walk the directory and search each file
    for result in builder.build() {
        total_files += 1;
        let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("Error walking directory: {}", err);
                continue;
            }
        };

        // Skip directories
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let file_path = entry.path();

        // Search the file
        let path_clone = file_path.to_owned();
        let mut found_match = false;
        let mut first_match_line = 0;

        if let Err(err) = searcher.search_path(
            &matcher,
            file_path,
            UTF8(|line_number, line| {
                // Check if line is longer than 2000 characters
                if line.len() > 2000 {
                    if debug_mode {
                        println!(
                            "DEBUG: Skipping line {} in file {:?} - line too long ({} characters)",
                            line_number,
                            file_path,
                            line.len()
                        );
                    }
                    return Ok(true); // Skip this line but continue searching
                }

                // We only need to know if there's at least one match
                found_match = true;
                first_match_line = line_number;
                Ok(false) // Stop after first match
            }),
        ) {
            // If we found a match, the search was interrupted
            if found_match {
                matching_files.push(path_clone.clone());

                if debug_mode {
                    println!(
                        "DEBUG: Found match in file: {:?} (term: {}) at line {}",
                        path_clone, pattern, first_match_line
                    );
                }
            } else {
                eprintln!("Error searching file {:?}: {}", file_path, err);
            }
            continue;
        }

        // If we found a match (and the search wasn't interrupted)
        if found_match {
            matching_files.push(path_clone.clone());

            if debug_mode {
                println!(
                    "DEBUG: Found match in file: {:?} (term: {}) at line {}",
                    path_clone, pattern, first_match_line
                );
            }
        }
    }

    if debug_mode {
        println!(
            "Searched {} files, found {} matches",
            total_files,
            matching_files.len()
        );
    }

    if debug_mode && !matching_files.is_empty() {
        println!("DEBUG: Raw search results - matching files:");
        for (i, file) in matching_files.iter().enumerate() {
            println!("DEBUG:   {}. {:?}", i + 1, file);
        }
    }
    Ok(matching_files)
}

/// Function to find files whose names match query words
/// This is now a simplified version that returns an empty vector since we're
/// adding the filename to the top of each code block instead of using pattern matching
pub fn find_matching_filenames(
    _path: &Path,
    queries: &[String],
    _already_found_files: &HashSet<PathBuf>,
    _custom_ignores: &[String],
    _allow_tests: bool,
) -> Result<Vec<PathBuf>> {
    // Debug output
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Filename matching is now handled by adding the filename to the top of each code block");
        println!("DEBUG: Queries: {:?}", queries);
    }

    println!(
        "Filename matching is now handled by adding the filename to the top of each code block"
    );

    // Return an empty vector since we're not using pattern matching anymore
    Ok(Vec::new())
}

/// Compatibility function for the old get_filename_matched_queries signature
/// This is now a simplified version that returns a set with all query indices
/// since we're adding the filename to the top of each code block
pub fn get_filename_matched_queries_compat(
    filename: &str,
    queries_terms: &[Vec<(String, String)>],
) -> HashSet<usize> {
    let mut matched_terms = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!(
            "DEBUG: Filename '{}' will be added to the top of code blocks",
            filename
        );
    }

    // Add all query indices to the matched_terms set
    // This ensures that all terms are considered "matched" by the filename
    for (query_idx, term_pairs) in queries_terms.iter().enumerate() {
        matched_terms.insert(query_idx);

        if debug_mode {
            for (term_idx, (original_term, _)) in term_pairs.iter().enumerate() {
                println!(
                    "DEBUG:   Term '{}' (index {}) considered matched by filename '{}'",
                    original_term, term_idx, filename
                );
            }
        }
    }

    if debug_mode {
        println!(
            "DEBUG:   Added {} term indices for filename '{}'",
            matched_terms.len(),
            filename
        );
    }

    matched_terms
}
