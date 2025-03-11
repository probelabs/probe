use crate::ranking;
use crate::search::elastic_query;
// No term_exceptions import needed
use crate::search::tokenization::{
    is_english_stop_word, load_vocabulary, split_camel_case, split_compound_word, tokenize,
};
use std::collections::{HashMap, HashSet};

/// Existing function for preprocessing a query into (original, stemmed) pairs
/// This remains, but we add additional logic below for AST-based usage.
pub fn preprocess_query(query: &str, exact: bool) -> Vec<(String, String)> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("Original query: {}", query);
    }

    // Try parsing with elastic query parser first to handle negated terms
    let excluded_terms = match elastic_query::parse_query_compat(query) {
        Ok(ast) => {
            if debug_mode {
                println!("Elastic query parsed successfully: {:?}", ast);
            }

            // Extract excluded terms from the AST
            let mut excluded = HashSet::new();
            extract_excluded_terms(&ast, &mut excluded);

            if debug_mode && !excluded.is_empty() {
                println!("Excluded terms from query: {:?}", excluded);
            }

            excluded
        }
        Err(e) => {
            if debug_mode {
                println!("Elastic query parse error: {:?}", e);
            }
            HashSet::new()
        }
    };

    if exact {
        let result = query
            .to_lowercase()
            .split_whitespace()
            .filter(|word| !word.is_empty())
            .filter(|word| !word.starts_with('-') && !excluded_terms.contains(*word))
            .map(|word| {
                let original = word.to_string();
                (original.clone(), original)
            })
            .collect();

        if debug_mode {
            println!("Exact mode result: {:?}", result);
        }
        result
    } else {
        let stemmer = ranking::get_stemmer();
        let vocabulary = load_vocabulary();

        let mut enhanced_vocab = vocabulary.clone();
        for term in [
            "rpc", "storage", "handler", "client", "server", "api", "service",
        ] {
            enhanced_vocab.insert(term.to_string());
        }

        let mut result = query
            .split_whitespace()
            .filter(|word| !word.starts_with('-'))
            .flat_map(|word| {
                if debug_mode {
                    println!("Processing word: {}", word);
                }

                let camel_parts = split_camel_case(word);
                if debug_mode {
                    println!("After camel case split: {:?}", camel_parts);
                }

                camel_parts
                    .into_iter()
                    .flat_map(|part| {
                        let compound_parts = split_compound_word(&part, &enhanced_vocab);
                        if debug_mode {
                            println!("After compound split of '{}': {:?}", part, compound_parts);
                        }
                        compound_parts
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|word| {
                !word.is_empty() && !is_english_stop_word(word) && !excluded_terms.contains(word)
            })
            .map(|part| {
                let original = part.clone();
                let stemmed = stemmer.stem(&part).to_string();
                if debug_mode {
                    println!("Term: {} (stemmed to {})", original, stemmed);
                }
                (original, stemmed)
            })
            .collect::<Vec<_>>();

        // Only apply tokenization if we couldn't parse the query with the elastic query parser
        // This ensures that terms with underscores are preserved
        if excluded_terms.is_empty() {
            let tokens = tokenize(query);

            if debug_mode {
                println!("After tokenization: {:?}", tokens);
            }

            for token in tokens {
                if excluded_terms.contains(&token) {
                    continue;
                }

                let already_exists = result.iter().any(|(_, stemmed)| stemmed == &token);

                if !already_exists {
                    result.push((token.clone(), token));
                }
            }
        } else if debug_mode {
            println!(
                "Skipping additional tokenization as query was parsed by elastic query parser"
            );
        }

        if debug_mode {
            println!("Final preprocessed terms: {:?}", result);
        }
        result
    }
}

/// Helper function to extract excluded terms from the AST
pub fn extract_excluded_terms(expr: &elastic_query::Expr, excluded: &mut HashSet<String>) {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!(
            "DEBUG: Extracting excluded terms from expression: {:?}",
            expr
        );
    }

    match expr {
        elastic_query::Expr::Term {
            keywords,
            field: _,
            excluded: is_excluded,
            ..
        } => {
            if debug_mode {
                println!(
                    "DEBUG: Checking term '{:?}', excluded={}",
                    keywords, is_excluded
                );
            }

            if *is_excluded {
                for keyword in keywords {
                    if debug_mode {
                        println!("DEBUG: Found excluded keyword: '{}'", keyword);
                    }

                    // Add the original keyword
                    excluded.insert(keyword.clone());

                    // For negative terms, we don't apply compound word splitting
                    if debug_mode {
                        println!(
                            "DEBUG: Not applying compound word splitting to excluded keyword '{}'",
                            keyword
                        );
                    }
                }
            }
        }
        elastic_query::Expr::And(left, right) => {
            if debug_mode {
                println!("DEBUG: Processing AND expression");
            }
            extract_excluded_terms(left, excluded);
            extract_excluded_terms(right, excluded);
        }
        elastic_query::Expr::Or(left, right) => {
            if debug_mode {
                println!("DEBUG: Processing OR expression");
            }
            extract_excluded_terms(left, excluded);
            extract_excluded_terms(right, excluded);
        }
    }

    if debug_mode {
        println!("DEBUG: Current excluded terms set: {:?}", excluded);
    }
}

/// Existing function creating regex patterns for (original, stemmed) pairs.
/// We'll keep it for backward-compat. For complex queries, we'll create specialized patterns separately.
pub fn create_term_patterns(term_pairs: &[(String, String)]) -> Vec<(String, HashSet<usize>)> {
    let mut patterns: Vec<(String, HashSet<usize>)> = Vec::new();

    for (term_idx, (original, stemmed)) in term_pairs.iter().enumerate() {
        let base_pattern = if original == stemmed
            || (stemmed.len() < original.len() && original.starts_with(stemmed))
        {
            regex_escape(original)
        } else if original.len() < stemmed.len() && stemmed.starts_with(original) {
            regex_escape(stemmed)
        } else {
            regex_escape(original)
        };

        let term_indices = HashSet::from([term_idx]);
        let pattern = format!("(\\b{}|{}\\b)", base_pattern, base_pattern);
        patterns.push((pattern, term_indices.clone()));
    }

    // Concatenate combos, etc. (already existing logic)...

    if term_pairs.len() > 1 {
        let terms: Vec<(String, usize)> = term_pairs
            .iter()
            .enumerate()
            .flat_map(|(term_idx, (o, s))| vec![(o.clone(), term_idx), (s.clone(), term_idx)])
            .collect();

        use itertools::Itertools;
        let mut concatenated_patterns: std::collections::HashMap<Vec<usize>, Vec<String>> =
            std::collections::HashMap::new();

        for perm in terms.iter().permutations(2).unique() {
            let term_indices: Vec<usize> = perm.iter().map(|(_, idx)| *idx).collect();
            if term_indices.iter().unique().count() < 2 {
                continue;
            }
            let concatenated = perm
                .iter()
                .map(|(term, _)| regex_escape(term))
                .collect::<String>();

            concatenated_patterns
                .entry(term_indices)
                .or_default()
                .push(concatenated);
        }

        for (term_indices, pattern_group) in concatenated_patterns {
            if pattern_group.len() == 1 {
                patterns.push((pattern_group[0].clone(), term_indices.into_iter().collect()));
            } else {
                let combined_pattern = format!("({})", pattern_group.join("|"));
                patterns.push((combined_pattern, term_indices.into_iter().collect()));
            }
        }
    }

    patterns
}

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
pub struct QueryPlan {
    pub ast: elastic_query::Expr,
    pub term_indices: HashMap<String, usize>,
    pub excluded_terms: HashSet<String>,
}

/// Create a QueryPlan from a raw query string. This fully parses the query into an AST,
/// then extracts all terms (including excluded), and prepares a term-index map.
pub fn create_query_plan(
    query: &str,
    _exact: bool,
) -> Result<QueryPlan, elastic_query::ParseError> {
    // Parse the query into an AST with processed terms
    // We always use AND for implicit combinations (space-separated terms)
    let ast = elastic_query::parse_query(query, false)?;

    // We'll walk the AST to build a set of all terms. We track excluded as well for reference.
    let mut all_terms = Vec::new();
    let mut excluded_terms = HashSet::new();
    collect_all_terms(&ast, &mut all_terms, &mut excluded_terms);

    // Remove duplicates from all_terms
    all_terms.sort();
    all_terms.dedup();

    // Build term index map
    let mut term_indices = HashMap::new();
    for (i, term) in all_terms.iter().enumerate() {
        term_indices.insert(term.clone(), i);
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

/// Generate regex patterns that respect the AST's logical structure.
/// This creates composite patterns for OR groups while keeping AND terms separate.
pub fn create_structured_patterns(plan: &QueryPlan) -> Vec<(String, HashSet<usize>)> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    let mut results = Vec::new();

    if debug_mode {
        println!("DEBUG: Creating structured patterns with AST awareness");
        println!("DEBUG: AST: {:?}", plan.ast);
        println!("DEBUG: Excluded terms: {:?}", plan.excluded_terms);
    }

    // Special handling for queries with excluded terms
    if !plan.excluded_terms.is_empty() {
        if debug_mode {
            println!("DEBUG: Query has excluded terms, using special pattern generation");
        }

        // For queries with excluded terms, we need to ensure we generate patterns
        // for all non-excluded terms, even if they're part of a complex expression
        for (term, &idx) in &plan.term_indices {
            if !plan.excluded_terms.contains(term) {
                let base_pattern = regex_escape(term);
                // Use more flexible pattern matching to ensure we catch all occurrences
                let pattern = format!("(\\b{}|{}\\b|{})", base_pattern, base_pattern, base_pattern);

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
                                let pattern = format!("(\\b{}|{}\\b)", part_pattern, part_pattern);

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
    } else {
        // Standard pattern generation for queries without excluded terms
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
                            let pattern = format!("(\\b{}|{}\\b)", base_pattern, base_pattern);

                            if debug_mode {
                                println!(
                                    "DEBUG: Created pattern for keyword '{}': '{}'",
                                    keyword, pattern
                                );
                            }

                            results.push((pattern, HashSet::from([idx])));
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
                            let pattern = format!("(\\b{}|{}\\b)", part_pattern, part_pattern);

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

        // Add compound patterns after AST-based patterns
        results.extend(compound_patterns);
    }

    // Deduplicate patterns by combining those with the same regex but different indices
    let mut pattern_map: HashMap<String, HashSet<usize>> = HashMap::new();

    for (pattern, indices) in results {
        pattern_map
            .entry(pattern)
            .and_modify(|existing_indices| existing_indices.extend(indices.iter().cloned()))
            .or_insert(indices);
    }

    let deduplicated_results: Vec<(String, HashSet<usize>)> = pattern_map.into_iter().collect();

    if debug_mode {
        println!(
            "DEBUG: Final pattern count after deduplication: {}",
            deduplicated_results.len()
        );
        for (pattern, indices) in &deduplicated_results {
            println!("DEBUG: Pattern: '{}', Indices: {:?}", pattern, indices);
        }
    }

    deduplicated_results
}
