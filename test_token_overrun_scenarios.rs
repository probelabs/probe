// use probe_code::models::SearchResult; // Commented out unused import
/// Token Limit Overrun Demonstration
///
/// This tool creates realistic scenarios where the 4 bytes/token approximation
/// causes actual token limit overruns, proving the issue exists in practice.
use probe_code::search::search_tokens::count_block_tokens;

#[derive(Debug)]
struct OverrunTest {
    name: String,
    content: String,
    #[allow(dead_code)]
    expected_category: String,
}

/// Simulate the exact logic from search_limiter.rs
fn simulate_search_limiter_logic(
    content_blocks: &[String],
    max_tokens: usize,
) -> (bool, usize, usize, Vec<String>) {
    let mut limited = Vec::new();
    let mut running_tokens = 0;
    let mut token_counting_started = false;

    println!(
        "Simulating search_limiter.rs with {} token limit",
        max_tokens
    );

    for (i, content) in content_blocks.iter().enumerate() {
        let r_bytes = content.len();
        let estimated_tokens = (r_bytes / 4).max(1);
        let estimated_total_after = running_tokens + estimated_tokens;

        println!(
            "Block {}: {} bytes, est. {} tokens",
            i + 1,
            r_bytes,
            estimated_tokens
        );

        // Check if we would start precise counting (90% threshold)
        if !token_counting_started && estimated_total_after >= (max_tokens as f64 * 0.9) as usize {
            token_counting_started = true;
            println!("  Started precise counting (90% threshold reached)");

            // Recalculate tokens for already included results
            running_tokens = limited
                .iter()
                .map(|block: &String| count_block_tokens(block))
                .sum();
            println!("  Recalculated running tokens: {}", running_tokens);
        }

        let r_tokens = if token_counting_started {
            count_block_tokens(content)
        } else {
            estimated_tokens
        };

        println!(
            "  Using {} tokens (precise={})",
            r_tokens, token_counting_started
        );

        // Check if we would exceed the limit
        if running_tokens + r_tokens > max_tokens {
            println!(
                "  Would exceed limit ({} + {} > {}), stopping",
                running_tokens, r_tokens, max_tokens
            );
            break;
        }

        running_tokens += r_tokens;
        limited.push(content.clone());
        println!("  Added block, running total: {} tokens", running_tokens);
    }

    // Calculate final actual token count
    let final_actual_tokens = limited.iter().map(|block| count_block_tokens(block)).sum();

    let overrun_occurred = final_actual_tokens > max_tokens;

    println!("Final results:");
    println!("  Blocks included: {}", limited.len());
    println!("  Final running tokens (as tracked): {}", running_tokens);
    println!("  Final actual tokens: {}", final_actual_tokens);
    println!("  Overrun occurred: {}", overrun_occurred);

    (
        overrun_occurred,
        running_tokens,
        final_actual_tokens,
        limited,
    )
}

fn create_overrun_test_cases() -> Vec<OverrunTest> {
    vec![
        // Case 1: Highly compressed code (very low bytes/token ratio)
        OverrunTest {
            name: "Compressed code overrun".to_string(),
            content: "let[a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,q,r,s,t,u,v,w,x,y,z]=[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26];const{x,y,z,w}={x:a,y:b,z:c,w:d};".to_string(),
            expected_category: "compressed".to_string(),
        },

        // Case 2: Symbol-heavy code
        OverrunTest {
            name: "Symbol heavy overrun".to_string(),
            content: "()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}()[]{}".to_string(),
            expected_category: "symbols".to_string(),
        },

        // Case 3: Short identifier heavy code
        OverrunTest {
            name: "Short identifiers overrun".to_string(),
            content: "a=1;b=2;c=3;d=4;e=5;f=6;g=7;h=8;i=9;j=10;k=11;l=12;m=13;n=14;o=15;p=16;q=17;r=18;s=19;t=20;u=21;v=22;w=23;x=24;y=25;z=26;".to_string(),
            expected_category: "short_vars".to_string(),
        },

        // Case 4: Mixed case that looks normal but has low bytes/token
        OverrunTest {
            name: "Deceptively normal code".to_string(),
            content: "fn f(a:i32,b:i32){a+b} fn g(x:i32){x*2} fn h(y:i32){y-1} fn i(z:i32){z/2} fn j(w:i32){w%3}".to_string(),
            expected_category: "normal_looking".to_string(),
        },

        // Case 5: Unicode heavy (can have surprising tokenization)
        OverrunTest {
            name: "Unicode tokenization surprise".to_string(),
            content: "变量一=1;变量二=2;变量三=3;变量四=4;变量五=5;变量六=6;变量七=7;变量八=8;变量九=9;变量十=10;变量十一=11;变量十二=12;".to_string(),
            expected_category: "unicode".to_string(),
        },
    ]
}

fn create_realistic_code_blocks() -> Vec<String> {
    vec![
        // Normal looking code that would pass estimation
        r#"function processUserData(userData) {
    const validated = validateInput(userData);
    return {
        processed: true,
        data: validated
    };
}"#.to_string(),

        // Another normal block
        r#"class DataProcessor {
    constructor(config) {
        this.config = config;
        this.results = [];
    }

    process(input) {
        return this.transform(input);
    }
}"#.to_string(),

        // The killer: compressed/symbol heavy code that will cause overrun
        "let[a,b,c,d,e]=[1,2,3,4,5];const{x,y,z}={x:a,y:b,z:c};if(x>y){z=x+y}else{z=x-y}for(let i=0;i<10;i++){console.log(i)}".to_string(),

        // More compressed code
        "()=>({a:1,b:2,c:3,d:4,e:5,f:6,g:7,h:8,i:9,j:10}).map((k,v)=>k+v).filter(x=>x>5).reduce((a,b)=>a+b,0)".to_string(),

        // Symbol soup
        "{}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[](){}[]()".to_string(),
    ]
}

fn test_overrun_scenarios() {
    println!("=== TOKEN LIMIT OVERRUN SCENARIOS ===\n");

    let test_cases = create_overrun_test_cases();

    for (i, test_case) in test_cases.iter().enumerate() {
        println!("=== Test Case {}: {} ===", i + 1, test_case.name);

        let bytes = test_case.content.len();
        let actual_tokens = count_block_tokens(&test_case.content);
        let estimated_tokens = (bytes / 4).max(1);
        let bytes_per_token = bytes as f64 / actual_tokens as f64;
        let error_pct =
            ((estimated_tokens as f64 - actual_tokens as f64) / actual_tokens as f64) * 100.0;

        println!(
            "Content: {}",
            if test_case.content.chars().count() > 80 {
                format!(
                    "{}...",
                    test_case.content.chars().take(80).collect::<String>()
                )
            } else {
                test_case.content.clone()
            }
        );
        println!("Bytes: {}", bytes);
        println!("Actual tokens: {}", actual_tokens);
        println!("Estimated tokens: {}", estimated_tokens);
        println!("Bytes/token: {:.2}", bytes_per_token);
        println!("Estimation error: {:.1}%", error_pct);

        if error_pct < -30.0 {
            println!("⚠️  SEVERE UNDERESTIMATION - HIGH RISK OF OVERRUN!");
        }

        println!();
    }
}

fn test_realistic_overrun_scenario() {
    println!("\n=== REALISTIC OVERRUN SIMULATION ===\n");

    let blocks = create_realistic_code_blocks();
    let token_limit = 100; // Small limit to trigger the issue

    println!(
        "Testing with realistic code blocks and {} token limit",
        token_limit
    );
    println!("Blocks to process:");

    for (i, block) in blocks.iter().enumerate() {
        let bytes = block.len();
        let tokens = count_block_tokens(block);
        let estimated = (bytes / 4).max(1);
        println!(
            "  {}: {} bytes, {} actual tokens, {} estimated",
            i + 1,
            bytes,
            tokens,
            estimated
        );
    }

    println!("\n--- Simulation Results ---");
    let (overrun, tracked_tokens, actual_tokens, included_blocks) =
        simulate_search_limiter_logic(&blocks, token_limit);

    if overrun {
        println!("🚨 OVERRUN DETECTED!");
        println!("  Limit: {} tokens", token_limit);
        println!("  Tracked total: {} tokens", tracked_tokens);
        println!("  Actual total: {} tokens", actual_tokens);
        println!("  Overrun amount: {} tokens", actual_tokens - token_limit);
        println!(
            "  Overrun percentage: {:.1}%",
            ((actual_tokens - token_limit) as f64 / token_limit as f64) * 100.0
        );
    } else {
        println!("No overrun detected in this scenario");
    }

    println!(
        "\nBlocks included: {}/{}",
        included_blocks.len(),
        blocks.len()
    );
}

fn test_edge_case_accumulation() {
    println!("\n=== EDGE CASE ACCUMULATION TEST ===\n");

    // Create many small blocks with low bytes/token ratios
    let mut blocks = Vec::new();

    // Add 20 blocks of compressed code
    for i in 0..20 {
        blocks.push(format!("a{}=1;b{}=2;c{}=3;d{}={};", i, i, i, i, i * 2));
    }

    // Add 10 blocks of symbol soup
    for _i in 0..10 {
        blocks.push("()[]{}()[]{}()[]{}".to_string());
    }

    println!("Created {} blocks of low bytes/token content", blocks.len());

    let token_limit = 200;
    println!("Testing with {} token limit", token_limit);

    let (overrun, tracked_tokens, actual_tokens, included_blocks) =
        simulate_search_limiter_logic(&blocks, token_limit);

    if overrun {
        println!("🚨 OVERRUN DETECTED!");
        println!("  Limit: {} tokens", token_limit);
        println!("  Tracked total: {} tokens", tracked_tokens);
        println!("  Actual total: {} tokens", actual_tokens);
        println!("  Overrun amount: {} tokens", actual_tokens - token_limit);
        println!(
            "  Blocks processed: {}/{}",
            included_blocks.len(),
            blocks.len()
        );
    } else {
        println!("No overrun in this accumulation test");
    }
}

fn demonstrate_90_percent_threshold_failure() {
    println!("\n=== 90% THRESHOLD FAILURE DEMONSTRATION ===\n");

    // Create a scenario where we stay under 90% with estimation
    // but exceed 100% with actual tokens

    let blocks = vec![
        // Add normal blocks that keep us under 90% threshold
        "function normal() { return 42; }".to_string(),
        "const value = getData();".to_string(), 
        "if (condition) { doSomething(); }".to_string(),
        // Add killer blocks with very low bytes/token
        "let[a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,q,r,s,t,u,v,w,x,y,z]=[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26];".to_string(),
        "()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()=>()".to_string(),
    ];

    let token_limit = 200;

    println!("Demonstration of 90% threshold failure:");
    println!("Token limit: {}", token_limit);
    println!("90% threshold: {}", (token_limit as f64 * 0.9) as usize);

    // Show what estimation would predict
    let mut estimated_total = 0;
    let mut actual_total = 0;

    println!("\nBlock analysis:");
    for (i, block) in blocks.iter().enumerate() {
        let bytes = block.len();
        let actual_tokens = count_block_tokens(block);
        let estimated_tokens = (bytes / 4).max(1);

        estimated_total += estimated_tokens;
        actual_total += actual_tokens;

        println!(
            "  Block {}: {} bytes → {} est, {} actual",
            i + 1,
            bytes,
            estimated_tokens,
            actual_tokens
        );
        println!(
            "    Running est: {}, Running actual: {}",
            estimated_total, actual_total
        );

        if estimated_total < (token_limit as f64 * 0.9) as usize && actual_total > token_limit {
            println!("    🚨 CRITICAL: Estimation shows safe ({} < {}) but actual exceeds limit ({} > {})!",
                     estimated_total, (token_limit as f64 * 0.9) as usize, actual_total, token_limit);
        }
    }

    println!("\nThe algorithm would:");
    println!(
        "  - Continue using estimation until {} tokens",
        (token_limit as f64 * 0.9) as usize
    );
    println!("  - Never trigger precise counting if estimation stays low");
    println!("  - Silently exceed the actual token limit");

    // Now run the actual simulation
    println!("\nRunning actual simulation:");
    let (overrun, _tracked_tokens, actual_tokens, _) =
        simulate_search_limiter_logic(&blocks, token_limit);

    if overrun {
        println!(
            "🚨 Confirmed overrun: {} actual vs {} limit",
            actual_tokens, token_limit
        );
    }
}

fn main() {
    test_overrun_scenarios();
    test_realistic_overrun_scenario();
    test_edge_case_accumulation();
    demonstrate_90_percent_threshold_failure();

    println!("\n=== CONCLUSIONS ===");
    println!("1. The 4 bytes/token approximation can severely underestimate tokens");
    println!("2. Compressed code, symbols, and short identifiers cause major errors");
    println!("3. The 90% threshold is not safe - it can be reached with estimation while actual tokens exceed 100%");
    println!("4. Accumulation of small errors can lead to significant overruns");
    println!("5. Real-world code mixing different patterns makes the issue unpredictable");

    println!("\n=== RECOMMENDED FIXES ===");
    println!("1. Lower threshold to 70-80% to provide safety margin");
    println!("2. Use dynamic bytes/token ratios based on content analysis");
    println!("3. Implement progressive thresholds (start checking earlier for risky content)");
    println!("4. Add content-type detection to adjust approximation ratios");
    println!("5. Consider token counting samples from early blocks to calibrate estimation");
}
