use anyhow::{anyhow, Result};
use rig::{
    agent::Agent,
    completion::Prompt,
    providers::{
        anthropic::{completion::CompletionModel, CLAUDE_3_5_SONNET},
        openai::GPT_4O,
    },
};

// Enum to represent the different model types we support
#[allow(dead_code)]
pub enum ModelType {
    Anthropic(Agent<CompletionModel>),
    OpenAI(Agent<rig::providers::openai::CompletionModel>),
}

// Add a method to ModelType to handle prompting for both model types
impl ModelType {
    pub async fn prompt(
        &self,
        prompt_text: String,
    ) -> Result<String, rig::completion::PromptError> {
        match self {
            ModelType::Anthropic(agent) => agent.prompt(prompt_text).await,
            ModelType::OpenAI(agent) => {
                // For OpenAI, we need to handle the prompt differently
                // This is to ensure that tool outputs are properly processed
                let result = agent.prompt(prompt_text).await?;

                // Debug output to help diagnose issues
                if std::env::var("DEBUG").unwrap_or_default() != "" {
                    println!("[DEBUG] OpenAI raw response: {}", result);
                }

                Ok(result)
            }
        }
    }
}

// Function to create a model based on environment variables
#[allow(dead_code)]
pub fn create_model(preamble: &str) -> Result<ModelType> {
    // Check for environment variables to determine which API to use
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let openai_key = std::env::var("OPENAI_API_KEY").ok();

    // Get model override if provided
    let model_override = std::env::var("MODEL_NAME").ok();

    // Get API URL overrides if provided
    let anthropic_api_url = std::env::var("ANTHROPIC_API_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let openai_api_url =
        std::env::var("OPENAI_API_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

    // Debug mode check
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() != "";

    if debug_mode {
        println!("[DEBUG] API URLs:");
        println!("[DEBUG] - Anthropic: {}", anthropic_api_url);
        println!("[DEBUG] - OpenAI: {}", openai_api_url);
    }

    // Create the appropriate chat based on available API keys
    let model_type = if let Some(key) = anthropic_key {
        if debug_mode {
            println!("[DEBUG] Using Anthropic API");
        }

        let client = rig::providers::anthropic::ClientBuilder::new(&key)
            .anthropic_version(rig::providers::anthropic::ANTHROPIC_VERSION_LATEST)
            .anthropic_beta("prompt-caching-2024-07-31")
            .base_url(&anthropic_api_url)
            .build();

        if debug_mode {
            println!("[DEBUG] Using Anthropic API URL: {}", anthropic_api_url);
        }

        // Use model override if provided, otherwise use default
        let model = if let Some(model_name) = &model_override {
            if debug_mode {
                println!("[DEBUG] Using custom Anthropic model: {}", model_name);
            }
            model_name.as_str()
        } else {
            CLAUDE_3_5_SONNET
        };

        let anthropic_chat = client
            .agent(model)
            .preamble(preamble)
            .max_tokens(8096)
            .temperature(0.1)
            .tool(super::tools::ProbeSearch)
            .tool(super::tools::AstGrepQuery)
            .tool(super::tools::Extract)
            .build();

        ModelType::Anthropic(anthropic_chat)
    } else if let Some(key) = openai_key {
        if debug_mode {
            println!("[DEBUG] Using OpenAI API");
        }

        // For OpenAI, use from_url method instead of ClientBuilder
        let client = rig::providers::openai::Client::from_url(&key, &openai_api_url.to_string());

        if debug_mode {
            println!("[DEBUG] Using OpenAI API URL: {}", openai_api_url);
        }

        // Use model override if provided, otherwise use default
        let model = if let Some(model_name) = &model_override {
            if debug_mode {
                println!("[DEBUG] Using custom OpenAI model: {}", model_name);
            }
            model_name.as_str()
        } else {
            GPT_4O
        };

        // For OpenAI, configure the chat
        let openai_chat = client
            .agent(model)
            .preamble(preamble)
            .max_tokens(8096) // OpenAI typically has lower token limits
            .temperature(0.1)
            .tool(super::tools::ProbeSearch)
            .tool(super::tools::AstGrepQuery)
            .tool(super::tools::Extract)
            .build();

        if debug_mode {
            println!("[DEBUG] OpenAI chat configured with default settings");
        }

        ModelType::OpenAI(openai_chat)
    } else {
        // No API keys found, print helpful message and exit
        return Err(anyhow!("No API keys found. Please set either ANTHROPIC_API_KEY or OPENAI_API_KEY environment variable."));
    };

    Ok(model_type)
}
