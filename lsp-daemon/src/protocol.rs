use crate::language_detector::Language;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonRequest {
    Connect {
        client_id: Uuid,
    },
    // Workspace management
    InitializeWorkspace {
        request_id: Uuid,
        workspace_root: PathBuf,
        language: Option<Language>,
    },
    ListWorkspaces {
        request_id: Uuid,
    },
    // Analysis requests with optional workspace hints
    CallHierarchy {
        request_id: Uuid,
        file_path: PathBuf,
        pattern: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Definition {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    References {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        include_declaration: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Hover {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Completion {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    // System requests
    Status {
        request_id: Uuid,
    },
    ListLanguages {
        request_id: Uuid,
    },
    Shutdown {
        request_id: Uuid,
    },
    Ping {
        request_id: Uuid,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonResponse {
    Connected {
        request_id: Uuid,
        daemon_version: String,
    },
    // Workspace responses
    WorkspaceInitialized {
        request_id: Uuid,
        workspace_root: PathBuf,
        language: Language,
        lsp_server: String,
    },
    WorkspaceList {
        request_id: Uuid,
        workspaces: Vec<WorkspaceInfo>,
    },
    // Analysis responses
    CallHierarchy {
        request_id: Uuid,
        result: CallHierarchyResult,
    },
    Definition {
        request_id: Uuid,
        locations: Vec<Location>,
    },
    References {
        request_id: Uuid,
        locations: Vec<Location>,
    },
    Hover {
        request_id: Uuid,
        content: Option<HoverContent>,
    },
    Completion {
        request_id: Uuid,
        items: Vec<CompletionItem>,
    },
    // System responses
    Status {
        request_id: Uuid,
        status: DaemonStatus,
    },
    LanguageList {
        request_id: Uuid,
        languages: Vec<LanguageInfo>,
    },
    Shutdown {
        request_id: Uuid,
    },
    Pong {
        request_id: Uuid,
    },
    Error {
        request_id: Uuid,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyResult {
    pub item: CallHierarchyItem,
    pub incoming: Vec<CallHierarchyCall>,
    pub outgoing: Vec<CallHierarchyCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyItem {
    pub name: String,
    pub kind: String,
    pub uri: String,
    pub range: Range,
    pub selection_range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverContent {
    pub contents: String,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<CompletionItemKind>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub uptime_secs: u64,
    pub pools: Vec<PoolStatus>,
    pub total_requests: u64,
    pub active_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    pub language: Language,
    pub ready_servers: usize,
    pub busy_servers: usize,
    pub total_servers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub language: Language,
    pub lsp_server: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub root: PathBuf,
    pub language: Language,
    pub server_status: ServerStatus,
    pub file_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerStatus {
    Initializing,
    Ready,
    Busy,
    Error(String),
}

pub struct MessageCodec;

impl MessageCodec {
    pub fn encode(msg: &DaemonRequest) -> Result<Vec<u8>> {
        let json = serde_json::to_string(msg)?;
        let bytes = json.as_bytes();

        // Simple length-prefixed encoding
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        encoded.extend_from_slice(bytes);

        Ok(encoded)
    }

    pub fn encode_response(msg: &DaemonResponse) -> Result<Vec<u8>> {
        let json = serde_json::to_string(msg)?;
        let bytes = json.as_bytes();

        let mut encoded = Vec::new();
        encoded.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        encoded.extend_from_slice(bytes);

        Ok(encoded)
    }

    pub fn decode_request(bytes: &[u8]) -> Result<DaemonRequest> {
        if bytes.len() < 4 {
            return Err(anyhow::anyhow!("Message too short"));
        }

        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        if bytes.len() < 4 + len {
            return Err(anyhow::anyhow!("Incomplete message"));
        }

        let json_bytes = &bytes[4..4 + len];
        let request = serde_json::from_slice(json_bytes)?;

        Ok(request)
    }

    pub fn decode_response(bytes: &[u8]) -> Result<DaemonResponse> {
        if bytes.len() < 4 {
            return Err(anyhow::anyhow!("Message too short"));
        }

        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        if bytes.len() < 4 + len {
            return Err(anyhow::anyhow!("Incomplete message"));
        }

        let json_bytes = &bytes[4..4 + len];
        let response = serde_json::from_slice(json_bytes)?;

        Ok(response)
    }
}

// Helper function to convert from serde_json::Value to our types
pub fn parse_call_hierarchy_from_lsp(value: &Value) -> Result<CallHierarchyResult> {
    let item = value
        .get("item")
        .ok_or_else(|| anyhow::anyhow!("Missing item in call hierarchy"))?;

    let incoming = value
        .get("incoming")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_call_hierarchy_call(v).ok())
                .collect()
        })
        .unwrap_or_default();

    let outgoing = value
        .get("outgoing")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_call_hierarchy_call(v).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(CallHierarchyResult {
        item: parse_call_hierarchy_item(item)?,
        incoming,
        outgoing,
    })
}

fn parse_call_hierarchy_item(value: &Value) -> Result<CallHierarchyItem> {
    Ok(CallHierarchyItem {
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        kind: value
            .get("kind")
            .and_then(|v| v.as_u64())
            .map(|k| k.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        uri: value
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        range: parse_range(value.get("range").unwrap_or(&json!({})))?,
        selection_range: parse_range(value.get("selectionRange").unwrap_or(&json!({})))?,
    })
}

fn parse_call_hierarchy_call(value: &Value) -> Result<CallHierarchyCall> {
    let from = value
        .get("from")
        .ok_or_else(|| anyhow::anyhow!("Missing 'from' in call"))?;

    let from_ranges = value
        .get("fromRanges")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|r| parse_range(r).ok()).collect())
        .unwrap_or_default();

    Ok(CallHierarchyCall {
        from: parse_call_hierarchy_item(from)?,
        from_ranges,
    })
}

fn parse_range(value: &Value) -> Result<Range> {
    let default_pos = json!({});
    let start = value.get("start").unwrap_or(&default_pos);
    let end = value.get("end").unwrap_or(&default_pos);

    Ok(Range {
        start: Position {
            line: start.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            character: start.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        },
        end: Position {
            line: end.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            character: end.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        },
    })
}

use serde_json::json;
