use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use probe::language::parse_file_for_code_blocks;
use tempfile::NamedTempFile;
use std::fs;
use std::collections::{HashMap, HashSet};

/// Create temporary files with different language content
fn create_temp_file(content: &str, extension: &str) -> NamedTempFile {
    let temp_file = NamedTempFile::new().unwrap();
    let temp_path = temp_file.path().with_extension(extension);
    fs::write(&temp_path, content).unwrap();
    temp_file
}

/// Simple sample code content for different languages
fn get_sample_code(language: &str) -> &'static str {
    match language {
        "rust" => r#"
use std::collections::HashMap;

pub struct SearchResult {
    pub file_path: String,
    pub line_number: usize,
    pub content: String,
    pub score: f64,
}

impl SearchResult {
    pub fn new(file_path: String, line_number: usize, content: String) -> Self {
        Self {
            file_path,
            line_number,
            content,
            score: 0.0,
        }
    }
}

fn calculate_score(line: &str) -> f64 {
    let mut score = 0.0;
    
    if line.contains("fn ") {
        score += 2.0;
    }
    
    if line.contains("struct ") {
        score += 1.5;
    }
    
    if line.contains("use ") {
        score += 1.0;
    }
    
    score += line.len() as f64 * 0.01;
    score
}

fn main() {
    let mut results = Vec::new();
    
    for i in 0..100 {
        let content = format!("Line {}: test content", i);
        let result = SearchResult::new(
            "test.rs".to_string(),
            i,
            content,
        );
        results.push(result);
    }
    
    println!("Found {} results", results.len());
}
"#,
        "javascript" => r#"
class SearchResult {
    constructor(filePath, lineNumber, content) {
        this.filePath = filePath;
        this.lineNumber = lineNumber;
        this.content = content;
        this.score = this.calculateScore(content);
    }
    
    calculateScore(line) {
        let score = 0;
        
        if (line.includes("function ")) {
            score += 2;
        }
        
        if (line.includes("class ")) {
            score += 1.5;
        }
        
        if (line.includes("const ")) {
            score += 1;
        }
        
        score += line.length * 0.01;
        return score;
    }
}

function main() {
    const results = [];
    
    for (let i = 0; i < 100; i++) {
        const content = `Line ${i}: test content`;
        const result = new SearchResult("test.js", i, content);
        results.push(result);
    }
    
    console.log(`Found ${results.length} results`);
}

main();
"#,
        "python" => r#"
class SearchResult:
    def __init__(self, file_path, line_number, content):
        self.file_path = file_path
        self.line_number = line_number
        self.content = content
        self.score = self.calculate_score(content)
    
    def calculate_score(self, line):
        score = 0.0
        
        if "def " in line:
            score += 2.0
        
        if "class " in line:
            score += 1.5
        
        if "import " in line:
            score += 1.0
        
        score += len(line) * 0.01
        return score

def main():
    results = []
    
    for i in range(100):
        content = f"Line {i}: test content"
        result = SearchResult("test.py", i, content)
        results.append(result)
    
    print(f"Found {len(results)} results")

if __name__ == "__main__":
    main()
"#,
        "go" => r#"
package main

import (
    "fmt"
    "strings"
)

type SearchResult struct {
    FilePath   string
    LineNumber int
    Content    string
    Score      float64
}

func NewSearchResult(filePath string, lineNumber int, content string) *SearchResult {
    return &SearchResult{
        FilePath:   filePath,
        LineNumber: lineNumber,
        Content:    content,
        Score:      calculateScore(content),
    }
}

func calculateScore(line string) float64 {
    score := 0.0
    
    if strings.Contains(line, "func ") {
        score += 2.0
    }
    
    if strings.Contains(line, "type ") {
        score += 1.5
    }
    
    if strings.Contains(line, "import") {
        score += 1.0
    }
    
    score += float64(len(line)) * 0.01
    return score
}

func main() {
    results := make([]*SearchResult, 0, 100)
    
    for i := 0; i < 100; i++ {
        content := fmt.Sprintf("Line %d: test content", i)
        result := NewSearchResult("test.go", i, content)
        results = append(results, result)
    }
    
    fmt.Printf("Found %d results\n", len(results))
}
"#,
        _ => "// Default content",
    }
}

/// Benchmark parsing different languages
fn benchmark_language_parsing(c: &mut Criterion) {
    let languages = vec![
        ("rust", "rs"),
        ("javascript", "js"),
        ("python", "py"),
        ("go", "go"),
    ];
    
    let mut group = c.benchmark_group("language_parsing");
    
    for (language, extension) in languages {
        let content = get_sample_code(language);
        
        group.bench_with_input(
            BenchmarkId::new("parse_language", language),
            &(content, extension),
            |b, (content, extension)| {
                b.iter(|| {
                    let temp_file = create_temp_file(content, extension);
                    let path = temp_file.path().with_extension(extension);
                    let file_content = fs::read_to_string(&path).unwrap();
                    
                    let result = parse_file_for_code_blocks(
                        &file_content,
                        extension,
                        &HashSet::new(),
                        false,
                        None
                    );
                    
                    black_box(result)
                })
            }
        );
    }
    
    group.finish();
}

/// Benchmark parsing files of different sizes
fn benchmark_file_sizes(c: &mut Criterion) {
    let base_content = get_sample_code("rust");
    let sizes = vec![
        ("small", 1),
        ("medium", 5),
        ("large", 20),
    ];
    
    let mut group = c.benchmark_group("file_sizes");
    
    for (size_name, multiplier) in sizes {
        let content = base_content.repeat(multiplier);
        
        group.bench_with_input(
            BenchmarkId::new("parse_size", size_name),
            &content,
            |b, content| {
                b.iter(|| {
                    let temp_file = create_temp_file(content, "rs");
                    let path = temp_file.path().with_extension("rs");
                    let file_content = fs::read_to_string(&path).unwrap();
                    
                    let result = parse_file_for_code_blocks(
                        &file_content,
                        "rs",
                        &HashSet::new(),
                        false,
                        None
                    );
                    
                    black_box(result)
                })
            }
        );
    }
    
    group.finish();
}

/// Benchmark parsing with different line number sets
fn benchmark_line_filtering(c: &mut Criterion) {
    let content = get_sample_code("rust");
    let line_sets = vec![
        ("all_lines", (1..=50).collect::<HashSet<_>>()),
        ("few_lines", vec![1, 5, 10, 15, 20].into_iter().collect()),
        ("many_lines", (1..=25).collect::<HashSet<_>>()),
    ];
    
    let mut group = c.benchmark_group("line_filtering");
    
    for (name, line_numbers) in line_sets {
        group.bench_with_input(
            BenchmarkId::new("filter_lines", name),
            &(content, line_numbers),
            |b, (content, line_numbers)| {
                b.iter(|| {
                    let temp_file = create_temp_file(content, "rs");
                    let path = temp_file.path().with_extension("rs");
                    let file_content = fs::read_to_string(&path).unwrap();
                    
                    let result = parse_file_for_code_blocks(
                        &file_content,
                        "rs",
                        line_numbers,
                        false,
                        None
                    );
                    
                    black_box(result)
                })
            }
        );
    }
    
    group.finish();
}

/// Benchmark parsing with and without test files
fn benchmark_test_inclusion(c: &mut Criterion) {
    let content = get_sample_code("rust");
    let test_options = vec![
        ("include_tests", true),
        ("exclude_tests", false),
    ];
    
    let mut group = c.benchmark_group("test_inclusion");
    
    for (name, include_tests) in test_options {
        group.bench_with_input(
            BenchmarkId::new("test_option", name),
            &(content, include_tests),
            |b, (content, include_tests)| {
                b.iter(|| {
                    let temp_file = create_temp_file(content, "rs");
                    let path = temp_file.path().with_extension("rs");
                    let file_content = fs::read_to_string(&path).unwrap();
                    
                    let result = parse_file_for_code_blocks(
                        &file_content,
                        "rs",
                        &HashSet::new(),
                        *include_tests,
                        None
                    );
                    
                    black_box(result)
                })
            }
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_language_parsing,
    benchmark_file_sizes,
    benchmark_line_filtering,
    benchmark_test_inclusion
);
criterion_main!(benches);