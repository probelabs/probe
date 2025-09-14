use anyhow::Result;
use clap::{Arg, Command};

/// A mock BERT reranker for demonstration purposes
/// This version doesn't require downloading models and can be used to test the interface
pub struct MockBertReranker;

impl MockBertReranker {
    /// Creates a new mock BERT reranker
    pub fn new() -> Result<Self> {
        println!("Creating mock BERT reranker (no model download required)");
        Ok(MockBertReranker)
    }

    /// Mock reranking using simple text similarity heuristics
    /// In a real implementation, this would use the BERT model
    pub fn rerank(&self, query: &str, documents: &[&str]) -> Result<Vec<RankedDocument>> {
        println!("Mock reranking {} documents for query: '{}'", documents.len(), query);

        let mut ranked_docs = Vec::new();

        for (idx, document) in documents.iter().enumerate() {
            // Simple mock scoring based on word overlap
            let score = self.compute_mock_relevance_score(query, document);
            ranked_docs.push(RankedDocument {
                index: idx,
                document: document.to_string(),
                score,
            });
        }

        // Sort by score (highest first)
        ranked_docs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        println!("Mock reranking completed");
        Ok(ranked_docs)
    }

    /// Computes a mock relevance score using simple word overlap
    fn compute_mock_relevance_score(&self, query: &str, document: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let doc_lower = document.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        let doc_words: Vec<&str> = doc_lower.split_whitespace().collect();

        let mut score = 0.0;
        for query_word in &query_words {
            for doc_word in &doc_words {
                if query_word == doc_word {
                    score += 1.0;
                } else if doc_word.contains(query_word) || query_word.contains(doc_word) {
                    score += 0.5;
                }
            }
        }

        // Add a small random component for demonstration
        score += (query.len() * document.len()) as f32 * 0.0001;
        score
    }
}

/// Represents a document with its relevance score
#[derive(Debug, Clone)]
pub struct RankedDocument {
    pub index: usize,
    pub document: String,
    pub score: f32,
}

impl std::fmt::Display for RankedDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}: {:.4} - {}", self.index + 1, self.score, self.document)
    }
}

fn main() -> Result<()> {
    let matches = Command::new("Mock BERT Reranker Demo")
        .version("0.1.0")
        .author("Code Search Team")
        .about("A mock BERT-based document reranker for testing the interface")
        .arg(
            Arg::new("query")
                .long("query")
                .short('q')
                .value_name("QUERY")
                .help("Search query")
                .required(true)
        )
        .arg(
            Arg::new("documents")
                .long("documents")
                .short('d')
                .value_name("DOCS")
                .help("Comma-separated list of documents to rerank")
                .required(false)
        )
        .arg(
            Arg::new("interactive")
                .long("interactive")
                .short('i')
                .help("Run in interactive mode")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let query = matches.get_one::<String>("query").unwrap();
    let interactive = matches.get_flag("interactive");

    println!("=== Mock BERT Reranker Demo ===");
    println!("This demo uses simple word overlap instead of a real BERT model");
    println!();

    // Initialize the mock reranker
    let reranker = MockBertReranker::new()?;

    if interactive {
        println!("=== Interactive Mode ===");
        println!("Query: {}", query);
        println!("Enter documents to rerank (one per line, empty line to finish):");

        let mut documents = Vec::new();
        loop {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim();
            if input.is_empty() {
                break;
            }
            documents.push(input.to_string());
        }

        if documents.is_empty() {
            println!("No documents provided. Exiting.");
            return Ok(());
        }

        let doc_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();
        let ranked = reranker.rerank(query, &doc_refs)?;

        println!("\n=== Reranking Results ===");
        for (rank, doc) in ranked.iter().enumerate() {
            println!("{}. {}", rank + 1, doc);
        }
    } else {
        // Use example documents or provided documents
        let documents = if let Some(docs_str) = matches.get_one::<String>("documents") {
            docs_str.split(',').collect::<Vec<&str>>()
        } else {
            // Default example documents for demonstration
            vec![
                "Rust is a systems programming language focused on safety and performance.",
                "Python is a high-level programming language known for its simplicity.",
                "Machine learning involves training algorithms on data to make predictions.",
                "BERT is a transformer-based model for natural language understanding.",
                "The Candle framework provides machine learning capabilities in Rust.",
                "Cross-encoders are used for reranking tasks in information retrieval.",
                "Tokenization is the process of breaking text into individual tokens.",
                "Neural networks consist of interconnected nodes that process information.",
            ]
        };

        println!("=== Example Usage ===");
        println!("Query: {}", query);
        println!("Documents to rerank:");
        for (i, doc) in documents.iter().enumerate() {
            println!("  {}. {}", i + 1, doc);
        }
        println!();

        // Perform mock reranking
        let ranked = reranker.rerank(query, &documents)?;

        println!("=== Mock Reranking Results ===");
        println!("Documents ranked by mock relevance to query:");
        for (rank, doc) in ranked.iter().enumerate() {
            println!("{}. {}", rank + 1, doc);
        }

        println!("\n=== Note ===");
        println!("This is a mock implementation using simple word overlap.");
        println!("The real BERT reranker in main.rs would use transformer models for much better results.");
    }

    Ok(())
}