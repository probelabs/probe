//! Advanced query patterns for DuckDB backend
//!
//! This module implements sophisticated graph analytics and complex LSP operations
//! using DuckDB's powerful SQL capabilities including recursive CTEs, window functions,
//! and advanced filtering for git-aware code analysis.
//!
//! ## Query Categories
//!
//! - **Call Graph Traversal**: Find call chains, paths between symbols, cycle detection
//! - **Impact Analysis**: Determine symbols affected by changes, dependency analysis
//! - **Hotspot Analysis**: Identify most referenced symbols, performance bottlenecks
//! - **Symbol Dependencies**: Build dependency graphs, find transitive dependencies
//! - **Graph Analytics**: Complex multi-table joins with recursive exploration

use anyhow::Result;
use duckdb::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::database::DatabaseError;

/// Result of a call path query - represents a path from one symbol to another through calls
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallPath {
    /// Symbols in the call path from source to target
    pub path: Vec<String>,
    /// Depth of the path (number of hops)
    pub depth: usize,
    /// Whether this path contains cycles
    pub has_cycle: bool,
    /// Call types along the path
    pub call_types: Vec<String>,
    /// Files involved in the path
    pub files: Vec<String>,
}

/// Symbol impact analysis result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolImpact {
    /// Symbol that is affected
    pub symbol_id: String,
    /// Symbol name
    pub name: String,
    /// Qualified name
    pub qualified_name: Option<String>,
    /// Symbol kind (function, class, etc.)
    pub kind: String,
    /// Distance from the changed symbol (0 = directly affected)
    pub depth: usize,
    /// Type of impact (caller, reference, dependent, etc.)
    pub impact_type: String,
    /// File containing the affected symbol
    pub file_path: String,
}

/// Symbol dependency information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolDependency {
    /// Symbol that has the dependency
    pub symbol_id: String,
    /// Symbol name
    pub name: String,
    /// Symbol that is depended upon
    pub depends_on_id: String,
    /// Name of dependency
    pub depends_on_name: String,
    /// Type of dependency (call, reference, import, etc.)
    pub dependency_type: String,
    /// Strength of dependency (number of references)
    pub weight: i32,
    /// File where dependency occurs
    pub file_path: String,
}

/// Hotspot analysis result - identifies symbols that are heavily referenced
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolHotspot {
    /// Symbol identifier
    pub symbol_id: String,
    /// Symbol name
    pub name: String,
    /// Qualified name
    pub qualified_name: Option<String>,
    /// Symbol kind
    pub kind: String,
    /// Number of incoming references
    pub reference_count: i32,
    /// Number of incoming calls
    pub call_count: i32,
    /// Total "heat" score (weighted combination)
    pub heat_score: f64,
    /// File containing the symbol
    pub file_path: String,
}

/// Graph traversal options for controlling query behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTraversalOptions {
    /// Maximum depth to traverse (prevents infinite recursion)
    pub max_depth: usize,
    /// Whether to detect and report cycles
    pub detect_cycles: bool,
    /// Include only specific call types
    pub call_types_filter: Option<Vec<String>>,
    /// Include only specific reference types
    pub reference_types_filter: Option<Vec<String>>,
    /// Limit number of results
    pub result_limit: Option<usize>,
}

impl Default for GraphTraversalOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            detect_cycles: true,
            call_types_filter: None,
            reference_types_filter: None,
            result_limit: Some(1000),
        }
    }
}

/// Advanced query patterns implementation
pub struct DuckDBQueries;

impl DuckDBQueries {
    /// Find all paths from symbol A to symbol B through call graph
    pub async fn find_call_paths(
        conn: &Connection,
        workspace_id: &str,
        from_symbol_id: &str,
        to_symbol_id: &str,
        commit_hash: Option<&str>,
        modified_files: &[String],
        options: GraphTraversalOptions,
    ) -> Result<Vec<CallPath>, DatabaseError> {
        let values_clause = build_modified_files_values_clause(modified_files);

        let call_types_filter = if let Some(ref types) = options.call_types_filter {
            format!("AND cg.call_type IN ('{}')", types.join("','"))
        } else {
            String::new()
        };

        let query = format!(
            r#"
            WITH RECURSIVE 
            modified_files (file_path) AS (
                {}
            ),
            call_paths AS (
                -- Base case: direct calls from source symbol
                SELECT 
                    cg.caller_symbol_id,
                    cg.callee_symbol_id,
                    1 as depth,
                    ARRAY[cg.caller_symbol_id, cg.callee_symbol_id] as path,
                    ARRAY[cg.call_type] as call_types,
                    ARRAY[f.relative_path] as files,
                    FALSE as has_cycle
                FROM call_graph cg
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE cg.workspace_id = ?
                  AND cg.caller_symbol_id = ?
                  {}
                  AND (
                    -- Latest version for modified files
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    -- Git commit version for unmodified files
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
                
                UNION ALL
                
                -- Recursive case: extend existing paths
                SELECT 
                    cp.caller_symbol_id,
                    cg.callee_symbol_id,
                    cp.depth + 1,
                    cp.path || ARRAY[cg.callee_symbol_id],
                    cp.call_types || ARRAY[cg.call_type],
                    cp.files || ARRAY[f.relative_path],
                    CASE 
                        WHEN cg.callee_symbol_id = ANY(cp.path) THEN TRUE
                        ELSE cp.has_cycle
                    END as has_cycle
                FROM call_paths cp
                JOIN call_graph cg ON cp.callee_symbol_id = cg.caller_symbol_id
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE cg.workspace_id = ?
                  AND cp.depth < ?
                  AND (NOT ? OR NOT cg.callee_symbol_id = ANY(cp.path))
                  {}
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
            )
            SELECT 
                array_to_string(path, ',') as path,
                depth,
                has_cycle,
                array_to_string(call_types, ',') as call_types,
                array_to_string(files, ',') as files
            FROM call_paths
            WHERE callee_symbol_id = ?
            ORDER BY depth, array_length(path, 1)
            {}
            "#,
            values_clause,
            call_types_filter,
            call_types_filter,
            if let Some(limit) = options.result_limit {
                format!("LIMIT {}", limit)
            } else {
                String::new()
            }
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to prepare call paths query: {}", e),
            })?;

        let rows = stmt
            .query_map(
                params![
                    workspace_id,
                    from_symbol_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    options.max_depth,
                    options.detect_cycles,
                    commit_hash.unwrap_or(""),
                    to_symbol_id,
                ],
                |row| {
                    let path_str: String = row.get(0)?;
                    let depth: usize = row.get::<_, i64>(1)? as usize;
                    let has_cycle: bool = row.get(2)?;
                    let call_types_str: String = row.get(3)?;
                    let files_str: String = row.get(4)?;

                    let path: Vec<String> = path_str.split(',').map(|s| s.to_string()).collect();
                    let call_types: Vec<String> =
                        call_types_str.split(',').map(|s| s.to_string()).collect();
                    let files: Vec<String> = files_str.split(',').map(|s| s.to_string()).collect();

                    Ok(CallPath {
                        path,
                        depth,
                        has_cycle,
                        call_types,
                        files,
                    })
                },
            )
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to execute call paths query: {}", e),
            })?;

        let results: Result<Vec<CallPath>, _> = rows.collect();
        results.map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to collect call paths results: {}", e),
        })
    }

    /// Find all symbols affected by changing a given symbol (impact analysis)
    pub async fn find_affected_symbols(
        conn: &Connection,
        workspace_id: &str,
        changed_symbol_id: &str,
        commit_hash: Option<&str>,
        modified_files: &[String],
        options: GraphTraversalOptions,
    ) -> Result<Vec<SymbolImpact>, DatabaseError> {
        let values_clause = build_modified_files_values_clause(modified_files);

        let query = format!(
            r#"
            WITH 
            modified_files (file_path) AS (
                {}
            ),
            affected_symbols AS (
                -- Direct references to the changed symbol
                SELECT DISTINCT 
                    sr.source_symbol_id as symbol_id,
                    1 as depth,
                    'reference' as impact_type
                FROM symbol_references sr
                JOIN files f ON sr.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE sr.workspace_id = ?
                  AND sr.target_symbol_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND sr.indexed_at = (
                        SELECT MAX(sr2.indexed_at)
                        FROM symbol_references sr2
                        WHERE sr2.reference_id = sr.reference_id
                    ))
                    OR
                    (mf.file_path IS NULL AND sr.git_commit_hash = ?)
                  )
                
                UNION
                
                -- Direct callers of the changed symbol
                SELECT DISTINCT 
                    cg.caller_symbol_id as symbol_id,
                    1 as depth,
                    'caller' as impact_type
                FROM call_graph cg
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE cg.workspace_id = ?
                  AND cg.callee_symbol_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
            )
            SELECT 
                a.symbol_id,
                a.depth,
                a.impact_type,
                s.name,
                s.qualified_name,
                s.kind,
                f.relative_path as file_path
            FROM affected_symbols a
            JOIN symbols s ON a.symbol_id = s.symbol_id
            JOIN files f ON s.file_id = f.file_id
            LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
            WHERE s.workspace_id = ?
              AND (
                (mf.file_path IS NOT NULL AND s.indexed_at = (
                    SELECT MAX(s2.indexed_at)
                    FROM symbols s2
                    WHERE s2.symbol_id = s.symbol_id
                ))
                OR
                (mf.file_path IS NULL AND s.git_commit_hash = ?)
              )
            ORDER BY a.depth, s.name
            {}
            "#,
            values_clause,
            if let Some(limit) = options.result_limit {
                format!("LIMIT {}", limit)
            } else {
                String::new()
            }
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to prepare affected symbols query: {}", e),
            })?;

        let rows = stmt
            .query_map(
                params![
                    workspace_id,
                    changed_symbol_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    changed_symbol_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    commit_hash.unwrap_or(""),
                ],
                |row| {
                    Ok(SymbolImpact {
                        symbol_id: row.get(0)?,
                        depth: row.get::<_, i64>(1)? as usize,
                        impact_type: row.get(2)?,
                        name: row.get(3)?,
                        qualified_name: row.get(4)?,
                        kind: row.get(5)?,
                        file_path: row.get(6)?,
                    })
                },
            )
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to execute affected symbols query: {}", e),
            })?;

        let results: Result<Vec<SymbolImpact>, _> = rows.collect();
        results.map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to collect affected symbols results: {}", e),
        })
    }

    /// Get symbol dependency graph
    pub async fn get_symbol_dependencies(
        conn: &Connection,
        workspace_id: &str,
        symbol_id: &str,
        commit_hash: Option<&str>,
        modified_files: &[String],
        options: GraphTraversalOptions,
    ) -> Result<Vec<SymbolDependency>, DatabaseError> {
        let values_clause = build_modified_files_values_clause(modified_files);

        let reference_filter = if let Some(ref types) = options.reference_types_filter {
            format!("AND sr.reference_kind IN ('{}')", types.join("','"))
        } else {
            String::new()
        };

        let query = format!(
            r#"
            WITH modified_files (file_path) AS (
                {}
            ),
            -- Direct dependencies from references
            reference_deps AS (
                SELECT 
                    sr.source_symbol_id as symbol_id,
                    source.name as name,
                    sr.target_symbol_id as depends_on_id,
                    target.name as depends_on_name,
                    sr.reference_kind as dependency_type,
                    COUNT(*) as weight,
                    f.relative_path as file_path
                FROM symbol_references sr
                JOIN symbols source ON sr.source_symbol_id = source.symbol_id
                JOIN symbols target ON sr.target_symbol_id = target.symbol_id
                JOIN files f ON sr.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE sr.workspace_id = ?
                  AND (? = '' OR sr.source_symbol_id = ? OR sr.target_symbol_id = ?)
                  {}
                  AND (
                    (mf.file_path IS NOT NULL AND sr.indexed_at = (
                        SELECT MAX(sr2.indexed_at)
                        FROM symbol_references sr2
                        WHERE sr2.reference_id = sr.reference_id
                    ))
                    OR
                    (mf.file_path IS NULL AND sr.git_commit_hash = ?)
                  )
                GROUP BY sr.source_symbol_id, source.name, sr.target_symbol_id, 
                         target.name, sr.reference_kind, f.relative_path
            ),
            -- Direct dependencies from calls
            call_deps AS (
                SELECT 
                    cg.caller_symbol_id as symbol_id,
                    caller.name as name,
                    cg.callee_symbol_id as depends_on_id,
                    callee.name as depends_on_name,
                    CONCAT('call_', cg.call_type) as dependency_type,
                    COUNT(*) as weight,
                    f.relative_path as file_path
                FROM call_graph cg
                JOIN symbols caller ON cg.caller_symbol_id = caller.symbol_id
                JOIN symbols callee ON cg.callee_symbol_id = callee.symbol_id
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE cg.workspace_id = ?
                  AND (? = '' OR cg.caller_symbol_id = ? OR cg.callee_symbol_id = ?)
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
                GROUP BY cg.caller_symbol_id, caller.name, cg.callee_symbol_id, 
                         callee.name, cg.call_type, f.relative_path
            )
            SELECT * FROM reference_deps
            UNION ALL
            SELECT * FROM call_deps
            ORDER BY weight DESC, dependency_type
            {}
            "#,
            values_clause,
            reference_filter,
            if let Some(limit) = options.result_limit {
                format!("LIMIT {}", limit)
            } else {
                String::new()
            }
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to prepare symbol dependencies query: {}", e),
            })?;

        let symbol_param = if symbol_id.is_empty() { "" } else { symbol_id };

        let rows = stmt
            .query_map(
                params![
                    workspace_id,
                    symbol_param,
                    symbol_id,
                    symbol_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    symbol_param,
                    symbol_id,
                    symbol_id,
                    commit_hash.unwrap_or(""),
                ],
                |row| {
                    Ok(SymbolDependency {
                        symbol_id: row.get(0)?,
                        name: row.get(1)?,
                        depends_on_id: row.get(2)?,
                        depends_on_name: row.get(3)?,
                        dependency_type: row.get(4)?,
                        weight: row.get(5)?,
                        file_path: row.get(6)?,
                    })
                },
            )
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to execute symbol dependencies query: {}", e),
            })?;

        let results: Result<Vec<SymbolDependency>, _> = rows.collect();
        results.map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to collect symbol dependencies results: {}", e),
        })
    }

    /// Analyze symbol hotspots (most referenced/called symbols)
    pub async fn analyze_symbol_hotspots(
        conn: &Connection,
        workspace_id: &str,
        commit_hash: Option<&str>,
        modified_files: &[String],
        limit: Option<usize>,
    ) -> Result<Vec<SymbolHotspot>, DatabaseError> {
        let values_clause = build_modified_files_values_clause(modified_files);

        let query = format!(
            r#"
            WITH modified_files (file_path) AS (
                {}
            ),
            -- Count references to each symbol
            reference_counts AS (
                SELECT 
                    sr.target_symbol_id as symbol_id,
                    COUNT(DISTINCT sr.source_symbol_id) as reference_count
                FROM symbol_references sr
                JOIN files f ON sr.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE sr.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND sr.indexed_at = (
                        SELECT MAX(sr2.indexed_at)
                        FROM symbol_references sr2
                        WHERE sr2.reference_id = sr.reference_id
                    ))
                    OR
                    (mf.file_path IS NULL AND sr.git_commit_hash = ?)
                  )
                GROUP BY sr.target_symbol_id
            ),
            -- Count calls to each symbol
            call_counts AS (
                SELECT 
                    cg.callee_symbol_id as symbol_id,
                    COUNT(DISTINCT cg.caller_symbol_id) as call_count
                FROM call_graph cg
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE cg.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
                GROUP BY cg.callee_symbol_id
            ),
            -- Combine counts and calculate heat scores
            hotspots AS (
                SELECT 
                    s.symbol_id,
                    s.name,
                    s.qualified_name,
                    s.kind,
                    COALESCE(rc.reference_count, 0) as reference_count,
                    COALESCE(cc.call_count, 0) as call_count,
                    -- Heat score: weighted combination (calls are worth more than references)
                    (COALESCE(rc.reference_count, 0) + COALESCE(cc.call_count, 0) * 2.0) as heat_score,
                    f.relative_path as file_path
                FROM symbols s
                JOIN files f ON s.file_id = f.file_id
                LEFT JOIN reference_counts rc ON s.symbol_id = rc.symbol_id
                LEFT JOIN call_counts cc ON s.symbol_id = cc.symbol_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE s.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND s.indexed_at = (
                        SELECT MAX(s2.indexed_at)
                        FROM symbols s2
                        WHERE s2.symbol_id = s.symbol_id
                    ))
                    OR
                    (mf.file_path IS NULL AND s.git_commit_hash = ?)
                  )
                  AND (COALESCE(rc.reference_count, 0) + COALESCE(cc.call_count, 0)) > 0
            )
            SELECT 
                symbol_id,
                name,
                qualified_name,
                kind,
                reference_count,
                call_count,
                heat_score,
                file_path
            FROM hotspots
            ORDER BY heat_score DESC, name
            {}
            "#,
            values_clause,
            if let Some(limit_val) = limit {
                format!("LIMIT {}", limit_val)
            } else {
                String::new()
            }
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to prepare hotspots query: {}", e),
            })?;

        let rows = stmt
            .query_map(
                params![
                    workspace_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    commit_hash.unwrap_or(""),
                ],
                |row| {
                    Ok(SymbolHotspot {
                        symbol_id: row.get(0)?,
                        name: row.get(1)?,
                        qualified_name: row.get(2)?,
                        kind: row.get(3)?,
                        reference_count: row.get(4)?,
                        call_count: row.get(5)?,
                        heat_score: row.get(6)?,
                        file_path: row.get(7)?,
                    })
                },
            )
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to execute hotspots query: {}", e),
            })?;

        let results: Result<Vec<SymbolHotspot>, _> = rows.collect();
        results.map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to collect hotspots results: {}", e),
        })
    }

    /// Perform call graph traversal with cycle detection
    pub async fn traverse_call_graph(
        conn: &Connection,
        workspace_id: &str,
        start_symbol_id: &str,
        direction: TraversalDirection,
        commit_hash: Option<&str>,
        modified_files: &[String],
        options: GraphTraversalOptions,
    ) -> Result<CallGraphTraversal, DatabaseError> {
        let values_clause = build_modified_files_values_clause(modified_files);

        let (source_col, target_col) = match direction {
            TraversalDirection::Outgoing => ("caller_symbol_id", "callee_symbol_id"),
            TraversalDirection::Incoming => ("callee_symbol_id", "caller_symbol_id"),
        };

        let query = format!(
            r#"
            WITH RECURSIVE
            modified_files (file_path) AS (
                {}
            ),
            call_traversal AS (
                -- Base case: direct calls from start symbol
                SELECT 
                    cg.{} as from_symbol,
                    cg.{} as to_symbol,
                    1 as depth,
                    cg.{} || ',' || cg.{} as path_str,
                    cg.call_type as call_types_str,
                    FALSE as has_cycle,
                    s_from.name as from_name,
                    s_to.name as to_name,
                    f_from.relative_path as from_file,
                    f_to.relative_path as to_file
                FROM call_graph cg
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                LEFT JOIN symbols s_from ON cg.{} = s_from.symbol_id
                LEFT JOIN symbols s_to ON cg.{} = s_to.symbol_id
                LEFT JOIN files f_from ON s_from.file_id = f_from.file_id
                LEFT JOIN files f_to ON s_to.file_id = f_to.file_id
                WHERE cg.workspace_id = ?
                  AND cg.{} = ?
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
                
                UNION ALL
                
                -- Recursive case: follow the call chain
                SELECT 
                    ct.to_symbol as from_symbol,
                    cg.{} as to_symbol,
                    ct.depth + 1 as depth,
                    ct.path_str || ',' || cg.{} as path_str,
                    ct.call_types_str || ',' || cg.call_type as call_types_str,
                    (position(cg.{} in ct.path_str) > 0) as has_cycle,
                    s_from.name as from_name,
                    s_to.name as to_name,
                    f_from.relative_path as from_file,
                    f_to.relative_path as to_file
                FROM call_traversal ct
                JOIN call_graph cg ON ct.to_symbol = cg.{}
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                LEFT JOIN symbols s_from ON cg.{} = s_from.symbol_id
                LEFT JOIN symbols s_to ON cg.{} = s_to.symbol_id
                LEFT JOIN files f_from ON s_from.file_id = f_from.file_id
                LEFT JOIN files f_to ON s_to.file_id = f_to.file_id
                WHERE cg.workspace_id = ?
                  AND ct.depth < ?
                  AND NOT (position(cg.{} in ct.path_str) > 0)  -- Cycle detection
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
            )
            SELECT * FROM call_traversal"#,
            values_clause,
            source_col,
            target_col,
            source_col,
            target_col,
            source_col,
            target_col,
            source_col,
            target_col,
            target_col,
            target_col,
            source_col,
            source_col,
            target_col,
            target_col
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to prepare graph traversal query: {}", e),
            })?;

        let _detect_cycles = options.detect_cycles;

        let rows = stmt
            .query_map(
                params![
                    workspace_id,
                    start_symbol_id,
                    commit_hash.unwrap_or(""),
                    workspace_id,
                    options.max_depth,
                    commit_hash.unwrap_or(""),
                ],
                |row| {
                    let path_str: String = row.get(3)?;
                    let call_types_str: String = row.get(4)?;
                    let path: Vec<String> = path_str.split(',').map(|s| s.to_string()).collect();
                    let call_types: Vec<String> =
                        call_types_str.split(',').map(|s| s.to_string()).collect();

                    Ok(CallGraphNode {
                        from_symbol: row.get(0)?,
                        to_symbol: row.get(1)?,
                        depth: row.get::<_, i64>(2)? as usize,
                        path,
                        call_types,
                        has_cycle: row.get(5)?,
                        from_name: row.get(6)?,
                        to_name: row.get(7)?,
                        from_file: row.get(8)?,
                        to_file: row.get(9)?,
                    })
                },
            )
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to execute graph traversal query: {}", e),
            })?;

        let nodes: Result<Vec<CallGraphNode>, _> = rows.collect();
        let nodes = nodes.map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to collect traversal results: {}", e),
        })?;

        // Analyze the traversal results
        let total_nodes = nodes.len();
        let cycles_detected = nodes.iter().filter(|n| n.has_cycle).count();
        let max_depth_reached = nodes.iter().map(|n| n.depth).max().unwrap_or(0);

        Ok(CallGraphTraversal {
            start_symbol: start_symbol_id.to_string(),
            direction,
            nodes,
            total_nodes,
            cycles_detected,
            max_depth_reached,
            options,
        })
    }
}

/// Direction for call graph traversal
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TraversalDirection {
    /// Follow outgoing calls (what this symbol calls)
    Outgoing,
    /// Follow incoming calls (what calls this symbol)
    Incoming,
}

/// Node in call graph traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphNode {
    pub from_symbol: String,
    pub to_symbol: String,
    pub depth: usize,
    pub path: Vec<String>,
    pub call_types: Vec<String>,
    pub has_cycle: bool,
    pub from_name: String,
    pub to_name: String,
    pub from_file: String,
    pub to_file: String,
}

/// Complete call graph traversal result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphTraversal {
    pub start_symbol: String,
    pub direction: TraversalDirection,
    pub nodes: Vec<CallGraphNode>,
    pub total_nodes: usize,
    pub cycles_detected: usize,
    pub max_depth_reached: usize,
    pub options: GraphTraversalOptions,
}

/// Build SQL VALUES clause for modified files list
fn build_modified_files_values_clause(modified_files: &[String]) -> String {
    if modified_files.is_empty() {
        "SELECT NULL as file_path WHERE FALSE".to_string()
    } else {
        let values: Vec<String> = modified_files
            .iter()
            .map(|path| format!("('{}')", path.replace("'", "''"))) // Escape single quotes
            .collect();
        format!("VALUES {}", values.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_connection() -> Result<Connection, duckdb::Error> {
        let conn = Connection::open_in_memory()?;

        // Create minimal test schema
        conn.execute_batch(
            r#"
            CREATE TABLE workspaces (
                workspace_id TEXT PRIMARY KEY,
                root_path TEXT NOT NULL,
                name TEXT NOT NULL,
                current_commit TEXT
            );
            
            CREATE TABLE files (
                file_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                absolute_path TEXT NOT NULL,
                language TEXT
            );
            
            CREATE TABLE symbols (
                symbol_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                file_id TEXT NOT NULL,
                git_commit_hash TEXT,
                name TEXT NOT NULL,
                qualified_name TEXT,
                kind TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                start_column INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                end_column INTEGER NOT NULL,
                indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE TABLE call_graph (
                call_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                caller_symbol_id TEXT NOT NULL,
                callee_symbol_id TEXT NOT NULL,
                file_id TEXT NOT NULL,
                git_commit_hash TEXT,
                call_line INTEGER NOT NULL,
                call_column INTEGER NOT NULL,
                call_type TEXT DEFAULT 'direct',
                indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE TABLE symbol_references (
                reference_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                source_symbol_id TEXT NOT NULL,
                target_symbol_id TEXT NOT NULL,
                file_id TEXT NOT NULL,
                git_commit_hash TEXT,
                ref_line INTEGER NOT NULL,
                ref_column INTEGER NOT NULL,
                reference_kind TEXT DEFAULT 'use',
                indexed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        Ok(conn)
    }

    fn insert_test_data(conn: &Connection) -> Result<(), duckdb::Error> {
        // Insert test workspace
        conn.execute(
            "INSERT INTO workspaces VALUES ('ws1', '/test', 'test-workspace', 'abc123')",
            [],
        )?;

        // Insert test files
        conn.execute(
            "INSERT INTO files VALUES ('file1', 'ws1', 'main.rs', '/test/main.rs', 'rust')",
            [],
        )?;
        conn.execute(
            "INSERT INTO files VALUES ('file2', 'ws1', 'lib.rs', '/test/lib.rs', 'rust')",
            [],
        )?;

        // Insert test symbols (symbol_id, workspace_id, file_id, git_commit_hash, name, qualified_name, kind, start_line, start_column, end_line, end_column, indexed_at)
        conn.execute(
            "INSERT INTO symbols VALUES ('sym1', 'ws1', 'file1', 'abc123', 'main', 'main', 'function', 1, 1, 10, 1, CURRENT_TIMESTAMP)",
            [],
        )?;
        conn.execute(
            "INSERT INTO symbols VALUES ('sym2', 'ws1', 'file1', 'abc123', 'helper', 'helper', 'function', 15, 1, 20, 1, CURRENT_TIMESTAMP)",
            [],
        )?;
        conn.execute(
            "INSERT INTO symbols VALUES ('sym3', 'ws1', 'file2', 'abc123', 'utility', 'mod::utility', 'function', 5, 1, 15, 1, CURRENT_TIMESTAMP)",
            [],
        )?;

        // Insert test calls (call_id, workspace_id, caller_symbol_id, callee_symbol_id, file_id, git_commit_hash, call_line, call_column, call_type, indexed_at)
        conn.execute(
            "INSERT INTO call_graph VALUES ('call1', 'ws1', 'sym1', 'sym2', 'file1', 'abc123', 5, 10, 'direct', CURRENT_TIMESTAMP)",
            [],
        )?;
        conn.execute(
            "INSERT INTO call_graph VALUES ('call2', 'ws1', 'sym2', 'sym3', 'file2', 'abc123', 8, 5, 'direct', CURRENT_TIMESTAMP)",
            [],
        )?;

        // Insert test references (reference_id, workspace_id, source_symbol_id, target_symbol_id, file_id, git_commit_hash, ref_line, ref_column, reference_kind, indexed_at)
        conn.execute(
            "INSERT INTO symbol_references VALUES ('ref1', 'ws1', 'sym1', 'sym3', 'file1', 'abc123', 3, 15, 'use', CURRENT_TIMESTAMP)",
            [],
        )?;

        Ok(())
    }

    #[tokio::test]
    async fn test_find_call_paths() {
        let conn = create_test_connection().expect("Failed to create test connection");
        insert_test_data(&conn).expect("Failed to insert test data");

        let options = GraphTraversalOptions::default();
        let result = DuckDBQueries::find_call_paths(
            &conn,
            "ws1",
            "sym1",
            "sym3",
            Some("abc123"),
            &[],
            options,
        )
        .await;

        assert!(result.is_ok());
        let paths = result.unwrap();
        assert!(!paths.is_empty());

        // Should find a path: sym1 -> sym2 -> sym3
        let path = &paths[0];
        assert_eq!(path.depth, 2);
        assert_eq!(path.path, vec!["sym1", "sym2", "sym3"]);
        assert!(!path.has_cycle);
    }

    #[tokio::test]
    async fn test_find_affected_symbols() {
        let conn = create_test_connection().expect("Failed to create test connection");
        insert_test_data(&conn).expect("Failed to insert test data");

        let options = GraphTraversalOptions::default();
        let result = DuckDBQueries::find_affected_symbols(
            &conn,
            "ws1",
            "sym3",
            Some("abc123"),
            &[],
            options,
        )
        .await;

        if let Err(ref e) = result {
            eprintln!("Error in find_affected_symbols: {:?}", e);
        }
        assert!(result.is_ok());
        let affected = result.unwrap();
        assert!(!affected.is_empty());

        // sym1 and sym2 should be affected (reference and call respectively)
        let symbol_ids: Vec<&str> = affected.iter().map(|s| s.symbol_id.as_str()).collect();
        assert!(symbol_ids.contains(&"sym1")); // References sym3
        assert!(symbol_ids.contains(&"sym2")); // Calls sym3
    }

    #[tokio::test]
    async fn test_get_symbol_dependencies() {
        let conn = create_test_connection().expect("Failed to create test connection");
        insert_test_data(&conn).expect("Failed to insert test data");

        let options = GraphTraversalOptions::default();
        let result = DuckDBQueries::get_symbol_dependencies(
            &conn,
            "ws1",
            "sym1",
            Some("abc123"),
            &[],
            options,
        )
        .await;

        assert!(result.is_ok());
        let deps = result.unwrap();
        assert!(!deps.is_empty());

        // sym1 should have dependencies on sym2 (call) and sym3 (reference)
        let dependency_names: Vec<&str> = deps.iter().map(|d| d.depends_on_name.as_str()).collect();
        assert!(dependency_names.contains(&"helper")); // sym2
        assert!(dependency_names.contains(&"utility")); // sym3
    }

    #[tokio::test]
    async fn test_analyze_symbol_hotspots() {
        let conn = create_test_connection().expect("Failed to create test connection");
        insert_test_data(&conn).expect("Failed to insert test data");

        let result =
            DuckDBQueries::analyze_symbol_hotspots(&conn, "ws1", Some("abc123"), &[], Some(10))
                .await;

        assert!(result.is_ok());
        let hotspots = result.unwrap();
        assert!(!hotspots.is_empty());

        // sym3 should be the hottest (called by sym2 and referenced by sym1)
        let hottest = &hotspots[0];
        assert_eq!(hottest.name, "utility");
        assert!(hottest.heat_score > 0.0);
    }

    #[tokio::test]
    async fn test_traverse_call_graph_outgoing() {
        let conn = create_test_connection().expect("Failed to create test connection");
        insert_test_data(&conn).expect("Failed to insert test data");

        let options = GraphTraversalOptions::default();
        let result = DuckDBQueries::traverse_call_graph(
            &conn,
            "ws1",
            "sym1",
            TraversalDirection::Outgoing,
            Some("abc123"),
            &[],
            options,
        )
        .await;

        assert!(result.is_ok());
        let traversal = result.unwrap();
        assert!(!traversal.nodes.is_empty());
        assert_eq!(traversal.start_symbol, "sym1");
        assert_eq!(traversal.cycles_detected, 0);

        // Should find sym1 -> sym2 -> sym3 path
        assert!(traversal
            .nodes
            .iter()
            .any(|n| n.from_symbol == "sym1" && n.to_symbol == "sym2"));
        assert!(traversal
            .nodes
            .iter()
            .any(|n| n.from_symbol == "sym2" && n.to_symbol == "sym3"));
    }

    #[tokio::test]
    async fn test_build_modified_files_values_clause() {
        let empty_files: Vec<String> = vec![];
        let clause = build_modified_files_values_clause(&empty_files);
        assert_eq!(clause, "SELECT NULL as file_path WHERE FALSE");

        let files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
        let clause = build_modified_files_values_clause(&files);
        assert_eq!(clause, "VALUES ('src/main.rs'), ('src/lib.rs')");

        // Test SQL injection protection
        let files_with_quotes = vec!["file'with'quotes.rs".to_string()];
        let clause = build_modified_files_values_clause(&files_with_quotes);
        assert_eq!(clause, "VALUES ('file''with''quotes.rs')");
    }

    #[tokio::test]
    async fn test_graph_traversal_options() {
        let options = GraphTraversalOptions::default();
        assert_eq!(options.max_depth, 10);
        assert!(options.detect_cycles);
        assert!(options.call_types_filter.is_none());
        assert!(options.reference_types_filter.is_none());
        assert_eq!(options.result_limit, Some(1000));

        let custom_options = GraphTraversalOptions {
            max_depth: 5,
            detect_cycles: false,
            call_types_filter: Some(vec!["direct".to_string()]),
            reference_types_filter: Some(vec!["use".to_string(), "call".to_string()]),
            result_limit: Some(100),
        };
        assert_eq!(custom_options.max_depth, 5);
        assert!(!custom_options.detect_cycles);
        assert_eq!(
            custom_options.call_types_filter,
            Some(vec!["direct".to_string()])
        );
    }
}
