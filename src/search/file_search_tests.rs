use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use probe_code::search::file_search::{search_file_for_pattern, find_files_with_pattern, find_matching_filenames};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> std::path::PathBuf {
        let file_path = dir.path().join(filename);
        let mut file = File::create(&file_path).expect("Failed to create test file");
        file.write_all(content.as_bytes()).expect("Failed to write test content");
        file_path
    }

    fn create_test_directory_structure(dir: &TempDir) -> Vec<PathBuf> {
        let mut created_files = Vec::new();

        // Create a few test files with different content
        let js_content = "function searchTest() {\n  return 'This is a test function';\n}\n";
        let rs_content = "fn search_test() -> &'static str {\n    \"This is a test function\"\n}\n";
        let txt_content = "This is a test file with the word search in it.\nIt has multiple lines.\n";

        created_files.push(create_test_file(dir, "test.js", js_content));
        created_files.push(create_test_file(dir, "test.rs", rs_content));
        created_files.push(create_test_file(dir, "test.txt", txt_content));
        created_files.push(create_test_file(dir, "search_result.txt", "This file's name contains search"));

        // Create a subdirectory with more files
        let subdir_path = dir.path().join("subdir");
        std::fs::create_dir(&subdir_path).expect("Failed to create subdirectory");

        let subfile_content = "Another file that mentions search but in a subdirectory";
        created_files.push(create_test_file(
            dir,
            "subdir/nested_file.txt",
            subfile_content
        ));

        // Create a file to be ignored
        let ignored_dir = dir.path().join("node_modules");
        std::fs::create_dir(&ignored_dir).expect("Failed to create ignored directory");
        create_test_file(
            dir,
            "node_modules/should_be_ignored.js",
            "This file should be ignored by default"
        );

        created_files
    }

    #[test]
    fn test_search_file_for_pattern() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "line 1: no match\nline 2: contains search term\nline 3: no match\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        let (matched, line_matches) = search_file_for_pattern(&file_path, "search")
            .expect("Failed to search file");

        assert!(matched);
        assert_eq!(line_matches.len(), 1);

        // Check that line 2 contains a match
        assert!(line_matches.contains_key(&2));

        // Check that the match content is correct
        let matches = line_matches.get(&2).unwrap();
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m == "search"));
    }

    #[test]
    fn test_search_file_for_pattern_no_match() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "line 1: no match\nline 2: no match\nline 3: no match\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        let (matched, line_matches) = search_file_for_pattern(&file_path, "nonexistent")
            .expect("Failed to search file");

        assert!(!matched);
        assert_eq!(line_matches.len(), 0);
    }

    #[test]
    fn test_search_file_for_pattern_case_insensitive() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "Line 1: no match\nLine 2: contains SEARCH term\nLine 3: no match\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        // Search should be case-insensitive by default
        let (matched, line_matches) = search_file_for_pattern(&file_path, "search")
            .expect("Failed to search file");

        assert!(matched);
        assert_eq!(line_matches.len(), 1);

        // Check that line 2 contains a match
        assert!(line_matches.contains_key(&2));

        // Check that the match content is correct (case-insensitive)
        let matches = line_matches.get(&2).unwrap();
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.to_uppercase() == "SEARCH"));
    }

    #[test]
    fn test_search_file_for_pattern_word_boundaries() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "line 1: ip address\nline 2: skipped\nline 3: ip\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        // With word boundaries (exact=false), "ip" should match "ip" but not "skipped"
        let (matched, line_matches) = search_file_for_pattern(&file_path, "ip")
            .expect("Failed to search file");

        assert!(matched);

        // Get the line numbers from the keys
        let line_numbers: HashSet<usize> = line_matches.keys().cloned().collect();

        assert_eq!(line_numbers.len(), 2);
        assert!(line_numbers.contains(&1));  // Line 1 contains "ip"
        assert!(line_numbers.contains(&3));  // Line 3 contains "ip"
        assert!(!line_numbers.contains(&2)); // Line 2 contains "skipped" but should not match

        // Check match content
        assert!(line_matches.get(&1).unwrap().iter().any(|m| m == "ip"));
        assert!(line_matches.get(&3).unwrap().iter().any(|m| m == "ip"));
    }

    #[test]
    fn test_search_file_for_pattern_exact_mode() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "line 1: ip address\nline 2: skipped\nline 3: ip\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        // In exact mode (exact=true), "ip" should match both "ip" and "skipped"
        let (matched, line_matches) = search_file_for_pattern(&file_path, "ip")
            .expect("Failed to search file");

        assert!(matched);

        // Get the line numbers from the keys
        let line_numbers: HashSet<usize> = line_matches.keys().cloned().collect();

        assert_eq!(line_numbers.len(), 3);  // All 3 lines should match
        assert!(line_numbers.contains(&1));  // Line 1 contains "ip"
        assert!(line_numbers.contains(&2));  // Line 2 contains "skipped" which contains "ip"
        assert!(line_numbers.contains(&3));  // Line 3 contains "ip"

        // Check match content
        assert!(line_matches.get(&1).unwrap().iter().any(|m| m == "ip"));
        assert!(line_matches.get(&2).unwrap().iter().any(|m| m == "ip"));
        assert!(line_matches.get(&3).unwrap().iter().any(|m| m == "ip"));
    }

    #[test]
    fn test_find_files_with_pattern() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        create_test_directory_structure(&temp_dir);

        let matching_files = find_files_with_pattern(
            temp_dir.path(),
            "search",
            &[],
            false
        ).expect("Failed to find files with pattern");

        // Should find all files containing "search" (ignoring node_modules)
        assert!(matching_files.len() >= 3);  // At least test.txt, search_result.txt, and nested_file.txt

        // Verify specific expected files
        let found_txt = matching_files.iter().any(|p| p.ends_with("test.txt"));
        let found_nested = matching_files.iter().any(|p| p.ends_with("nested_file.txt"));

        assert!(found_txt);
        assert!(found_nested);

        // node_modules should be ignored
        let found_ignored = matching_files.iter().any(|p| p.to_string_lossy().contains("node_modules"));
        assert!(!found_ignored);
    }

    #[test]
    fn test_find_matching_filenames() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        create_test_directory_structure(&temp_dir);

        let already_found = HashSet::new();
        let matching_files = find_matching_filenames(
            temp_dir.path(),
            &["search".to_string()],
            &already_found,
            &[],
            false
        ).expect("Failed to find matching filenames");

        // Should find files with "search" in the name
        assert!(!matching_files.is_empty());

        // Verify the specific expected file is found
        let found_search_result = matching_files.iter().any(|p| p.ends_with("search_result.txt"));
        assert!(found_search_result);
    }

    #[test]
    fn test_find_matching_filenames_with_already_found() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let files = create_test_directory_structure(&temp_dir);

        // Mark search_result.txt as already found
        let mut already_found = HashSet::new();
        for file in &files {
            if file.ends_with("search_result.txt") {
                already_found.insert(file.clone());
                break;
            }
        }

        let matching_files = find_matching_filenames(
            temp_dir.path(),
            &["search".to_string()],
            &already_found,
            &[],
            false
        ).expect("Failed to find matching filenames");

        // Should not find search_result.txt since it's already found
        let found_search_result = matching_files.iter().any(|p| p.ends_with("search_result.txt"));
        assert!(!found_search_result);
    }

    #[test]
    fn test_find_files_with_custom_ignore() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        create_test_directory_structure(&temp_dir);

        // Create a custom ignore pattern for txt files
        let custom_ignores = vec!["*.txt".to_string()];

        let matching_files = find_files_with_pattern(
            temp_dir.path(),
            "search",
            &custom_ignores,
            false
        ).expect("Failed to find files with pattern");

        // Should not find any txt files
        let found_txt = matching_files.iter().any(|p| p.to_string_lossy().ends_with(".txt"));
        assert!(!found_txt);
    }
}
