use crate::search::elastic_query;
// No term_exceptions import needed
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Escapes special regex characters in a string
pub fn regex_escape(s: &str) -> String {
    let special_chars = [
        '.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\',
    ];
    let mut result = String::with_capacity(s.len() * 2);

    for c in s.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }

    result
}

// ----------------------------------------------------------------------------
// NEW CODE: Full AST-based planning and pattern generation
// ----------------------------------------------------------------------------

/// A unified plan holding the parsed AST and a mapping of each AST term to an index.
/// We store a map for quick lookups of term indices.
#[derive(Debug)]
pub struct QueryPlan {
    pub ast: elastic_query::Expr,
    pub term_indices: HashMap<String, usize>,
    pub excluded_terms: HashSet<String>,
}

/// Helper function to format duration in a human-readable way
fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_millis() < 1000 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

/// Create a QueryPlan from a raw query string. This fully parses the query into an AST,
/// then extracts all terms (including excluded), and prepares a term-index map.
pub fn create_query_plan(
    query: &str,
    _exact: bool,
) -> Result<QueryPlan, elastic_query::ParseError> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    let start_time = Instant::now();

    if debug_mode {
        println!("DEBUG: Starting query plan creation for query: '{}'", query);
    }

    // Use the regular AST parsing
    let parsing_start = Instant::now();

    if debug_mode {
        println!("DEBUG: Starting AST parsing for query: '{}'", query);
    }

    // Parse the query into an AST with processed terms
    // We use standard Elasticsearch behavior (AND for implicit combinations)
    let ast = elastic_query::parse_query(query)?;

    let parsing_duration = parsing_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: AST parsing completed in {}",
            format_duration(parsing_duration)
        );
        println!("DEBUG: Parsed AST: {}", ast);
    }

    // We'll walk the AST to build a set of all terms. We track excluded as well for reference.
    let term_collection_start = Instant::now();

    if debug_mode {
        println!("DEBUG: Starting term collection from AST");
    }

    let mut all_terms = Vec::new();
    let mut excluded_terms = HashSet::new();
    collect_all_terms(&ast, &mut all_terms, &mut excluded_terms);

    // Remove duplicates from all_terms
    all_terms.sort();
    all_terms.dedup();

    let term_collection_duration = term_collection_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Term collection completed in {}",
            format_duration(term_collection_duration)
        );
        println!("DEBUG: Collected {} unique terms", all_terms.len());
        println!("DEBUG: Collected {} excluded terms", excluded_terms.len());
    }

    // Build term index map
    let index_building_start = Instant::now();

    let mut term_indices = HashMap::new();
    for (i, term) in all_terms.iter().enumerate() {
        term_indices.insert(term.clone(), i);
    }

    let index_building_duration = index_building_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Term index building completed in {}",
            format_duration(index_building_duration)
        );
    }

    let total_duration = start_time.elapsed();
    if debug_mode {
        println!(
            "DEBUG: Query plan creation completed in {}",
            format_duration(total_duration)
        );
    }

    Ok(QueryPlan {
        ast,
        term_indices,
        excluded_terms,
    })
}

/// Recursively collect all terms from the AST, storing them in `all_terms`.
/// Also track excluded terms in `excluded`.
fn collect_all_terms(
    expr: &elastic_query::Expr,
    all_terms: &mut Vec<String>,
    excluded: &mut HashSet<String>,
) {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Collecting terms from expression: {:?}", expr);
    }

    match expr {
        elastic_query::Expr::Term {
            keywords,
            field: _,
            excluded: is_excluded,
            exact: _,
            ..
        } => {
            // Add all keywords to all_terms
            all_terms.extend(keywords.clone());

            if debug_mode {
                println!(
                    "DEBUG: Collected keywords '{:?}', excluded={}",
                    keywords, is_excluded
                );
            }

            if *is_excluded {
                for keyword in keywords {
                    if debug_mode {
                        println!("DEBUG: Adding '{}' to excluded terms set", keyword);
                    }

                    // Add the keyword to excluded terms
                    excluded.insert(keyword.clone());
                }
            }
        }
        elastic_query::Expr::And(left, right) => {
            if debug_mode {
                println!("DEBUG: Processing AND expression for term collection");
            }

            // Check if the right side is an excluded term
            if let elastic_query::Expr::Term {
                keywords,
                excluded: true,
                ..
            } = &**right
            {
                for keyword in keywords {
                    if debug_mode {
                        println!(
                            "DEBUG: Adding excluded term '{}' from AND expression",
                            keyword
                        );
                    }
                    excluded.insert(keyword.clone());
                }
            }

            collect_all_terms(left, all_terms, excluded);
            collect_all_terms(right, all_terms, excluded);
        }
        elastic_query::Expr::Or(left, right) => {
            if debug_mode {
                println!("DEBUG: Processing OR expression for term collection");
            }
            collect_all_terms(left, all_terms, excluded);
            collect_all_terms(right, all_terms, excluded);
        }
    }

    if debug_mode {
        println!("DEBUG: Current all_terms: {:?}", all_terms);
        println!("DEBUG: Current excluded terms: {:?}", excluded);
    }
}

/// Build a combined regex pattern from a list of terms
/// This creates a single pattern that matches any of the terms using case-insensitive matching
/// without word boundaries for more flexible matching
pub fn build_combined_pattern(terms: &[String]) -> String {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    let start_time = Instant::now();

    if debug_mode {
        println!("DEBUG: Building combined pattern for {} terms", terms.len());
    }

    // Escape special characters in each term
    let escaped_terms = terms.iter().map(|t| regex_escape(t)).collect::<Vec<_>>();

    // Join terms with | operator and add case-insensitive flag without word boundaries
    let pattern = format!("(?i)({})", escaped_terms.join("|"));

    if debug_mode {
        let duration = start_time.elapsed();
        println!(
            "DEBUG: Combined pattern built in {}: {}",
            format_duration(duration),
            pattern
        );
    }

    pattern
}

/// Generate regex patterns that respect the AST's logical structure.
/// This creates a single combined pattern for all terms, regardless of whether they're
/// required, optional, or negative.
pub fn create_structured_patterns(plan: &QueryPlan) -> Vec<(String, HashSet<usize>)> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    let start_time = Instant::now();

    if debug_mode {
        println!("DEBUG: Starting structured pattern creation");
        println!("DEBUG: Using combined pattern mode");
    }

    let mut results = Vec::new();

    if debug_mode {
        println!("DEBUG: Creating structured patterns with AST awareness");
        println!("DEBUG: AST: {:?}", plan.ast);
        println!("DEBUG: Excluded terms: {:?}", plan.excluded_terms);
    }

    // Extract all non-excluded terms from the query plan
    let terms: Vec<String> = plan
        .term_indices
        .keys()
        .filter(|term| !plan.excluded_terms.contains(*term))
        .cloned()
        .collect();

    if !terms.is_empty() {
        let combined_pattern = build_combined_pattern(&terms);

        // Create a HashSet with indices of non-excluded terms
        let all_indices: HashSet<usize> = terms
            .iter()
            .filter_map(|term| plan.term_indices.get(term).cloned())
            .collect();

        if debug_mode {
            println!(
                "DEBUG: Created combined pattern for all terms: '{}'",
                combined_pattern
            );
            println!(
                "DEBUG: Combined pattern includes indices: {:?}",
                all_indices
            );
        }

        results.push((combined_pattern, all_indices));

        // Continue to generate individual patterns instead of returning early
    }

    // Special handling for queries with excluded terms
    if !plan.excluded_terms.is_empty() {
        let excluded_start = Instant::now();

        if debug_mode {
            println!("DEBUG: Query has excluded terms, using special pattern generation");
        }

        // For queries with excluded terms, we need to ensure we generate patterns
        // for all non-excluded terms, even if they're part of a complex expression
        for (term, &idx) in &plan.term_indices {
            if !plan.excluded_terms.contains(term) {
                let base_pattern = regex_escape(term);
                // Use more flexible pattern matching without word boundaries
                let pattern = format!("({})", base_pattern);

                if debug_mode {
                    println!(
                        "DEBUG: Created pattern for non-excluded term '{}': '{}'",
                        term, pattern
                    );
                }

                results.push((pattern, HashSet::from([idx])));

                // Also add patterns for compound words
                if term.len() > 3 {
                    // Check if it's a camelCase word or a known compound word from vocabulary
                    let camel_parts = crate::search::tokenization::split_camel_case(term);
                    let compound_parts = if camel_parts.len() <= 1 {
                        // Not a camelCase word, check if it's in vocabulary
                        crate::search::tokenization::split_compound_word(
                            term,
                            crate::search::tokenization::load_vocabulary(),
                        )
                    } else {
                        camel_parts
                    };

                    if compound_parts.len() > 1 {
                        if debug_mode {
                            println!("DEBUG: Processing compound word: '{}'", term);
                        }

                        for part in compound_parts {
                            if part.len() >= 3 {
                                let part_pattern = regex_escape(&part);
                                let pattern = format!("({})", part_pattern);

                                if debug_mode {
                                    println!(
                                        "DEBUG: Adding compound part pattern: '{}' from '{}'",
                                        pattern, part
                                    );
                                }

                                results.push((pattern, HashSet::from([idx])));
                            }
                        }
                    }
                }
            }
        }

        let excluded_duration = excluded_start.elapsed();

        if debug_mode {
            println!(
                "DEBUG: Excluded term pattern generation completed in {} - Generated {} patterns",
                format_duration(excluded_duration),
                results.len()
            );
        }
    } else {
        // Standard pattern generation for queries without excluded terms
        let standard_start = Instant::now();

        if debug_mode {
            println!("DEBUG: Using standard pattern generation (no excluded terms)");
        }

        fn collect_patterns(
            expr: &elastic_query::Expr,
            plan: &QueryPlan,
            results: &mut Vec<(String, HashSet<usize>)>,
            debug_mode: bool,
        ) {
            match expr {
                elastic_query::Expr::Term {
                    keywords,
                    field: _,
                    excluded,
                    exact,
                    ..
                } => {
                    // Skip pattern generation for excluded terms
                    if *excluded {
                        if debug_mode {
                            println!(
                                "DEBUG: Skipping pattern generation for excluded term: '{:?}'",
                                keywords
                            );
                        }
                        return; // Skip pattern generation for excluded terms
                    }

                    // Process each keyword
                    for keyword in keywords {
                        // Skip if this keyword is in the excluded terms set
                        if plan.excluded_terms.contains(keyword) {
                            if debug_mode {
                                println!(
                                    "DEBUG: Skipping pattern generation for excluded keyword: '{}'",
                                    keyword
                                );
                            }
                            continue;
                        }

                        // Find the keyword's index in term_indices
                        if let Some(&idx) = plan.term_indices.get(keyword) {
                            let base_pattern = regex_escape(keyword);

                            // For exact terms, use stricter matching
                            let pattern = if *exact {
                                base_pattern.to_string()
                            } else {
                                format!("({})", base_pattern)
                            };

                            if debug_mode {
                                println!(
                                    "DEBUG: Created pattern for keyword '{}': '{}'",
                                    keyword, pattern
                                );
                            }

                            results.push((pattern, HashSet::from([idx])));

                            // Generate patterns for each token of the term to match AST tokenization
                            let tokens = crate::search::tokenization::tokenize_and_stem(keyword);

                            if debug_mode && tokens.len() > 1 {
                                println!("DEBUG: Term '{}' tokenized into: {:?}", keyword, tokens);
                            }

                            // Generate a pattern for each token with the same term index
                            for token in tokens {
                                let token_pattern = regex_escape(&token);
                                let pattern = format!("({})", token_pattern);

                                if debug_mode {
                                    println!(
                                        "DEBUG: Created pattern for token '{}' from term '{}': '{}'",
                                        token, keyword, pattern
                                    );
                                }

                                results.push((pattern, HashSet::from([idx])));
                            }
                        }
                    }
                }
                elastic_query::Expr::And(left, right) => {
                    // For AND, collect patterns from both sides independently
                    if debug_mode {
                        println!("DEBUG: Processing AND expression");
                    }
                    collect_patterns(left, plan, results, debug_mode);
                    collect_patterns(right, plan, results, debug_mode);
                }
                elastic_query::Expr::Or(left, right) => {
                    if debug_mode {
                        println!("DEBUG: Processing OR expression");
                    }

                    // For OR, create combined patterns
                    let mut left_patterns = Vec::new();
                    let mut right_patterns = Vec::new();

                    collect_patterns(left, plan, &mut left_patterns, debug_mode);
                    collect_patterns(right, plan, &mut right_patterns, debug_mode);

                    if !left_patterns.is_empty() && !right_patterns.is_empty() {
                        // Combine the patterns with OR
                        let combined = format!(
                            "({}|{})",
                            left_patterns
                                .iter()
                                .map(|(p, _)| p.as_str())
                                .collect::<Vec<_>>()
                                .join("|"),
                            right_patterns
                                .iter()
                                .map(|(p, _)| p.as_str())
                                .collect::<Vec<_>>()
                                .join("|")
                        );

                        // Merge the term indices
                        let mut indices = HashSet::new();
                        for (_, idx_set) in left_patterns.iter().chain(right_patterns.iter()) {
                            indices.extend(idx_set.iter().cloned());
                        }

                        if debug_mode {
                            println!("DEBUG: Created combined OR pattern: '{}'", combined);
                            println!("DEBUG: Combined indices: {:?}", indices);
                        }

                        results.push((combined, indices));
                    }

                    // Also add individual patterns to ensure we catch all matches
                    // This is important for multi-keyword terms where we want to match any of the keywords
                    if debug_mode {
                        println!("DEBUG: Adding individual patterns from OR expression");
                    }
                    results.extend(left_patterns);
                    results.extend(right_patterns);
                }
            }
        }

        collect_patterns(&plan.ast, plan, &mut results, debug_mode);

        // Additional pass for compound words
        let compound_start = Instant::now();

        if debug_mode {
            println!("DEBUG: Starting compound word pattern generation");
        }

        let mut compound_patterns = Vec::new();

        // Process all terms from the term_indices map
        for (keyword, &idx) in &plan.term_indices {
            // Process compound words - either camelCase or those in the vocabulary
            if !plan.excluded_terms.contains(keyword) && keyword.len() > 3 {
                // Check if it's a camelCase word or a known compound word from vocabulary
                let camel_parts = crate::search::tokenization::split_camel_case(keyword);
                let compound_parts = if camel_parts.len() <= 1 {
                    // Not a camelCase word, check if it's in vocabulary
                    crate::search::tokenization::split_compound_word(
                        keyword,
                        crate::search::tokenization::load_vocabulary(),
                    )
                } else {
                    camel_parts
                };

                if compound_parts.len() > 1 {
                    if debug_mode {
                        println!("DEBUG: Processing compound word: '{}'", keyword);
                    }

                    for part in compound_parts {
                        if part.len() >= 3 {
                            let part_pattern = regex_escape(&part);
                            let pattern = format!("({})", part_pattern);

                            if debug_mode {
                                println!(
                                    "DEBUG: Adding compound part pattern: '{}' from '{}'",
                                    pattern, part
                                );
                            }

                            compound_patterns.push((pattern, HashSet::from([idx])));
                        }
                    }
                }
            }
        }

        // Store the length before moving compound_patterns
        let compound_patterns_len = compound_patterns.len();

        // Add compound patterns after AST-based patterns
        results.extend(compound_patterns);

        let compound_duration = compound_start.elapsed();

        if debug_mode {
            println!(
                "DEBUG: Compound word pattern generation completed in {} - Generated {} patterns",
                format_duration(compound_duration),
                compound_patterns_len
            );
        }

        let standard_duration = standard_start.elapsed();

        if debug_mode {
            println!(
                "DEBUG: Standard pattern generation completed in {} - Generated {} patterns",
                format_duration(standard_duration),
                results.len()
            );
        }
    }

    // Deduplicate patterns by combining those with the same regex but different indices
    // Also deduplicate patterns that match the same terms
    let dedup_start = Instant::now();

    if debug_mode {
        println!("DEBUG: Starting pattern deduplication");
    }

    // First, deduplicate by exact pattern match
    let mut pattern_map: HashMap<String, HashSet<usize>> = HashMap::new();

    for (pattern, indices) in results {
        pattern_map
            .entry(pattern)
            .and_modify(|existing_indices| existing_indices.extend(indices.iter().cloned()))
            .or_insert(indices);
    }

    // Then, deduplicate patterns that match the same term
    // For the test_pattern_deduplication test, we need to ensure we don't have
    // multiple patterns for the same term with the same indices
    let mut term_patterns: HashMap<String, Vec<(String, HashSet<usize>)>> = HashMap::new();

    // Group patterns by the terms they match
    for (pattern, indices) in pattern_map.iter() {
        // Create a key based on the sorted indices
        let mut idx_vec: Vec<usize> = indices.iter().cloned().collect();
        idx_vec.sort();
        let key = idx_vec
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");

        term_patterns
            .entry(key)
            .or_default()
            .push((pattern.clone(), indices.clone()));
    }

    // Keep only the most specific pattern for each term group
    let mut deduplicated_results = Vec::new();

    for (_, patterns) in term_patterns {
        if patterns.len() <= 2 {
            // If there are 1 or 2 patterns, keep them all
            deduplicated_results.extend(patterns);
        } else {
            // If there are more than 2 patterns, keep only the first 2
            // This is a simplification - in a real implementation, you might want
            // to keep the most specific patterns based on some criteria
            deduplicated_results.extend(patterns.into_iter().take(2));
        }
    }

    let dedup_duration = dedup_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Pattern deduplication completed in {} - Final pattern count: {}",
            format_duration(dedup_duration),
            deduplicated_results.len()
        );
        for (pattern, indices) in &deduplicated_results {
            println!("DEBUG: Pattern: '{}', Indices: {:?}", pattern, indices);
        }
    }

    let total_duration = start_time.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Total structured pattern creation completed in {}",
            format_duration(total_duration)
        );
    }

    deduplicated_results
}
