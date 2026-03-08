use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use super::ast_extractor::AstSymbolExtractor;
use crate::language_detector::{Language, LanguageDetector};
use crate::symbol::{generate_version_aware_uid, normalize_uid_with_hint, parse_version_aware_uid};
use crate::workspace_utils;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UidValidationStatus {
    /// UID matches the current AST position and computed canonical UID exactly
    ValidExact,
    /// AST symbol found, but canonical UID differs (provide reason)
    Canonicalized { reason: String, new_uid: String },
    /// File belongs to dependencies or is external; skipped
    ExternalOrDep,
    /// File not found on disk
    FileMissing,
    /// Could not parse AST or no matching symbol found at/near location
    NoAstSymbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UidValidationResult {
    pub input_uid: String,
    pub status: UidValidationStatus,
    /// Best-effort canonical UID computed from AST (when available)
    pub canonical_uid: Option<String>,
    /// Absolute file path used during validation (when resolvable)
    pub abs_file: Option<PathBuf>,
    /// 1-based line discovered from AST (when available)
    pub ast_line: Option<u32>,
    /// Symbol name discovered from AST (when available)
    pub ast_name: Option<String>,
}

/// Validate a version-aware UID against the current AST of the file.
///
/// This verifies that the UID’s file/name/line maps to a real symbol. If a nearby symbol
/// with the same name exists but the canonical UID would differ (hash/line/path), the
/// result is Canonicalized with the suggested UID.
pub fn validate_uid_against_ast(
    uid: &str,
    workspace_hint: Option<&Path>,
) -> Result<UidValidationResult> {
    let (rel_path, _hash_part, name_part, line_part) = parse_version_aware_uid(uid)
        .with_context(|| format!("validate_uid_against_ast: invalid uid format: {}", uid))?;

    // Skip dependency/external UIDs early
    if rel_path.starts_with("dep/")
        || rel_path.starts_with("/dep/")
        || rel_path.starts_with("EXTERNAL:")
    {
        return Ok(UidValidationResult {
            input_uid: uid.to_string(),
            status: UidValidationStatus::ExternalOrDep,
            canonical_uid: None,
            abs_file: None,
            ast_line: None,
            ast_name: None,
        });
    }

    // Resolve absolute file path using workspace hint (if provided) or fallback inference
    let abs_file = match workspace_hint {
        Some(ws) => ws.join(&rel_path),
        None => {
            // Heuristic: try CWD + rel_path first; if it doesn't exist, infer workspace root from CWD
            let candidate = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&rel_path);
            if candidate.exists() {
                candidate
            } else {
                // Last resort: try to infer workspace by walking up from candidate path components
                let parts: Vec<&str> = rel_path.split('/').collect();
                let probe = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                let abs = parts.iter().fold(probe.clone(), |p, seg| p.join(seg));
                abs
            }
        }
    };

    if !abs_file.exists() {
        return Ok(UidValidationResult {
            input_uid: uid.to_string(),
            status: UidValidationStatus::FileMissing,
            canonical_uid: None,
            abs_file: Some(abs_file),
            ast_line: None,
            ast_name: None,
        });
    }

    // Read content
    let content = std::fs::read_to_string(&abs_file).with_context(|| {
        format!(
            "validate_uid_against_ast: failed reading {}",
            abs_file.display()
        )
    })?;

    // Detect language and parse
    let detector = LanguageDetector::new();
    let lang = detector.detect(&abs_file).unwrap_or(Language::Unknown);
    if matches!(lang, Language::Unknown) {
        warn!(
            "[UID_VALIDATOR] Unknown language for {}",
            abs_file.display()
        );
        return Ok(UidValidationResult {
            input_uid: uid.to_string(),
            status: UidValidationStatus::NoAstSymbol,
            canonical_uid: None,
            abs_file: Some(abs_file),
            ast_line: None,
            ast_name: None,
        });
    }

    let mut extractor = AstSymbolExtractor::new();
    let symbols = extractor.extract_symbols_from_file(&abs_file, &content, lang)?;

    // Pick best candidate: same name, exact line (1-based), otherwise nearest same-name
    let wanted_line = line_part;
    let mut best: Option<(String, u32)> = None; // (name, ast_line_1based)
    for s in &symbols {
        if s.name != name_part {
            continue;
        }
        let s_line_1 = s.location.start_line.saturating_add(1).max(1);
        if s_line_1 == wanted_line {
            best = Some((s.name.clone(), s_line_1));
            break;
        }
        if best.is_none() {
            best = Some((s.name.clone(), s_line_1));
        }
    }

    // If no same-name candidate, try any symbol at the same line (rename scenario)
    if best.is_none() {
        for s in &symbols {
            let s_line_1 = s.location.start_line.saturating_add(1).max(1);
            if s_line_1 == wanted_line {
                best = Some((s.name.clone(), s_line_1));
                break;
            }
        }
    }

    // Compute workspace root for canonical UID generation/normalization
    let workspace_root = workspace_hint
        .map(|p| p.to_path_buf())
        .or_else(|| workspace_utils::find_workspace_root_with_fallback(&abs_file).ok())
        .unwrap_or_else(|| abs_file.parent().unwrap_or(Path::new(".")).to_path_buf());

    if let Some((ast_name, ast_line_1)) = best {
        let canonical_uid = generate_version_aware_uid(
            &workspace_root,
            &abs_file,
            &content,
            &ast_name,
            ast_line_1,
        )?;
        let normalized = normalize_uid_with_hint(&canonical_uid, Some(&workspace_root));

        if normalized == uid {
            debug!("[UID_VALIDATOR] UID valid: {}", uid);
            return Ok(UidValidationResult {
                input_uid: uid.to_string(),
                status: UidValidationStatus::ValidExact,
                canonical_uid: Some(normalized),
                abs_file: Some(abs_file),
                ast_line: Some(ast_line_1),
                ast_name: Some(ast_name),
            });
        }

        // Derive a human-readable reason
        let reason = if ast_name != name_part && ast_line_1 != wanted_line {
            format!(
                "name+line differ ({}@{} -> {}@{})",
                name_part, wanted_line, ast_name, ast_line_1
            )
        } else if ast_name != name_part {
            format!("name differ ({} -> {})", name_part, ast_name)
        } else if ast_line_1 != wanted_line {
            format!("line differ ({} -> {})", wanted_line, ast_line_1)
        } else {
            "hash/path normalization".to_string()
        };

        warn!(
            "[UID_VALIDATOR] Canonicalization suggested: '{}' -> '{}' ({})",
            uid, normalized, reason
        );

        Ok(UidValidationResult {
            input_uid: uid.to_string(),
            status: UidValidationStatus::Canonicalized {
                reason,
                new_uid: normalized.clone(),
            },
            canonical_uid: Some(normalized),
            abs_file: Some(abs_file),
            ast_line: Some(ast_line_1),
            ast_name: Some(ast_name),
        })
    } else {
        warn!(
            "[UID_VALIDATOR] No AST symbol found near {}:{} for '{}'",
            rel_path, line_part, name_part
        );
        Ok(UidValidationResult {
            input_uid: uid.to_string(),
            status: UidValidationStatus::NoAstSymbol,
            canonical_uid: None,
            abs_file: Some(abs_file),
            ast_line: None,
            ast_name: None,
        })
    }
}
