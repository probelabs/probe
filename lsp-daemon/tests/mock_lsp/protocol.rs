//! LSP JSON-RPC protocol definitions for mock server
//!
//! This module defines the basic LSP protocol structures needed
//! for the mock server implementation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// LSP JSON-RPC request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// LSP JSON-RPC response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LspError>,
}

/// LSP JSON-RPC notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
}

/// LSP error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// LSP Initialize request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub process_id: Option<u32>,
    pub root_path: Option<String>,
    pub root_uri: Option<String>,
    pub initialization_options: Option<Value>,
    pub capabilities: ClientCapabilities,
    pub trace: Option<String>,
    pub workspace_folders: Option<Vec<WorkspaceFolder>>,
}

/// Client capabilities for initialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document: Option<TextDocumentClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub general: Option<GeneralClientCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_edit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_edit: Option<WorkspaceEditCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did_change_configuration: Option<DynamicRegistrationCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did_change_watched_files: Option<DynamicRegistrationCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<WorkspaceSymbolCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execute_command: Option<DynamicRegistrationCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synchronization: Option<TextDocumentSyncCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<CompletionCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<HoverCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_help: Option<SignatureHelpCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declaration: Option<GotoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition: Option<GotoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_definition: Option<GotoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation: Option<GotoCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<ReferenceCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_highlight: Option<DocumentHighlightCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_symbol: Option<DocumentSymbolCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_action: Option<CodeActionCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_lens: Option<CodeLensCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_link: Option<DocumentLinkCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_provider: Option<DocumentColorCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatting: Option<DocumentFormattingCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_formatting: Option<DocumentRangeFormattingCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_type_formatting: Option<DocumentOnTypeFormattingCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename: Option<RenameCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_diagnostics: Option<PublishDiagnosticsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folding_range: Option<FoldingRangeCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_range: Option<SelectionRangeCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_hierarchy: Option<CallHierarchyCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_done_progress: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_message: Option<ShowMessageRequestCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_document: Option<ShowDocumentCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeneralClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regular_expressions: Option<RegularExpressionsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<MarkdownCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_request_support: Option<StaleRequestSupportCapability>,
}

/// Workspace folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
}

/// Initialize result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
}

/// Server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Server capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_document_sync: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_help_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declaration_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_definition_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_highlight_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_symbol_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_action_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_lens_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_link_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_formatting_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_range_formatting_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_on_type_formatting_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rename_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folding_range_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execute_command_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_range_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_hierarchy_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_tokens_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_symbol_provider: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,
}

// Capability structures (simplified for mock purposes)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DynamicRegistrationCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceEditCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_changes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_operations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_handling: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSymbolCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextDocumentSyncCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_save: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will_save_wait_until: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did_save: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompletionCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_item_kind: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HoverCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_format: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignatureHelpCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_information: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GotoCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReferenceCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentHighlightCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentSymbolCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_kind: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchical_document_symbol_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeActionCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_action_literal_support: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_preferred_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeLensCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentLinkCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentColorCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentFormattingCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentRangeFormattingCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentOnTypeFormattingCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RenameCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepare_support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PublishDiagnosticsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_information: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_support: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_support: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FoldingRangeCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_folding_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectionRangeCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CallHierarchyCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_registration: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShowMessageRequestCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_action_item: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShowDocumentCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegularExpressionsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarkdownCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StaleRequestSupportCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_on_content_modified: Option<Vec<String>>,
}

/// Helper function to create a default initialize result
pub fn default_initialize_result(server_name: &str) -> InitializeResult {
    let mut capabilities = ServerCapabilities::default();
    capabilities.text_document_sync = Some(serde_json::json!(1));
    capabilities.hover_provider = Some(serde_json::json!(true));
    capabilities.definition_provider = Some(serde_json::json!(true));
    capabilities.references_provider = Some(serde_json::json!(true));
    capabilities.document_symbol_provider = Some(serde_json::json!(true));
    capabilities.workspace_symbol_provider = Some(serde_json::json!(true));
    capabilities.call_hierarchy_provider = Some(serde_json::json!(true));
    capabilities.completion_provider = Some(serde_json::json!({}));

    InitializeResult {
        capabilities,
        server_info: Some(ServerInfo {
            name: server_name.to_string(),
            version: Some("mock-0.1.0".to_string()),
        }),
    }
}
