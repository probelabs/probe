// Structure to hold both limited search results and skipped files
#[derive(Debug)]
pub struct LimitedSearchResults {
    pub results: Vec<SearchResult>,
    pub skipped_files: Vec<SearchResult>,
    pub limits_applied: Option<SearchLimits>,
}

// Structure to track which limits were applied
#[derive(Debug)]
pub struct SearchLimits {
    pub max_results: Option<usize>,
    pub max_bytes: Option<usize>,
    pub max_tokens: Option<usize>,
    #[allow(dead_code)]
    pub total_bytes: usize,
    #[allow(dead_code)]
    pub total_tokens: usize,
}

// Structure to hold search results
#[derive(Debug)]
pub struct SearchResult {
    pub file: String,
    pub lines: (usize, usize),
    pub node_type: String,
    pub code: String,
    // Indicates if this result was found by filename matching
    #[allow(dead_code)]
    pub matched_by_filename: Option<bool>,
    // Ranking information
    pub rank: Option<usize>,
    // Combined score from the ranking algorithm
    pub score: Option<f64>,
    // Individual TF-IDF score
    pub tfidf_score: Option<f64>,
    // Individual BM25 score
    pub bm25_score: Option<f64>,
    // TF-IDF rank (1 is most relevant)
    pub tfidf_rank: Option<usize>,
    // BM25 rank (1 is most relevant)
    pub bm25_rank: Option<usize>,
    // Number of distinct search terms matched in the file
    pub file_unique_terms: Option<usize>,
    // Total count of matches across the file (content + filename matches)
    pub file_total_matches: Option<usize>,
    // Rank of the file based on total matches
    pub file_match_rank: Option<usize>,
    // Number of unique search terms matched in the block
    pub block_unique_terms: Option<usize>,
    // Total frequency of term matches in the block
    pub block_total_matches: Option<usize>,
}

// Structure to hold node information for merging
#[derive(Clone)]
pub struct CodeBlock {
    pub start_row: usize,
    pub end_row: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub node_type: String,
}
