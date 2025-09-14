use anyhow::Result;
use rust_bert::bert::{BertConfigResources, BertModelResources, BertVocabResources};
use rust_bert::pipelines::common::ModelType;
use rust_bert::pipelines::sequence_classification::{
    SequenceClassificationBuilder, SequenceClassificationConfig,
};
use rust_bert::resources::RemoteResource;

mod simple_test;

fn main() -> Result<()> {
    println!("Running rust-bert test to demonstrate cross-encoder approach...\n");

    // Note: rust-bert doesn't directly support MS-MARCO cross-encoder models
    // without conversion. This example shows the general approach using BERT-base.

    println!("IMPORTANT NOTES:");
    println!("1. rust-bert requires models in TorchScript format (.ot files)");
    println!("2. MS-MARCO models would need conversion from HuggingFace format");
    println!("3. This example uses BERT-base to demonstrate the approach");
    println!("4. For actual MS-MARCO support, models need to be converted first\n");

    // Run the simple test
    simple_test::run_simple_test()?;

    println!("\n=== Comparison with our Candle implementation ===");
    println!("Our Candle approach:");
    println!("- Directly loads PyTorch .bin files");
    println!("- Implements cross-encoder architecture manually");
    println!("- Returns raw logits for scoring");
    println!("\nrust-bert approach:");
    println!("- Requires TorchScript format");
    println!("- Provides high-level pipelines");
    println!("- Designed more for classification than regression scoring");

    Ok(())
}

fn test_with_local_model(queries: &[&str], document: &str, model_path: &PathBuf) -> Result<()> {
    println!("Loading model from local files...");

    // Create resources pointing to local files
    let model_resource = Box::new(LocalResource {
        local_path: model_path.join("rust_model.ot"),
    });

    let config_resource = Box::new(LocalResource {
        local_path: model_path.join("config.json"),
    });

    let vocab_resource = Box::new(LocalResource {
        local_path: model_path.join("vocab.txt"),
    });

    let merges_resource = Box::new(LocalResource {
        local_path: model_path.join("merges.txt"), // May not exist for BERT
    });

    // Build the model with local resources
    let model = SequenceClassificationBuilder::new()
        .with_model(ModelResource::Torch(model_resource))
        .with_config(config_resource)
        .with_vocab(vocab_resource)
        .with_merges(merges_resource)
        .with_device(tch::Device::Cpu)
        .build()?;

    run_scoring(&model, queries, document)
}

fn test_with_remote_model(queries: &[&str], document: &str) -> Result<()> {
    println!("Loading BERT base model (for demonstration)...");

    // Use rust-bert's default BERT resources
    let model = SequenceClassificationBuilder::new()
        .with_model(ModelResource::Torch(Box::new(RemoteResource::from_pretrained(
            BertModelResources::BERT_BASE_UNCASED,
        ))))
        .with_config(RemoteResource::from_pretrained(
            BertConfigResources::BERT_BASE_UNCASED,
        ))
        .with_vocab(RemoteResource::from_pretrained(
            BertVocabResources::BERT_BASE_UNCASED,
        ))
        .with_device(tch::Device::Cpu)
        .build()?;

    println!("Note: Using BERT-base instead of TinyBERT for demonstration.\n");
    run_scoring(&model, queries, document)
}

fn run_scoring(model: &SequenceClassificationModel, queries: &[&str], document: &str) -> Result<()> {
    println!("Model loaded successfully!\n");
    println!("="*80);
    println!("SCORING RESULTS");
    println!("="*80);

    let mut scores = Vec::new();

    for query in queries {
        // For cross-encoder, we need to pass query and document together
        // rust-bert expects the input as a single string
        let input = format!("{} [SEP] {}", query, document);

        // Get predictions
        let outputs = model.predict(&[input]);

        // Extract score from the first (and only) output
        if let Some(output) = outputs.first() {
            println!("\nQuery: '{}'", query);
            println!("Label: {}", output.label);
            println!("Score: {:.6}", output.score);
            println!("Confidence: {:.2}%", output.score * 100.0);

            scores.push((query, output.score));
        }
    }

    // Compare scores
    println!("\n" + &"="*80);
    println!("SCORE COMPARISON");
    println!("="*80);

    if scores.len() == 2 {
        let relevant_score = scores[0].1;
        let nonsense_score = scores[1].1;
        let difference = relevant_score - nonsense_score;

        println!("\nRelevant query ('{}'):", scores[0].0);
        println!("  Score: {:.6}", relevant_score);

        println!("\nNonsense query ('{}'):", scores[1].0);
        println!("  Score: {:.6}", nonsense_score);

        println!("\nScore difference: {:.6}", difference.abs());

        if difference > 0.1 {
            println!("✓ Good discrimination: Relevant query scores higher");
        } else if difference.abs() < 0.1 {
            println!("⚠ Poor discrimination: Scores too similar");
        } else {
            println!("❌ Wrong order: Nonsense query scores higher");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_encoder_discrimination() {
        // Test that we can create inputs correctly
        let query = "how does authentication work";
        let doc = "Authentication verifies identity";
        let input = format!("{} [SEP] {}", query, doc);

        assert!(input.contains("[SEP]"));
        assert!(input.starts_with(query));
    }
}