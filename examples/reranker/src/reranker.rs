use anyhow::{Result, Context};
use candle_core::{Device, Tensor, IndexOp};
use candle_nn::{VarBuilder, Module, Linear, linear};
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::tokio::Api, Repo, RepoType};
use tokenizers::Tokenizer;
use serde_json;

pub struct BertReranker {
    bert: BertModel,
    classifier: Linear,
    tokenizer: Tokenizer,
    device: Device,
    max_length: usize,
}

impl BertReranker {
    pub async fn new(model_name: &str) -> Result<Self> {
        println!("Loading BERT model: {}", model_name);
        
        // Check if we have local model files first
        let model_dir_name = match model_name {
            "cross-encoder/ms-marco-TinyBERT-L-2-v2" => "ms-marco-TinyBERT-L-2-v2",
            "cross-encoder/ms-marco-MiniLM-L-6-v2" => "ms-marco-MiniLM-L-6-v2", 
            "cross-encoder/ms-marco-MiniLM-L-2-v2" | _ => "ms-marco-MiniLM-L-2-v2",
        };
        
        let local_model_dir = std::path::Path::new("models").join(model_dir_name);
        let (config_path, tokenizer_path, weights_path) = if local_model_dir.exists() {
            println!("Using local model files from: {:?}", local_model_dir);
            (
                local_model_dir.join("config.json"),
                local_model_dir.join("tokenizer.json"),
                local_model_dir.join("pytorch_model.bin"),
            )
        } else {
            println!("Downloading model files from HuggingFace Hub...");
            
            let api = Api::new()?;
            let repo = api.repo(Repo::with_revision(
                model_name.to_string(),
                RepoType::Model,
                "main".to_string(),
            ));

            // Download model files
            let config_path = repo.get("config.json").await
                .context("Failed to download config.json")?;
            let tokenizer_path = repo.get("tokenizer.json").await
                .context("Failed to download tokenizer.json")?;
            
            // Try different weight file formats
            let weights_path = match repo.get("model.safetensors").await {
                Ok(path) => {
                    println!("Using model.safetensors");
                    path
                },
                Err(_) => match repo.get("pytorch_model.bin").await {
                    Ok(path) => {
                        println!("Using pytorch_model.bin");
                        path
                    },
                    Err(e) => {
                        println!("Trying model.bin as fallback...");
                        repo.get("model.bin").await
                            .context(format!("Could not find model weights: {}", e))?
                    }
                }
            };
            
            (config_path, tokenizer_path, weights_path)
        };

        println!("Loading configuration...");
        // Load configuration
        let config_content = std::fs::read_to_string(&config_path)
            .context("Failed to read config file")?;
        let config: Config = serde_json::from_str(&config_content)
            .context("Failed to parse model configuration")?;

        let max_length = config.max_position_embeddings.min(512); // Limit for performance
        println!("Model config loaded - max_length: {}, hidden_size: {}", max_length, config.hidden_size);

        println!("Loading tokenizer...");
        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        println!("Setting up device and loading model weights...");
        // Setup device (CPU for compatibility, could be GPU if available)
        let device = Device::Cpu;
        let dtype = DTYPE;

        // Load model weights
        let vb = if weights_path.extension() == Some(std::ffi::OsStr::new("safetensors")) {
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], dtype, &device)? }
        } else {
            VarBuilder::from_pth(&weights_path, dtype, &device)?
        };

        println!("Creating BERT model...");
        // Create BERT model
        let bert = BertModel::load(vb.pp("bert"), &config)
            .or_else(|_| BertModel::load(vb.clone(), &config))
            .context("Failed to load BERT model")?;

        println!("Creating classification head...");
        // Create classification head (linear layer for sequence classification)
        let classifier = linear(config.hidden_size, 1, vb.pp("classifier"))
            .context("Failed to create classification head")?;

        println!("BERT reranker loaded successfully!");

        Ok(Self {
            bert,
            classifier,
            tokenizer,
            device,
            max_length,
        })
    }

    pub async fn rerank(&self, query: &str, documents: &[&str]) -> Result<Vec<(usize, f32)>> {
        let mut scores = Vec::new();

        for (idx, document) in documents.iter().enumerate() {
            let score = self.score_pair(query, document)
                .context(format!("Failed to score document {}", idx))?;
            scores.push((idx, score));
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scores)
    }

    fn score_pair(&self, query: &str, document: &str) -> Result<f32> {
        // Truncate document if too long (keep query + document under max_length)
        let max_doc_length = self.max_length.saturating_sub(query.len() / 4).saturating_sub(10); // rough estimate
        let doc_truncated = if document.len() > max_doc_length {
            &document[..max_doc_length]
        } else {
            document
        };
        
        // Prepare input text for cross-encoder (BERT format: [CLS] query [SEP] document [SEP])
        let input_text = format!("{} [SEP] {}", query, doc_truncated);
        
        // Tokenize with proper settings
        let mut encoding = self.tokenizer
            .encode(input_text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        // Truncate if too long
        if encoding.len() > self.max_length {
            encoding.truncate(self.max_length, 0, tokenizers::TruncationDirection::Right);
        }

        // Pad to max length for consistent batching
        use tokenizers::PaddingDirection;
        encoding.pad(self.max_length, 0, 0, "[PAD]", PaddingDirection::Right);

        // Convert to tensors
        let input_ids = Tensor::new(encoding.get_ids().to_vec(), &self.device)?
            .unsqueeze(0)?; // Add batch dimension [1, seq_len]
        
        let attention_mask = Tensor::new(encoding.get_attention_mask().to_vec(), &self.device)?
            .unsqueeze(0)?; // Add batch dimension [1, seq_len]

        let token_type_ids = if encoding.get_type_ids().len() > 0 {
            Some(Tensor::new(encoding.get_type_ids().to_vec(), &self.device)?.unsqueeze(0)?)
        } else {
            // Create token type ids manually: 0 for query, 1 for document
            let mut type_ids = vec![0u32; encoding.len()];
            let mut in_document = false;
            for (i, token_id) in encoding.get_ids().iter().enumerate() {
                if *token_id == 102 { // [SEP] token id (might vary by tokenizer)
                    in_document = true;
                } else if in_document {
                    type_ids[i] = 1;
                }
            }
            Some(Tensor::new(type_ids, &self.device)?.unsqueeze(0)?)
        };

        // Forward pass through BERT
        let bert_outputs = self.bert.forward(
            &input_ids,
            &attention_mask,
            token_type_ids.as_ref(),
        )?;

        // Get [CLS] token representation (first token)
        let cls_output = bert_outputs.i((.., 0, ..))?; // [batch_size, hidden_size]
        
        // Pass through classification head
        let logits = self.classifier.forward(&cls_output)?; // [batch_size, 1]
        
        // Get the relevance score
        let score = logits.i((0, 0))?.to_scalar::<f32>()?;
        
        Ok(score)
    }
}

// Demo reranker that uses simple word overlap scoring
pub struct DemoReranker;

impl DemoReranker {
    pub fn new() -> Self {
        Self
    }

    pub fn rerank(&self, query: &str, documents: &[&str]) -> Vec<(usize, f32)> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower
            .split_whitespace()
            .collect();
        
        let mut scores: Vec<(usize, f32)> = documents
            .iter()
            .enumerate()
            .map(|(idx, doc)| {
                let doc_lower = doc.to_lowercase();
                let mut score = 0.0f32;
                
                // Calculate TF-IDF-like score
                for word in &query_words {
                    if doc_lower.contains(word) {
                        // Count occurrences
                        let count = doc_lower.matches(word).count() as f32;
                        // Apply TF component (with log normalization)
                        let tf = (1.0 + count.ln()).max(0.0);
                        // Simple IDF (assume documents are diverse)
                        let idf = 2.0;
                        score += tf * idf;
                    }
                }
                
                // Normalize by document length (simple approach)
                let doc_length = doc.split_whitespace().count().max(1) as f32;
                score = score / doc_length.sqrt();
                
                (idx, score)
            })
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
    }
}