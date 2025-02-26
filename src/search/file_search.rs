use anyhow::{Context, Result};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use regex::Regex;

use crate::language::is_test_file;
use crate::search::query::{preprocess_query, regex_escape};

/// Searches a file for a pattern and returns whether it matched and the matching line numbers
pub fn search_file_for_pattern(file_path: &Path, pattern: &str, exact: bool) -> Result<(bool, HashSet<usize>)> {
    let mut matched = false;
    let mut line_numbers = HashSet::new();
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    // Use word boundaries unless exact mode is specified
    let adjusted_pattern = if exact { 
        pattern.to_string() 
    } else { 
        format!(r"\b{}\b", pattern) 
    };

    // Create a case-insensitive regex matcher for the pattern
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(&adjusted_pattern)
        .context(format!("Failed to create regex matcher for: {}", adjusted_pattern))?;

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
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    println!("Running rgrep search with pattern: {}", pattern);

    if debug_mode {
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
    let common_ignores = [
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
    ];

    for pattern in &common_ignores {
        builder.add_custom_ignore_filename(pattern);
    }

    // Add custom ignore patterns
    for pattern in custom_ignores {
        // Create an override builder for glob patterns
        let mut override_builder = ignore::overrides::OverrideBuilder::new(path);
        override_builder.add(&format!("!{}", pattern)).unwrap();
        let overrides = override_builder.build().unwrap();
        builder.overrides(overrides);
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
        
        // Skip test files unless allow_tests is true
        if !allow_tests && is_test_file(file_path) {
            if debug_mode {
                println!("DEBUG: Skipping test file: {:?}", file_path);
            }
            continue;
        }

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

    println!(
        "Searched {} files, found {} matches",
        total_files,
        matching_files.len()
    );

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
    let mut term_pairs = Vec::new();
    for query in queries {
        term_pairs.extend(preprocess_query(query, false)); // Use non-exact mode for filename matching
    }

    println!("Looking for filenames matching queries (with stemming): {:?}", queries);
    
    // Debug output for stemmed terms
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";
    if debug_mode && !term_pairs.is_empty() {
        println!("DEBUG: Using the following term pairs for filename matching:");
        for (i, (original, stemmed)) in term_pairs.iter().enumerate() {
            if original == stemmed {
                println!("DEBUG:   {}. {} (stemmed same as original)", i + 1, original);
            } else {
                println!("DEBUG:   {}. {} (stemmed to {})", i + 1, original, stemmed);
            }
        }
    }

    // Create a WalkBuilder that respects .gitignore files and common ignore patterns
    let mut builder = WalkBuilder::new(path);

    // Configure the builder to respect .gitignore files
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);

    // Add common directories to ignore
    let common_ignores = [
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
    ];

    for pattern in &common_ignores {
        builder.add_custom_ignore_filename(pattern);
    }

    // Add custom ignore patterns
    for pattern in custom_ignores {
        // Create an override builder for glob patterns
        let mut override_builder = ignore::overrides::OverrideBuilder::new(path);
        override_builder.add(&format!("!{}", pattern)).unwrap();
        let overrides = override_builder.build().unwrap();
        builder.overrides(overrides);
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
        
        // Skip test files unless allow_tests is true
        if !allow_tests && is_test_file(file_path) {
            continue;
        }

        // Get the file name as a string
        let file_name = match file_path.file_name() {
            Some(name) => name.to_string_lossy().to_lowercase(),
            None => continue,
        };

        // Check if any term (original or stemmed) matches the file name using word boundaries
        for (original, stemmed) in &term_pairs {
            // Create a regex pattern that matches either the original or stemmed term
            let pattern = if original == stemmed {
                // If stemmed and original are the same, just use one with word boundaries
                format!(r"\b{}\b", regex_escape(original))
            } else {
                // Otherwise, create an OR pattern with word boundaries
                format!(r"\b({}|{})\b", regex_escape(original), regex_escape(stemmed))
            };
            
            // Create and check the regex
            let re = Regex::new(&pattern).unwrap();
            if re.is_match(&file_name) {
                if debug_mode {
                    if original == stemmed {
                        println!("DEBUG: File '{}' matched term '{}'", file_name, original);
                    } else {
                        println!("DEBUG: File '{}' matched term '{}' or its stemmed form '{}'", 
                                 file_name, original, stemmed);
                    }
                }
                matching_files.push(file_path.to_owned());
                break;
            }
        }
    }

    println!(
        "Found {} files with names containing whole-word matches of query words (including stemmed forms)",
        matching_files.len()
    );
    Ok(matching_files)
}
