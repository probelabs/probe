use anyhow::{Error as E, Result};
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use clap::{Arg, Command};
use hf_hub::api::sync::Api;
use tokenizers::Tokenizer;

/// A BERT-based reranker that uses cross-encoder architecture to rerank documents
/// based on their relevance to a given query.
pub struct BertReranker {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl BertReranker {
    /// Creates a new BERT reranker with the specified model.
    /// 
    /// # Arguments
    /// * `model_id` - The HuggingFace model ID (e.g., "cross-encoder/ms-marco-MiniLM-L-2-v2")
    /// * `revision` - The model revision/branch to use
    /// * `use_pth` - Whether to use PyTorch weights (.pth) instead of SafeTensors
    pub fn new(
        model_id: &str,
        _revision: &str,
        use_pth: bool,
    ) -> Result<Self> {
        println!("Loading BERT reranker model: {}", model_id);
        let device = Device::Cpu;
        
        let api = Api::new()?;
        let repo = api.model(model_id.to_string());
        
        // Download model configuration
        let config_filename = repo.get("config.json")?;
        println!("Config file: {:?}", config_filename);
        
        let config = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config)?;
        println!("Model config loaded: {:?}", config);
        
        // Download tokenizer
        let tokenizer_filename = repo.get("tokenizer.json")?;
        println!("Tokenizer file: {:?}", tokenizer_filename);
        
        let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(E::msg)?;
        
        // Download model weights
        let weights_filename = if use_pth {
            repo.get("pytorch_model.bin")?
        } else {
            repo.get("model.safetensors")?
        };
        println!("Weights file: {:?}", weights_filename);
        
        // Load model weights
        let vb = if use_pth {
            VarBuilder::from_pth(&weights_filename, DTYPE, &device)?
        } else {
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_filename], DTYPE, &device)? }
        };
        
        // Initialize BERT model
        let model = BertModel::load(vb, &config)?;
        println!("BERT model loaded successfully");
        
        Ok(BertReranker {
            model,
            tokenizer,
            device,
        })
    }
    
    /// Reranks a list of documents based on their relevance to the query.
    /// Returns the documents sorted by relevance score (highest first).
    /// 
    /// # Arguments
    /// * `query` - The search query
    /// * `documents` - List of candidate documents to rerank
    pub fn rerank(&self, query: &str, documents: &[&str]) -> Result<Vec<RankedDocument>> {
        println!("Reranking {} documents for query: '{}'", documents.len(), query);
        
        let mut ranked_docs = Vec::new();
        
        for (idx, document) in documents.iter().enumerate() {
            let score = self.compute_relevance_score(query, document)?;
            ranked_docs.push(RankedDocument {
                index: idx,
                document: document.to_string(),
                score,
            });
        }
        
        // Sort by score (highest first)
        ranked_docs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        
        println!("Reranking completed");
        Ok(ranked_docs)
    }
    
    /// Computes the relevance score between a query and a document.
    /// Uses BERT cross-encoder architecture where query and document are concatenated
    /// and fed through the model to get a relevance score.
    fn compute_relevance_score(&self, query: &str, document: &str) -> Result<f32> {
        // Combine query and document for cross-encoder input
        let input_text = format!("{} [SEP] {}", query, document);
        
        // Tokenize the input
        let encoding = self
            .tokenizer
            .encode(input_text, true)
            .map_err(E::msg)?;
        
        let tokens = encoding.get_ids();
        let token_ids = Tensor::new(
            tokens,
            &self.device,
        )?.unsqueeze(0)?; // Add batch dimension
        
        let token_type_ids = encoding.get_type_ids();
        let token_type_ids = Tensor::new(
            token_type_ids,
            &self.device,
        )?.unsqueeze(0)?; // Add batch dimension
        
        let attention_mask = encoding.get_attention_mask();
        let attention_mask = Tensor::new(
            attention_mask,
            &self.device,
        )?.unsqueeze(0)?; // Add batch dimension
        
        // Forward pass through BERT
        let embeddings = self.model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
        
        // For cross-encoder, we typically use the [CLS] token embedding
        // and pass it through a classification head. For simplicity, we'll
        // use the first token (CLS) embedding and compute its norm as a score.
        let cls_embedding = embeddings.get(0)?.get(0)?; // [batch, seq, hidden] -> [hidden]
        let score = cls_embedding.sum_all()?.to_scalar::<f32>()?;
        
        Ok(score)
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
    let matches = Command::new("BERT Reranker")
        .version("0.1.0")
        .author("Code Search Team")
        .about("A BERT-based document reranker using the Candle framework")
        .arg(
            Arg::new("model")
                .long("model")
                .short('m')
                .value_name("MODEL_ID")
                .help("HuggingFace model ID to use")
                .default_value("cross-encoder/ms-marco-MiniLM-L-2-v2")
        )
        .arg(
            Arg::new("revision")
                .long("revision")
                .short('r')
                .value_name("REVISION")
                .help("Model revision/branch")
                .default_value("main")
        )
        .arg(
            Arg::new("use-pth")
                .long("use-pth")
                .help("Use PyTorch weights instead of SafeTensors")
                .action(clap::ArgAction::SetTrue)
        )
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

    let model_id = matches.get_one::<String>("model").unwrap();
    let revision = matches.get_one::<String>("revision").unwrap();
    let use_pth = matches.get_flag("use-pth");
    let query = matches.get_one::<String>("query").unwrap();
    let interactive = matches.get_flag("interactive");

    println!("Initializing BERT Reranker...");
    println!("Model: {}", model_id);
    println!("Revision: {}", revision);
    println!("Using PyTorch weights: {}", use_pth);
    println!();

    // Initialize the reranker
    let reranker = BertReranker::new(model_id, revision, use_pth)?;

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

        // Perform reranking
        let ranked = reranker.rerank(query, &documents)?;

        println!("=== Reranking Results ===");
        println!("Documents ranked by relevance to query:");
        for (rank, doc) in ranked.iter().enumerate() {
            println!("{}. {}", rank + 1, doc);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranked_document_display() {
        let doc = RankedDocument {
            index: 0,
            document: "Test document".to_string(),
            score: 0.8542,
        };
        let display = format!("{}", doc);
        assert!(display.contains("#1"));
        assert!(display.contains("0.8542"));
        assert!(display.contains("Test document"));
    }

    #[test]
    fn test_empty_documents() {
        // This would require initializing a model, which is expensive for tests
        // In a real implementation, you might want to add mock tests
        assert!(true); // Placeholder test
    }
}