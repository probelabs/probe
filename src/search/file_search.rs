use anyhow::{Context, Result};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::search::query::{create_term_patterns, preprocess_query};

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
        UTF8(|line_number, _line| {
            if !matched && debug_mode {
                // Only log the first match
                println!("  Match found in file: {}", file_name);
            }
            matched = true;
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
        "go.sum",
    ].into_iter().map(String::from).collect();

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
        ].into_iter().map(String::from).collect();
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
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let file_path = entry.path();

        // Search the file
        let path_clone = file_path.to_owned();
        let mut found_match = false;

        if let Err(err) = searcher.search_path(
            &matcher,
            file_path,
            UTF8(|_, _| {
                // We only need to know if there's at least one match
                found_match = true;
                Ok(false) // Stop after first match
            }),
        ) {
            // If we found a match, the search was interrupted
            if found_match {
                matching_files.push(path_clone);
            } else {
                eprintln!("Error searching file {:?}: {}", file_path, err);
            }
            continue;
        }

        // If we found a match (and the search wasn't interrupted)
        if found_match {
            matching_files.push(path_clone.clone());

            if debug_mode {
                println!("DEBUG: Found match in file: {:?}", path_clone);
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
pub fn find_matching_filenames(
    path: &Path,
    queries: &[String],
    already_found_files: &HashSet<PathBuf>,
    custom_ignores: &[String],
    allow_tests: bool,
) -> Result<Vec<PathBuf>> {
    let mut matching_files = Vec::new();

    // Process queries to get both original and stemmed terms
    let queries_terms: Vec<Vec<(String, String)>> = queries
        .iter()
        .map(|q| preprocess_query(q, false)) // Use non-exact mode for filename matching
        .collect();

    // Generate all patterns using the new create_term_patterns function
    let all_patterns: Vec<String> = queries_terms
        .iter()
        .flat_map(|term_pairs| {
            // Extract just the pattern strings from the tuples
            create_term_patterns(term_pairs)
                .into_iter()
                .map(|(pattern, _)| pattern)
                .collect::<Vec<String>>()
        })
        .collect();

    println!(
        "Looking for filenames matching queries (with flexible patterns): {:?}",
        queries
    );

    // Debug output for patterns
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    if debug_mode && !all_patterns.is_empty() {
        println!("DEBUG: Using the following patterns for filename matching:");
        for (i, pattern) in all_patterns.iter().enumerate() {
            println!("DEBUG:   {}. {}", i + 1, pattern);
        }
    }

    // Create a WalkBuilder that respects .gitignore files and common ignore patterns
    let mut builder = WalkBuilder::new(path);

    // Configure the builder to respect .gitignore files
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
    ].into_iter().map(String::from).collect();

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
        ].into_iter().map(String::from).collect();
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

    // Recursively walk the directory and check each file
    for result in builder.build() {
        let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("Error walking directory: {}", err);
                continue;
            }
        };

        // Skip directories
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let file_path = entry.path();

        // Skip if this file was already found in the code search
        if already_found_files.contains(file_path) {
            continue;
        }

        // Get the file name as a string
        let file_name = match file_path.file_name() {
            Some(name) => name.to_string_lossy().to_lowercase(),
            None => continue,
        };

        // Check if any pattern matches the file name
        if all_patterns
            .iter()
            .any(|pattern| match Regex::new(pattern) {
                Ok(re) => re.is_match(&file_name),
                Err(e) => {
                    eprintln!("Error compiling regex pattern '{}': {}", pattern, e);
                    false
                }
            })
        {
            if debug_mode {
                println!("DEBUG: File '{}' matched a pattern", file_name);
            }
            matching_files.push(file_path.to_owned());
        }
    }

    println!(
        "Found {} files with names matching flexible patterns (including concatenated forms)",
        matching_files.len()
    );
    Ok(matching_files)
}

/// Function to determine which terms match in a filename or path
pub fn get_filename_matched_queries(
    file_path: &Path,
    search_root: &Path,
    term_pairs: &[Vec<(String, usize)>],
) -> HashSet<usize> {
    // Get the relative path from the search root
    let relative_path = file_path
        .strip_prefix(search_root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_lowercase();

    let mut matched_indices = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Checking path '{}' for term matches", relative_path);
    }

    for (_query_idx, terms) in term_pairs.iter().enumerate() {
        // Convert the terms to the format expected by create_term_patterns
        let term_string_pairs: Vec<(String, String)> = terms
            .iter()
            .map(|(term, _)| (term.clone(), term.clone()))
            .collect();

        // Generate patterns with term indices
        let patterns_with_term_indices = create_term_patterns(&term_string_pairs);

        // Check each pattern against the path
        for (pattern, pattern_term_indices) in patterns_with_term_indices {
            match Regex::new(&pattern) {
                Ok(re) => {
                    if re.is_match(&relative_path) {
                        // Map the pattern's term indices to the original term indices
                        for pattern_term_idx in pattern_term_indices {
                            if pattern_term_idx < terms.len() {
                                let (term, original_index) = &terms[pattern_term_idx];
                                if debug_mode {
                                    println!(
                                        "DEBUG:   Term '{}' (index {}) matched in path '{}'",
                                        term, original_index, relative_path
                                    );
                                }
                                matched_indices.insert(*original_index);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error compiling regex pattern '{}': {}", pattern, e);
                }
            }
        }
    }

    if debug_mode && !matched_indices.is_empty() {
        println!(
            "DEBUG:   Found {} term matches in path '{}'",
            matched_indices.len(),
            relative_path
        );
    }

    matched_indices
}

/// Compatibility function for the old get_filename_matched_queries signature
/// This will be used by existing code until it's updated to use the new function
pub fn get_filename_matched_queries_compat(
    filename: &str,
    queries_terms: &[Vec<(String, String)>],
) -> HashSet<usize> {
    let mut matched_terms = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Checking filename '{}' for term matches", filename);
    }

    let filename_lower = filename.to_lowercase();

    for (_query_idx, term_pairs) in queries_terms.iter().enumerate() {
        // Generate flexible patterns for this query's terms
        let patterns_with_term_indices = create_term_patterns(term_pairs);

        // Check each pattern against the filename
        for (pattern, term_indices) in patterns_with_term_indices {
            match Regex::new(&pattern) {
                Ok(re) => {
                    if re.is_match(&filename_lower) {
                        // If the pattern matches, add all the term indices it corresponds to
                        for term_idx in term_indices {
                            if debug_mode {
                                // Get the original term for debugging
                                if term_idx < term_pairs.len() {
                                    let (original_term, _) = &term_pairs[term_idx];
                                    println!(
                                        "DEBUG:   Term '{}' (index {}) matched in filename '{}'",
                                        original_term, term_idx, filename
                                    );
                                }
                            }
                            matched_terms.insert(term_idx);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error compiling regex pattern '{}': {}", pattern, e);
                }
            }
        }
    }

    if debug_mode && !matched_terms.is_empty() {
        println!(
            "DEBUG:   Found {} term matches in filename '{}'",
            matched_terms.len(),
            filename
        );
    }

    matched_terms
}
