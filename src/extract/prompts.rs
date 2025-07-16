//! Prompt templates for LLM models.
//!
//! This module provides functionality for loading and formatting prompt templates
//! for use with LLM models. It supports built-in templates and loading from files.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Built-in engineer prompt template
pub const ENGINEER_PROMPT: &str = r#"As a senior software engineer, your task is providing explicit, actionable code adjustments. For each required change:

1. Clearly indicate:
   - File path and name.
   - Function or class to modify.
   - Type of modification: add, modify, or remove.

2. Provide complete code blocks only for:
   - Newly created functions or methods.
   - Entire modified functions.
   - Updated class definitions.
   - Adjusted configuration segments.

3. Structure responses precisely as follows:

   File: path/filename.ext  
   Change: Concise description of change made  
   ```language  
   [Complete code block of the specified change] "#;

/// Built-in architect prompt template
pub const ARCHITECT_PROMPT: &str = r#"You are a senior software architect with expertise in designing code structures and planning implementations. Your responsibilities include:

1. Analyzing requested modifications, clearly identifying actionable steps.
2. Preparing a thorough implementation plan, detailing:
   - Files requiring modification.
   - Specific code segments to update.
   - New functions, classes, or methods to introduce.
   - Dependencies and import statements to revise.
   - Adjustments to data structures.
   - Changes in interfaces.
   - Updates needed in configuration files.

For every change, explicitly state:
- Exact location within the codebase.
- Reasoning and logic behind each decision.
- Example code signatures with parameters and return types.
- Possible side effects or impacts on existing code.
- Essential architectural choices that must be addressed.

Brief code snippets may be included for clarity, but do not produce a full implementation.

Your analysis should strictly cover the technical implementation plan, excluding deployment, testing, or validation unless explicitly tied to architectural impact."#;

/// Enum representing different prompt template sources
#[derive(Debug, Clone)]
pub enum PromptTemplate {
    /// Built-in engineer template
    Engineer,
    /// Built-in architect template
    Architect,
    /// Custom template loaded from a file
    Custom(String),
}

impl PromptTemplate {
    /// Parse a prompt template string into a PromptTemplate enum
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(template_str: &str) -> Result<Self> {
        match template_str.to_lowercase().as_str() {
            "engineer" => Ok(PromptTemplate::Engineer),
            "architect" => Ok(PromptTemplate::Architect),
            path => {
                // Check if the string is a valid file path
                let path_obj = Path::new(path);
                if path_obj.exists() && path_obj.is_file() {
                    Ok(PromptTemplate::Custom(path.to_string()))
                } else {
                    Err(anyhow::anyhow!(
                        "Invalid prompt template: '{}'. Use 'engineer', 'architect', or a valid file path.",
                        template_str
                    ))
                }
            }
        }
    }

    /// Get the content of the prompt template
    pub fn get_content(&self) -> Result<String> {
        match self {
            PromptTemplate::Engineer => Ok(ENGINEER_PROMPT.to_string()),
            PromptTemplate::Architect => Ok(ARCHITECT_PROMPT.to_string()),
            PromptTemplate::Custom(path) => fs::read_to_string(path)
                .with_context(|| format!("Failed to read prompt file: {path}")),
        }
    }
}
