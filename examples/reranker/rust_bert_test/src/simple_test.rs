use anyhow::Result;
use rust_bert::bert::{BertConfigResources, BertModelResources, BertVocabResources};
use rust_bert::pipelines::common::ModelType;
use rust_bert::pipelines::sequence_classification::{
    Label, SequenceClassificationBuilder, SequenceClassificationConfig,
};
use rust_bert::resources::RemoteResource;

/// This is a simplified test that uses BERT-base to demonstrate the approach
/// since we can't directly use MS-MARCO models without conversion
pub fn run_simple_test() -> Result<()> {
    println!("=== Rust-BERT Simple Cross-Encoder Style Test ===\n");

    // Create configuration for sequence classification
    // Note: This uses BERT-base, not TinyBERT
    let config = SequenceClassificationConfig {
        model_type: ModelType::Bert,
        model_resource: RemoteResource::from_pretrained(BertModelResources::BERT_BASE_UNCASED)
            .into(),
        config_resource: RemoteResource::from_pretrained(BertConfigResources::BERT_BASE_UNCASED)
            .into(),
        vocab_resource: RemoteResource::from_pretrained(BertVocabResources::BERT_BASE_UNCASED)
            .into(),
        ..Default::default()
    };

    println!("Loading BERT model (this will download if not cached)...");
    let model = SequenceClassificationBuilder::from_config(&config).build()?;
    println!("Model loaded!\n");

    // Test queries and document
    let queries = vec![
        "how does authentication work",
        "foobar random nonsense gibberish",
    ];

    let document = "Authentication is the process of verifying the identity of a user.";

    println!("Document: '{}'\n", document);
    println!("Testing queries:");

    // Process each query-document pair
    let mut results = Vec::new();

    for query in &queries {
        // Format as BERT expects: [CLS] text [SEP]
        // For cross-encoder style, concatenate query and document
        let input = format!("{} {}", query, document);

        println!("\nProcessing: '{}'", query);

        // Get prediction
        let output = model.predict(&[&input]);

        if let Some(prediction) = output.first() {
            println!("  Prediction: {:?}", prediction);
            results.push((query.as_str(), prediction.clone()));
        }
    }

    // Analyze results
    println!("\n" + &"=" * 60);
    println!("ANALYSIS");
    println!(&"=" * 60);

    if results.len() == 2 {
        let (q1, label1) = &results[0];
        let (q2, label2) = &results[1];

        println!(
            "\nQuery 1: '{}' -> Label: {}, Score: {:.4}",
            q1, label1.label, label1.score
        );
        println!(
            "Query 2: '{}' -> Label: {}, Score: {:.4}",
            q2, label2.label, label2.score
        );

        // Note: For classification models, the score is a probability
        // and the label indicates the class (e.g., "POSITIVE", "NEGATIVE")
        println!("\nNote: BERT-base is a masked LM, not trained for sequence classification.");
        println!("For proper cross-encoder scoring, use a model fine-tuned on MS-MARCO.");
    }

    Ok(())
}

#[allow(dead_code)]
fn main() -> Result<()> {
    run_simple_test()
}
