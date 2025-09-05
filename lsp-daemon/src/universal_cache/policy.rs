//! Cache Policy Management
//!
//! This module defines caching policies for different LSP methods, including
//! scope rules and invalidation strategies.

use crate::universal_cache::LspMethod;
use std::collections::HashMap;

/// Cache scope defining when cache entries should be invalidated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheScope {
    /// Cache is valid only for the current file content
    /// Invalidated when file changes
    FileContent,

    /// Cache is valid for the current file structure
    /// Invalidated when file or its dependencies change
    FileStructure,

    /// Cache is valid across the workspace
    /// Invalidated when workspace structure changes significantly
    Workspace,

    /// Cache is valid across projects
    /// Rarely invalidated, used for static analysis
    Project,

    /// Cache is session-scoped
    /// Invalidated when language server restarts
    Session,
}

/// Caching policy for an LSP method
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Whether caching is enabled for this method
    pub enabled: bool,

    /// Cache scope determining invalidation rules
    pub scope: CacheScope,

    /// Maximum number of entries to cache for this method per workspace
    pub max_entries_per_workspace: Option<usize>,

    /// Whether to compress cached values
    pub compress: bool,

    /// Priority for LRU eviction (higher = keep longer)
    pub priority: u8,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            scope: CacheScope::FileContent,
            max_entries_per_workspace: Some(1000),
            compress: true,
            priority: 5,
        }
    }
}

/// Registry of caching policies for different LSP methods
pub struct PolicyRegistry {
    policies: HashMap<LspMethod, CachePolicy>,
}

impl PolicyRegistry {
    /// Create a new policy registry with default policies
    pub fn new() -> Self {
        let mut policies = HashMap::new();

        // Definition: File-scoped, medium TTL
        policies.insert(
            LspMethod::Definition,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(2000),
                compress: true,
                priority: 7,
            },
        );

        // References: Workspace-scoped, longer TTL
        policies.insert(
            LspMethod::References,
            CachePolicy {
                enabled: true,
                scope: CacheScope::Workspace,
                max_entries_per_workspace: Some(1500),
                compress: true,
                priority: 8,
            },
        );

        // Hover: File-scoped, short TTL (frequently accessed)
        policies.insert(
            LspMethod::Hover,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(5000),
                compress: false, // Small values, compression overhead not worth it
                priority: 9,     // High priority - frequently accessed
            },
        );

        // Document symbols: File-scoped, medium TTL
        policies.insert(
            LspMethod::DocumentSymbols,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(1000),
                compress: true,
                priority: 6,
            },
        );

        // Workspace symbols: Workspace-scoped, long TTL
        policies.insert(
            LspMethod::WorkspaceSymbols,
            CachePolicy {
                enabled: true,
                scope: CacheScope::Workspace,
                max_entries_per_workspace: Some(500),
                compress: true,
                priority: 5,
            },
        );

        // Type definition: File-scoped, medium TTL
        policies.insert(
            LspMethod::TypeDefinition,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(1000),
                compress: true,
                priority: 6,
            },
        );

        // Implementation: Workspace-scoped, longer TTL
        policies.insert(
            LspMethod::Implementation,
            CachePolicy {
                enabled: true,
                scope: CacheScope::Workspace,
                max_entries_per_workspace: Some(1000),
                compress: true,
                priority: 7,
            },
        );

        // Call hierarchy: Workspace-scoped, long TTL (expensive to compute)
        policies.insert(
            LspMethod::CallHierarchy,
            CachePolicy {
                enabled: true,
                scope: CacheScope::Workspace,
                max_entries_per_workspace: Some(800),
                compress: true,
                priority: 10, // Highest priority - very expensive to compute
            },
        );

        // Signature help: File-scoped, short TTL
        policies.insert(
            LspMethod::SignatureHelp,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(2000),
                compress: false,
                priority: 4,
            },
        );

        // Completion: Disabled by default (too dynamic)
        policies.insert(
            LspMethod::Completion,
            CachePolicy {
                enabled: false,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(100),
                compress: false,
                priority: 2,
            },
        );

        // Code actions: Disabled by default (context-dependent)
        policies.insert(
            LspMethod::CodeAction,
            CachePolicy {
                enabled: false,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(500),
                compress: true,
                priority: 3,
            },
        );

        // Rename: Disabled (one-time operations)
        policies.insert(
            LspMethod::Rename,
            CachePolicy {
                enabled: false,
                scope: CacheScope::Workspace,
                max_entries_per_workspace: None,
                compress: true,
                priority: 1,
            },
        );

        // Folding ranges: File-scoped, long TTL (structural)
        policies.insert(
            LspMethod::FoldingRange,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileStructure,
                max_entries_per_workspace: Some(1000),
                compress: true,
                priority: 4,
            },
        );

        // Selection ranges: File-scoped, long TTL (structural)
        policies.insert(
            LspMethod::SelectionRange,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileStructure,
                max_entries_per_workspace: Some(1000),
                compress: true,
                priority: 4,
            },
        );

        // Semantic tokens: File-scoped, medium TTL
        policies.insert(
            LspMethod::SemanticTokens,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(800),
                compress: true,
                priority: 6,
            },
        );

        // Inlay hints: File-scoped, short TTL
        policies.insert(
            LspMethod::InlayHint,
            CachePolicy {
                enabled: true,
                scope: CacheScope::FileContent,
                max_entries_per_workspace: Some(1500),
                compress: false,
                priority: 5,
            },
        );

        Self { policies }
    }

    /// Get the caching policy for a specific LSP method
    pub fn get_policy(&self, method: LspMethod) -> CachePolicy {
        self.policies.get(&method).cloned().unwrap_or_default()
    }

    /// Update the policy for a specific method
    pub fn set_policy(&mut self, method: LspMethod, policy: CachePolicy) {
        self.policies.insert(method, policy);
    }

    /// Enable or disable caching for a specific method
    pub fn set_enabled(&mut self, method: LspMethod, enabled: bool) {
        if let Some(policy) = self.policies.get_mut(&method) {
            policy.enabled = enabled;
        }
    }

    /// Get all methods that are enabled for caching
    pub fn enabled_methods(&self) -> Vec<LspMethod> {
        self.policies
            .iter()
            .filter_map(
                |(method, policy)| {
                    if policy.enabled {
                        Some(*method)
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    /// Get methods by cache scope
    pub fn methods_by_scope(&self, scope: CacheScope) -> Vec<LspMethod> {
        self.policies
            .iter()
            .filter_map(|(method, policy)| {
                if policy.enabled && policy.scope == scope {
                    Some(*method)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for PolicyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_registry() {
        let registry = PolicyRegistry::new();

        // Check that call hierarchy has highest priority
        let call_hierarchy_policy = registry.get_policy(LspMethod::CallHierarchy);
        assert_eq!(call_hierarchy_policy.priority, 10);
        assert!(call_hierarchy_policy.enabled);

        // Check that completion is disabled by default
        let completion_policy = registry.get_policy(LspMethod::Completion);
        assert!(!completion_policy.enabled);

        // Check that hover has high priority
        let hover_policy = registry.get_policy(LspMethod::Hover);
        assert_eq!(hover_policy.priority, 9);
        assert!(hover_policy.enabled);
    }

    #[test]
    fn test_policy_modification() {
        let mut registry = PolicyRegistry::new();

        // Enable completion
        registry.set_enabled(LspMethod::Completion, true);
        assert!(registry.get_policy(LspMethod::Completion).enabled);

        // Set custom policy
        let custom_policy = CachePolicy {
            enabled: true,
            scope: CacheScope::Session,
            max_entries_per_workspace: Some(10),
            compress: false,
            priority: 1,
        };

        registry.set_policy(LspMethod::CodeAction, custom_policy.clone());
        let retrieved_policy = registry.get_policy(LspMethod::CodeAction);
        assert_eq!(retrieved_policy.scope, CacheScope::Session);
    }

    #[test]
    fn test_methods_by_scope() {
        let registry = PolicyRegistry::new();

        let file_content_methods = registry.methods_by_scope(CacheScope::FileContent);
        assert!(file_content_methods.contains(&LspMethod::Definition));
        assert!(file_content_methods.contains(&LspMethod::Hover));

        let workspace_methods = registry.methods_by_scope(CacheScope::Workspace);
        assert!(workspace_methods.contains(&LspMethod::References));
        assert!(workspace_methods.contains(&LspMethod::CallHierarchy));
    }

    #[test]
    fn test_enabled_methods() {
        let registry = PolicyRegistry::new();

        let enabled = registry.enabled_methods();

        // Should include enabled methods
        assert!(enabled.contains(&LspMethod::Definition));
        assert!(enabled.contains(&LspMethod::References));
        assert!(enabled.contains(&LspMethod::Hover));

        // Should not include disabled methods
        assert!(!enabled.contains(&LspMethod::Completion));
        assert!(!enabled.contains(&LspMethod::Rename));
    }

    #[test]
    fn test_cache_scope_ordering() {
        // Verify that our cache scopes make sense from narrow to broad
        let scopes = [
            CacheScope::FileContent,
            CacheScope::FileStructure,
            CacheScope::Workspace,
            CacheScope::Project,
            CacheScope::Session,
        ];

        // This is more of a documentation test to ensure we think about scope ordering
        assert_eq!(scopes[0], CacheScope::FileContent); // Narrowest
        assert_eq!(scopes[4], CacheScope::Session); // Broadest
    }
}
