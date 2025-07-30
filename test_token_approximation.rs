/// Token Approximation Accuracy Research Tool
///
/// This tool tests the accuracy of the "1 token ≈ 4 bytes" approximation used in
/// src/search/search_limiter.rs and identifies scenarios where it fails.
use probe_code::search::search_tokens::count_block_tokens;
use std::collections::HashMap;

/// Test case structure for token approximation accuracy testing
#[derive(Debug, Clone)]
struct TokenAccuracyTest {
    name: String,
    content: String,
    category: String,
}

/// Results of token approximation analysis
#[derive(Debug, Clone)]
struct AccuracyAnalysis {
    content: String,
    byte_length: usize,
    actual_tokens: usize,
    estimated_tokens: usize,
    bytes_per_token: f64,
    error_percentage: f64,
    category: String,
}

impl AccuracyAnalysis {
    fn new(content: &str, category: &str) -> Self {
        let byte_length = content.len();
        let actual_tokens = count_block_tokens(content);
        let estimated_tokens = (byte_length / 4).max(1); // Same logic as search_limiter.rs

        let bytes_per_token = if actual_tokens > 0 {
            byte_length as f64 / actual_tokens as f64
        } else {
            0.0
        };

        let error_percentage = if actual_tokens > 0 {
            ((estimated_tokens as f64 - actual_tokens as f64) / actual_tokens as f64) * 100.0
        } else {
            0.0
        };

        Self {
            content: content.to_string(),
            byte_length,
            actual_tokens,
            estimated_tokens,
            bytes_per_token,
            error_percentage,
            category: category.to_string(),
        }
    }
}

fn create_test_cases() -> Vec<TokenAccuracyTest> {
    vec![
        // Rust code samples
        TokenAccuracyTest {
            name: "Simple Rust function".to_string(),
            content: r#"fn main() {
    println!("Hello, world!");
}"#.to_string(),
            category: "rust_simple".to_string(),
        },
        TokenAccuracyTest {
            name: "Complex Rust struct".to_string(),
            content: r#"#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file: String,
    pub lines: (usize, usize),
    pub node_type: String,
    pub code: String,
    pub matched_by_filename: Option<bool>,
    pub rank: Option<usize>,
}"#.to_string(),
            category: "rust_complex".to_string(),
        },
        TokenAccuracyTest {
            name: "Rust with long identifiers".to_string(),
            content: r#"impl VeryLongAndDescriptiveStructName {
    pub fn extremely_long_method_name_with_many_parameters(
        &self,
        very_long_parameter_name_first: ComplexGenericType<WithManyTypeParameters>,
        another_extremely_long_parameter_name: Option<Result<String, Box<dyn Error>>>,
    ) -> Result<VeryComplexReturnType, CustomErrorType> {
        // implementation
    }
}"#.to_string(),
            category: "rust_long_identifiers".to_string(),
        },

        // JavaScript/TypeScript samples
        TokenAccuracyTest {
            name: "Simple JavaScript function".to_string(),
            content: r#"function calculateTokens(text) {
    return text.split(' ').length;
}"#.to_string(),
            category: "javascript_simple".to_string(),
        },
        TokenAccuracyTest {
            name: "Complex TypeScript interface".to_string(),
            content: r#"interface SearchConfiguration {
    maxResults?: number;
    maxTokens?: number;
    maxBytes?: number;
    enableCaching: boolean;
    rankingAlgorithm: 'tfidf' | 'bm25' | 'hybrid';
    filters: {
        fileTypes: string[];
        excludePatterns: RegExp[];
    };
}"#.to_string(),
            category: "typescript_complex".to_string(),
        },

        // Python samples
        TokenAccuracyTest {
            name: "Simple Python function".to_string(),
            content: r#"def count_tokens(text):
    """Count tokens in text."""
    return len(text.split())
"#.to_string(),
            category: "python_simple".to_string(),
        },
        TokenAccuracyTest {
            name: "Complex Python class".to_string(),
            content: r#"class TokenApproximationAnalyzer:
    """Analyzes the accuracy of token count approximations."""
    
    def __init__(self, tokenizer_model: str = "gpt-3.5-turbo"):
        self.tokenizer_model = tokenizer_model
        self.approximation_ratio = 4.0  # bytes per token
        self.accuracy_threshold = 0.9
    
    def analyze_content(self, content: str) -> Dict[str, Union[int, float]]:
        """Analyze token approximation accuracy for given content."""
        actual_tokens = self._count_actual_tokens(content)
        estimated_tokens = len(content) // 4
        
        return {
            'actual_tokens': actual_tokens,
            'estimated_tokens': estimated_tokens,
            'accuracy': estimated_tokens / actual_tokens if actual_tokens > 0 else 0,
            'bytes_per_token': len(content) / actual_tokens if actual_tokens > 0 else 0
        }
"#.to_string(),
            category: "python_complex".to_string(),
        },

        // Go samples
        TokenAccuracyTest {
            name: "Simple Go function".to_string(),
            content: r#"func countTokens(text string) int {
    words := strings.Fields(text)
    return len(words)
}"#.to_string(),
            category: "go_simple".to_string(),
        },
        TokenAccuracyTest {
            name: "Complex Go struct with methods".to_string(),
            content: r#"type TokenApproximationConfig struct {
    BytesPerTokenRatio    float64           `json:"bytes_per_token_ratio"`
    AccuracyThreshold     float64           `json:"accuracy_threshold"`
    EnablePreciseCounting bool              `json:"enable_precise_counting"`
    CacheConfiguration    *CacheConfig      `json:"cache_config,omitempty"`
    SupportedLanguages    []string          `json:"supported_languages"`
    CustomTokenizers      map[string]string `json:"custom_tokenizers"`
}

func (c *TokenApproximationConfig) EstimateTokens(content string) int {
    if c.EnablePreciseCounting {
        return c.countPreciseTokens(content)
    }
    return int(float64(len(content)) / c.BytesPerTokenRatio)
}"#.to_string(),
            category: "go_complex".to_string(),
        },

        // Edge cases
        TokenAccuracyTest {
            name: "Very short identifiers".to_string(),
            content: "a b c d e f g h i j k l m n o p q r s t u v w x y z".to_string(),
            category: "edge_short_identifiers".to_string(),
        },
        TokenAccuracyTest {
            name: "Single character tokens".to_string(),
            content: "( ) { } [ ] ; , . : = + - * / % & | ^ ~ ! @ # $ ? < >".to_string(),
            category: "edge_symbols".to_string(),
        },
        TokenAccuracyTest {
            name: "Long string literals".to_string(),
            content: r#"const message = "This is a very long string literal that contains many words and should represent a common case where string content dominates the token count and we want to see how the approximation handles this scenario";
const anotherMessage = "Another extremely long string that continues the pattern of having much more content within strings than in code structure itself";
"#.to_string(),
            category: "edge_long_strings".to_string(),
        },
        TokenAccuracyTest {
            name: "Comments heavy code".to_string(),
            content: r#"// This is a very long comment that explains the complex algorithm below
// and continues for many lines to test how comments affect token counting
// since comments tend to have natural language that might have different
// tokenization characteristics compared to code structure and identifiers
fn algorithm() {
    // More comments here explaining each step in detail
    // with comprehensive documentation for every line
    let x = 1; // inline comment explaining this variable
    let y = 2; // another inline comment with more explanation
}"#.to_string(),
            category: "edge_comments_heavy".to_string(),
        },
        TokenAccuracyTest {
            name: "Unicode and special characters".to_string(),
            content: r#"const message = "こんにちは世界"; // Japanese Hello World
const emoji = "🚀 🎉 💻 🔥"; // Emoji tokens
const math = "∑∏∆∇∂∞≠≤≥±×÷"; // Mathematical symbols
const currency = "€£¥₹₽₩₪"; // Currency symbols
"#.to_string(),
            category: "edge_unicode".to_string(),
        },
        TokenAccuracyTest {
            name: "Highly compressed code".to_string(),
            content: "let[a,b,c,d,e,f,g,h]=[1,2,3,4,5,6,7,8];const{x,y,z}={x:1,y:2,z:3};".to_string(),
            category: "edge_compressed".to_string(),
        },
        TokenAccuracyTest {
            name: "Whitespace heavy code".to_string(),
            content: r#"function                processData                (                input                ) {
    return                input
        .                split                (                ' '                )
        .                map                (                item                =>                item                .                trim                (                )                )
        .                filter                (                item                =>                item                .                length                >                0                );
}"#.to_string(),
            category: "edge_whitespace_heavy".to_string(),
        },

        // Real-world samples from the codebase itself
        TokenAccuracyTest {
            name: "Actual search_limiter.rs snippet".to_string(),
            content: r#"// Use rough estimation and only start precise counting if we're very close to the limit
let estimated_tokens = (r_bytes / 4).max(1);
let estimated_total_after = running_tokens + estimated_tokens;

// Only start precise counting if we're within 90% of the limit based on estimation
if !token_counting_started
    && estimated_total_after >= (max_token_limit as f64 * 0.9) as usize
{"#.to_string(),
            category: "real_world_rust".to_string(),
        },
    ]
}

fn analyze_approximation_accuracy() {
    println!("=== TOKEN APPROXIMATION ACCURACY ANALYSIS ===\n");

    let test_cases = create_test_cases();
    let mut analyses: Vec<AccuracyAnalysis> = Vec::new();
    let mut category_stats: HashMap<String, Vec<f64>> = HashMap::new();

    println!("Testing {} code samples...\n", test_cases.len());

    for test_case in test_cases {
        let analysis = AccuracyAnalysis::new(&test_case.content, &test_case.category);

        println!("=== {} ===", test_case.name);
        println!("Category: {}", analysis.category);
        println!("Content length: {} bytes", analysis.byte_length);
        println!("Actual tokens: {}", analysis.actual_tokens);
        println!("Estimated tokens (÷4): {}", analysis.estimated_tokens);
        println!("Actual bytes/token: {:.2}", analysis.bytes_per_token);
        println!("Error: {:.1}%", analysis.error_percentage);

        if analysis.error_percentage.abs() > 20.0 {
            println!("⚠️  HIGH ERROR: {}% deviation!", analysis.error_percentage);
        }

        println!(
            "Content preview: {}",
            if analysis.content.len() > 100 {
                format!("{}...", &analysis.content[..100])
            } else {
                analysis.content.clone()
            }
        );
        println!();

        // Track category statistics
        category_stats
            .entry(analysis.category.clone())
            .or_default()
            .push(analysis.bytes_per_token);

        analyses.push(analysis);
    }

    // Overall statistics
    println!("=== OVERALL ANALYSIS ===");
    let total_actual_tokens: usize = analyses.iter().map(|a| a.actual_tokens).sum();
    let total_estimated_tokens: usize = analyses.iter().map(|a| a.estimated_tokens).sum();
    let total_bytes: usize = analyses.iter().map(|a| a.byte_length).sum();

    let overall_bytes_per_token = total_bytes as f64 / total_actual_tokens as f64;
    let overall_error = ((total_estimated_tokens as f64 - total_actual_tokens as f64)
        / total_actual_tokens as f64)
        * 100.0;

    println!("Total bytes: {}", total_bytes);
    println!("Total actual tokens: {}", total_actual_tokens);
    println!("Total estimated tokens: {}", total_estimated_tokens);
    println!("Overall bytes/token: {:.2}", overall_bytes_per_token);
    println!("Overall error: {:.1}%", overall_error);

    // Error distribution
    let mut high_errors = 0;
    let mut medium_errors = 0;
    let mut low_errors = 0;

    for analysis in &analyses {
        let abs_error = analysis.error_percentage.abs();
        if abs_error > 50.0 {
            high_errors += 1;
        } else if abs_error > 20.0 {
            medium_errors += 1;
        } else {
            low_errors += 1;
        }
    }

    println!("\nError Distribution:");
    println!("  High errors (>50%): {} samples", high_errors);
    println!("  Medium errors (20-50%): {} samples", medium_errors);
    println!("  Low errors (<20%): {} samples", low_errors);

    // Category analysis
    println!("\n=== CATEGORY ANALYSIS ===");
    for (category, ratios) in category_stats {
        let avg_ratio: f64 = ratios.iter().sum::<f64>() / ratios.len() as f64;
        let min_ratio = ratios.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_ratio = ratios.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        println!(
            "{}: avg={:.2}, min={:.2}, max={:.2} bytes/token",
            category, avg_ratio, min_ratio, max_ratio
        );
    }

    // Find worst cases
    println!("\n=== WORST CASES (Highest Error %) ===");
    let mut sorted_analyses = analyses.clone();
    sorted_analyses.sort_by(|a, b| {
        b.error_percentage
            .abs()
            .partial_cmp(&a.error_percentage.abs())
            .unwrap()
    });

    for analysis in sorted_analyses.iter().take(5) {
        println!(
            "{}: {:.1}% error ({:.2} bytes/token)",
            analysis.category, analysis.error_percentage, analysis.bytes_per_token
        );
    }

    // Simulate 90% threshold behavior
    println!("\n=== 90% THRESHOLD SIMULATION ===");
    simulate_threshold_behavior(&analyses);
}

fn simulate_threshold_behavior(analyses: &[AccuracyAnalysis]) {
    println!("Simulating search_limiter.rs behavior with 90% threshold...");

    let test_limits = vec![1000, 2000, 5000, 10000]; // Different token limits to test

    for limit in test_limits {
        println!("\n--- Testing with {} token limit ---", limit);

        let mut running_tokens = 0;
        let mut running_bytes = 0;
        let mut token_counting_started = false;
        let mut overruns = 0;
        let mut close_calls = 0;

        for analysis in analyses {
            // Simulate the logic from search_limiter.rs
            let r_bytes = analysis.byte_length;
            let estimated_tokens = (r_bytes / 4).max(1);
            let estimated_total_after = running_tokens + estimated_tokens;

            // Check if we would start precise counting (90% threshold)
            if !token_counting_started && estimated_total_after >= (limit as f64 * 0.9) as usize {
                token_counting_started = true;
                println!(
                    "  Started precise counting at {} estimated tokens",
                    estimated_total_after
                );
            }

            let r_tokens = if token_counting_started {
                analysis.actual_tokens
            } else {
                estimated_tokens
            };

            // Check if we would exceed the limit
            if running_tokens + r_tokens > limit {
                if !token_counting_started {
                    // This is a potential overrun due to estimation error
                    let _actual_total = running_bytes + analysis.byte_length;

                    if running_tokens + analysis.actual_tokens > limit {
                        overruns += 1;
                        println!("  ⚠️  OVERRUN: Would exceed limit due to estimation error");
                        println!(
                            "      Estimated: {} + {} = {}",
                            running_tokens,
                            r_tokens,
                            running_tokens + r_tokens
                        );
                        println!(
                            "      Actual would be: {} + {} = {}",
                            running_tokens,
                            analysis.actual_tokens,
                            running_tokens + analysis.actual_tokens
                        );
                    }
                }
                break; // Would stop processing here
            }

            // Check for close calls (within 5% of limit)
            if (running_tokens + r_tokens) as f64 > limit as f64 * 0.95 {
                close_calls += 1;
            }

            running_tokens += r_tokens;
            running_bytes += r_bytes;
        }

        println!("  Final token count: {}/{}", running_tokens, limit);
        println!("  Precise counting started: {}", token_counting_started);
        println!("  Potential overruns: {}", overruns);
        println!("  Close calls: {}", close_calls);
    }
}

fn main() {
    analyze_approximation_accuracy();

    println!("\n=== RECOMMENDATIONS ===");
    println!("1. The 4 bytes/token approximation shows significant variation across content types");
    println!("2. Consider using more conservative thresholds (e.g., 80% instead of 90%)");
    println!("3. Implement dynamic approximation based on content characteristics");
    println!("4. Use adaptive ratios for different programming languages");
    println!("5. Add safety margins to prevent overruns in critical scenarios");
}
