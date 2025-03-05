use std::sync::OnceLock;
use tiktoken_rs::{p50k_base, CoreBPE};

/// Returns a reference to the tiktoken tokenizer
pub fn get_tokenizer() -> &'static CoreBPE {
    static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
    TOKENIZER.get_or_init(|| p50k_base().expect("Failed to initialize tiktoken tokenizer"))
}

/// Helper function to count tokens in a string using tiktoken (same tokenizer as GPT models)
pub fn count_tokens(text: &str) -> usize {
    let tokenizer = get_tokenizer();
    tokenizer.encode_with_special_tokens(text).len()
}
