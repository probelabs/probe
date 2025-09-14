use anyhow::{Result, Context};
use candle_core::{Device, Tensor, IndexOp};
use candle_nn::{VarBuilder, Module, Linear, linear};
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::tokio::Api, Repo, RepoType};
use tokenizers::Tokenizer;
use serde_json;
use rayon::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;

/// Thread-safe wrapper for BERT components
pub struct BertInferenceEngine {
    bert: BertModel,
    classifier: Linear,
    tokenizer: Tokenizer,
    device: Device,
    max_length: usize,
}

unsafe impl Send for BertInferenceEngine {}
unsafe impl Sync for BertInferenceEngine {}

pub struct ParallelBertReranker {
    engines: Vec<Arc<Mutex<BertInferenceEngine>>>,
    num_threads: usize,
}

impl ParallelBertReranker {
    pub async fn new(model_name: &str, num_threads: Option<usize>) -> Result<Self> {
        let num_threads = num_threads.unwrap_or_else(|| {
            let cores = thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4);
            println!("Auto-detected {} CPU cores", cores);
            cores
        });

        println!("Creating parallel BERT reranker with {} threads", num_threads);

        // Load model configuration and weights once
        let (config, tokenizer_data, vb_data) = Self::load_model_data(model_name).await?;

        // Create multiple inference engines (one per thread)
        let mut engines = Vec::new();
        for i in 0..num_threads {
            println!("Initializing inference engine {}/{}", i + 1, num_threads);
            let engine = Self::create_inference_engine(&config, &tokenizer_data, &vb_data)?;
            engines.push(Arc::new(Mutex::new(engine)));
        }

        // Configure Rayon thread pool
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build_global()
            .context("Failed to configure thread pool")?;

        println!("Parallel BERT reranker initialized with {} engines", num_threads);

        Ok(Self {
            engines,
            num_threads,
        })
    }

    async fn load_model_data(model_name: &str) -> Result<(Config, Vec<u8>, Vec<u8>)> {
        println!("Loading model data for: {}", model_name);

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

            let config_path = repo.get("config.json").await?;
            let tokenizer_path = repo.get("tokenizer.json").await?;
            let weights_path = match repo.get("pytorch_model.bin").await {
                Ok(path) => path,
                Err(_) => repo.get("model.safetensors").await?,
            };

            (config_path, tokenizer_path, weights_path)
        };

        // Load config
        let config_content = std::fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&config_content)?;

        // Load tokenizer data
        let tokenizer_data = std::fs::read(&tokenizer_path)?;

        // Load model weights data
        let weights_data = std::fs::read(&weights_path)?;

        println!("Model data loaded - config: {} bytes, tokenizer: {} bytes, weights: {} bytes",
                 config_content.len(), tokenizer_data.len(), weights_data.len());

        Ok((config, tokenizer_data, weights_data))
    }

    fn create_inference_engine(
        config: &Config,
        tokenizer_data: &[u8],
        weights_data: &[u8]
    ) -> Result<BertInferenceEngine> {
        let device = Device::Cpu;
        let dtype = DTYPE;
        let max_length = config.max_position_embeddings.min(512);

        // Create tokenizer from data
        let tokenizer = Tokenizer::from_bytes(tokenizer_data)
            .map_err(|e| anyhow::anyhow!("Failed to create tokenizer: {}", e))?;

        // Create VarBuilder from weights data
        let temp_file = tempfile::NamedTempFile::new()?;
        std::fs::write(temp_file.path(), weights_data)?;

        let vb = VarBuilder::from_pth(temp_file.path(), dtype, &device)
            .context("Failed to load model weights")?;

        // Create BERT model
        let bert = BertModel::load(vb.pp("bert"), config)
            .or_else(|_| BertModel::load(vb.clone(), config))?;

        // Create classification head
        let classifier = linear(config.hidden_size, 1, vb.pp("classifier"))?;

        Ok(BertInferenceEngine {
            bert,
            classifier,
            tokenizer,
            device,
            max_length,
        })
    }

    pub fn rerank_parallel(&self, query: &str, documents: &[&str]) -> Result<Vec<(usize, f32)>> {
        println!("Processing {} documents in parallel across {} threads",
                 documents.len(), self.num_threads);

        // Create chunks for parallel processing
        let chunk_size = (documents.len() + self.num_threads - 1) / self.num_threads;
        let chunks: Vec<_> = documents
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        println!("Created {} chunks of size ~{}", chunks.len(), chunk_size);

        // Process chunks in parallel
        let query = query.to_string(); // Clone for thread safety
        let engines = Arc::new(&self.engines);

        let results: Result<Vec<Vec<(usize, f32)>>> = chunks
            .into_par_iter()
            .enumerate()
            .map(|(chunk_idx, chunk)| {
                let engine_idx = chunk_idx % self.engines.len();
                let engine = &engines[engine_idx];

                println!("Thread {} processing chunk {} with {} documents",
                         chunk_idx, chunk_idx, chunk.len());

                let mut chunk_results = Vec::new();

                // Lock the engine for this thread
                let engine_guard = engine.lock();

                for (doc_idx, document) in chunk {
                    let score = Self::score_pair_with_engine(&engine_guard, &query, document)
                        .with_context(|| format!("Failed to score document {}", doc_idx))?;
                    chunk_results.push((doc_idx, score));
                }

                drop(engine_guard); // Explicitly release lock

                Ok(chunk_results)
            })
            .collect();

        // Flatten results and sort
        let mut all_scores: Vec<(usize, f32)> = results?
            .into_iter()
            .flatten()
            .collect();

        // Sort by score descending
        all_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        println!("Parallel processing complete, {} results sorted", all_scores.len());

        Ok(all_scores)
    }

    pub fn rerank_sequential(&self, query: &str, documents: &[&str]) -> Result<Vec<(usize, f32)>> {
        println!("Processing {} documents sequentially", documents.len());

        let mut scores = Vec::new();
        let engine = &self.engines[0]; // Use first engine for sequential processing
        let engine_guard = engine.lock();

        for (idx, document) in documents.iter().enumerate() {
            let score = Self::score_pair_with_engine(&engine_guard, query, document)
                .with_context(|| format!("Failed to score document {}", idx))?;
            scores.push((idx, score));
        }

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scores)
    }

    fn score_pair_with_engine(
        engine: &BertInferenceEngine,
        query: &str,
        document: &str
    ) -> Result<f32> {
        // Truncate document if too long (safe Unicode truncation)
        let max_doc_length = engine.max_length.saturating_sub(query.len() / 4).saturating_sub(10);
        let doc_truncated = if document.len() > max_doc_length {
            // Find a safe Unicode boundary
            let mut boundary = max_doc_length;
            while boundary > 0 && !document.is_char_boundary(boundary) {
                boundary -= 1;
            }
            &document[..boundary]
        } else {
            document
        };

        // Prepare input text for cross-encoder
        let input_text = format!("{} [SEP] {}", query, doc_truncated);

        // Tokenize
        let mut encoding = engine.tokenizer
            .encode(input_text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        // Truncate if too long
        if encoding.len() > engine.max_length {
            encoding.truncate(engine.max_length, 0, tokenizers::TruncationDirection::Right);
        }

        // Pad to max length
        use tokenizers::PaddingDirection;
        encoding.pad(engine.max_length, 0, 0, "[PAD]", PaddingDirection::Right);

        // Convert to tensors
        let input_ids = Tensor::new(encoding.get_ids().to_vec(), &engine.device)?
            .unsqueeze(0)?;

        let attention_mask = Tensor::new(encoding.get_attention_mask().to_vec(), &engine.device)?
            .unsqueeze(0)?;

        let token_type_ids = if encoding.get_type_ids().len() > 0 {
            Some(Tensor::new(encoding.get_type_ids().to_vec(), &engine.device)?.unsqueeze(0)?)
        } else {
            // Create token type ids manually
            let mut type_ids = vec![0u32; encoding.len()];
            let mut in_document = false;
            for (i, token_id) in encoding.get_ids().iter().enumerate() {
                if *token_id == 102 { // [SEP] token
                    in_document = true;
                } else if in_document {
                    type_ids[i] = 1;
                }
            }
            Some(Tensor::new(type_ids, &engine.device)?.unsqueeze(0)?)
        };

        // Forward pass through BERT
        let bert_outputs = engine.bert.forward(
            &input_ids,
            &attention_mask,
            token_type_ids.as_ref(),
        )?;

        // Get [CLS] token representation
        let cls_output = bert_outputs.i((.., 0, ..))?;

        // Pass through classification head
        let logits = engine.classifier.forward(&cls_output)?;

        // Get the relevance score
        let score = logits.i((0, 0))?.to_scalar::<f32>()?;

        Ok(score)
    }

    pub fn get_num_threads(&self) -> usize {
        self.num_threads
    }
}