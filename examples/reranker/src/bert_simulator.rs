use anyhow::Result;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Simulates BERT inference performance based on real-world benchmarks
pub struct BertSimulator {
    hidden_size: usize,
    max_length: usize,
    inference_time_per_token: Duration,
    setup_overhead: Duration,
}

impl BertSimulator {
    pub fn new() -> Self {
        Self {
            hidden_size: 384, // MiniLM-L2 size
            max_length: 512,
            // Real BERT CPU inference: ~1-2ms per token for small models
            inference_time_per_token: Duration::from_micros(1500), // 1.5ms per token
            setup_overhead: Duration::from_millis(5), // 5ms overhead per document
        }
    }

    pub fn rerank(&self, query: &str, documents: &[&str]) -> Vec<(usize, f32)> {
        let mut scores = Vec::new();

        for (idx, document) in documents.iter().enumerate() {
            let score = self.score_pair(query, document);
            scores.push((idx, score));
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
    }

    fn score_pair(&self, query: &str, document: &str) -> f32 {
        // Simulate tokenization and processing time
        let token_count = self.estimate_token_count(query, document);
        let inference_time = self.setup_overhead + (self.inference_time_per_token * token_count as u32);
        
        // Actually sleep to simulate real inference time
        std::thread::sleep(inference_time);

        // Generate realistic BERT-like scores based on semantic similarity
        self.compute_semantic_score(query, document)
    }

    fn estimate_token_count(&self, query: &str, document: &str) -> usize {
        // Rough estimation: ~0.75 tokens per word + special tokens
        let word_count = query.split_whitespace().count() + document.split_whitespace().count();
        let token_count = ((word_count as f32 * 0.75) as usize + 3).min(self.max_length); // +3 for [CLS], [SEP], [SEP]
        token_count
    }

    fn compute_semantic_score(&self, query: &str, document: &str) -> f32 {
        // Simulate BERT cross-encoder output by computing more sophisticated similarity
        let query_lower = query.to_lowercase();
        let doc_lower = document.to_lowercase();
        
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let doc_words: Vec<&str> = doc_lower.split_whitespace().collect();
        
        if query_words.is_empty() || doc_words.is_empty() {
            return 0.0;
        }

        // 1. Exact word matches (high weight)
        let mut exact_matches = 0.0;
        for q_word in &query_words {
            if doc_words.contains(q_word) {
                exact_matches += 1.0;
            }
        }
        let exact_match_score = exact_matches / query_words.len() as f32;

        // 2. Substring/partial matches (medium weight)
        let mut partial_matches = 0.0;
        for q_word in &query_words {
            for d_word in &doc_words {
                if q_word.len() > 3 && d_word.contains(q_word) {
                    partial_matches += 0.5;
                } else if d_word.len() > 3 && q_word.contains(d_word) {
                    partial_matches += 0.3;
                }
            }
        }
        let partial_match_score = (partial_matches / query_words.len() as f32).min(1.0);

        // 3. Programming-specific keyword matching
        let prog_keywords = self.get_programming_keywords();
        let mut prog_score = 0.0;
        for q_word in &query_words {
            if prog_keywords.contains_key(q_word) {
                for d_word in &doc_words {
                    if let Some(related_words) = prog_keywords.get(q_word) {
                        if related_words.contains(d_word) {
                            prog_score += 0.8;
                        }
                    }
                }
            }
        }
        let prog_match_score = (prog_score / query_words.len() as f32).min(1.0);

        // 4. Document length normalization (BERT tends to favor mid-length docs)
        let doc_length = doc_words.len() as f32;
        let length_penalty = if doc_length < 10.0 {
            0.8 // Too short
        } else if doc_length > 200.0 {
            0.9 // Too long
        } else {
            1.0 // Good length
        };

        // Combine scores with weights that simulate BERT's behavior
        let final_score = (exact_match_score * 3.0 + 
                          partial_match_score * 2.0 + 
                          prog_match_score * 1.5) * length_penalty;

        // Add some realistic noise and transform to BERT-like logit range
        let noise = (rand::random() - 0.5) * 0.2; // Small random noise
        let logit_score = (final_score * 2.0 - 1.0) + noise; // Transform to roughly [-1, 5] range

        logit_score
    }

    fn get_programming_keywords(&self) -> HashMap<&'static str, Vec<&'static str>> {
        let mut keywords = HashMap::new();
        
        keywords.insert("rust", vec!["cargo", "rustc", "trait", "impl", "struct", "enum", "match", "ownership", "borrow"]);
        keywords.insert("async", vec!["await", "future", "tokio", "task", "runtime", "executor"]);
        keywords.insert("search", vec!["index", "query", "algorithm", "tree", "hash", "lookup", "find"]);
        keywords.insert("algorithm", vec!["sort", "tree", "graph", "hash", "binary", "linear", "complexity"]);
        keywords.insert("performance", vec!["optimize", "benchmark", "profile", "speed", "memory", "cache"]);
        keywords.insert("machine", vec!["learning", "model", "neural", "training", "inference", "ai"]);
        keywords.insert("vector", vec!["embedding", "similarity", "distance", "cosine", "dot", "product"]);
        keywords.insert("neural", vec!["network", "transformer", "bert", "attention", "layer", "weight"]);
        keywords.insert("database", vec!["sql", "index", "table", "query", "schema", "transaction"]);
        keywords.insert("api", vec!["rest", "http", "endpoint", "request", "response", "server"]);
        
        keywords
    }
}

impl Default for BertSimulator {
    fn default() -> Self {
        Self::new()
    }
}

// Performance characteristics based on real BERT benchmarks
pub struct BertPerformanceStats {
    pub model_name: String,
    pub avg_inference_time_ms: f64,
    pub tokens_per_second: f64,
    pub docs_per_second: f64,
    pub memory_usage_mb: f64,
}

impl BertPerformanceStats {
    pub fn minilm_l2_cpu() -> Self {
        Self {
            model_name: "ms-marco-MiniLM-L-2-v2".to_string(),
            avg_inference_time_ms: 45.0, // ~45ms per document pair on CPU
            tokens_per_second: 850.0,    // ~850 tokens/sec on modern CPU
            docs_per_second: 22.0,       // ~22 documents/sec (assuming avg 512 tokens)
            memory_usage_mb: 45.0,       // ~45MB model size
        }
    }

    pub fn minilm_l6_cpu() -> Self {
        Self {
            model_name: "ms-marco-MiniLM-L-6-v2".to_string(),
            avg_inference_time_ms: 85.0, // ~85ms per document pair on CPU
            tokens_per_second: 450.0,    // ~450 tokens/sec on modern CPU
            docs_per_second: 12.0,       // ~12 documents/sec
            memory_usage_mb: 90.0,       // ~90MB model size
        }
    }

    pub fn print_comparison(&self) {
        println!("\n🤖 BERT MODEL PERFORMANCE CHARACTERISTICS");
        println!("==========================================");
        println!("Model: {}", self.model_name);
        println!("Average inference time: {:.1}ms per document", self.avg_inference_time_ms);
        println!("Processing speed: {:.1} tokens/second", self.tokens_per_second);
        println!("Document throughput: {:.1} docs/second", self.docs_per_second);
        println!("Memory usage: {:.1} MB", self.memory_usage_mb);
        println!("==========================================");
    }
}

// Random number generation for noise
mod rand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn random() -> f32 {
        let mut hasher = DefaultHasher::new();
        let time_nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        time_nanos.hash(&mut hasher);
        let hash = hasher.finish();
        (hash as f32) / (u64::MAX as f32)
    }
}