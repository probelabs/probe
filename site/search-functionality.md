# Search Functionality Reference

Complete reference documentation for Probe's search capabilities, including query syntax, ranking algorithms, and advanced search techniques.

## SEARCH COMMAND

```bash
probe search <QUERY> [PATH] [OPTIONS]
```

### CORE PARAMETERS

| Parameter | Description |
|-----------|-------------|
| `<QUERY>` | **Required**: Search terms or expression |
| `[PATH]` | Directory to search (defaults to current directory) |

### KEY OPTIONS

| Option | Description | Default |
|--------|-------------|---------|
| `--files-only` | List matching files without code blocks | Off |
| `--ignore <PATTERN>` | Additional patterns to ignore | None |
| `--exclude-filenames, -n` | Exclude filenames from matching | Off |
| `--reranker, -r <TYPE>` | Ranking algorithm: `hybrid`, `hybrid2`, `bm25`, `tfidf` | `hybrid` |
| `--frequency, -s` | Enable smart token matching | On |
| `--max-results <N>` | Limit number of results | No limit |
| `--max-bytes <N>` | Limit total bytes of code returned | No limit |
| `--max-tokens <N>` | Limit total tokens | No limit |
| `--allow-tests` | Include test files and code | Off |
| `--any-term` | Match any search term (OR logic) | Off |
| `--no-merge` | Keep code blocks separate | Off |
| `--merge-threshold <N>` | Max lines between blocks to merge | 5 |
| `--session <ID>` | Session ID for caching results | None |
| `--format <TYPE>` | Output format: `color`, `plain`, `markdown`, `json` | `color` |

For complete option details, see `probe search --help`.

## QUERY PROCESSING

Probe enhances search queries through several techniques:

### TOKENIZATION

Breaks down terms into tokens:
```
findUserByEmail → [find, user, by, email]
```

### STEMMING

Reduces words to their root form:
```
implementing, implementation → implement
```

### SMART PATTERN GENERATION

- **Term Boundaries**: Understands where code tokens start/end
- **Case Handling**: Works with camelCase, snake_case, etc.
- **Compound Handling**: Breaks down compound terms

## QUERY SYNTAX

Probe supports an Elasticsearch-like query syntax:

### BASIC TERMS

```bash
probe search "authentication"  # Single term
probe search "user authentication"  # Multiple terms (AND logic)
```

### BOOLEAN OPERATORS

```bash
probe search "error AND handling"  # Require both terms
probe search "login OR authentication"  # Match either term
probe search "database NOT sqlite"  # Exclude term
```

### GROUPING

```bash
probe search "(error OR exception) AND (handle OR process)"
```

### TERM MODIFIERS

```bash
probe search "+authentication login"  # Required term
probe search "database -sqlite"  # Excluded term
probe search "\"handle error\""  # Exact phrase
```

### FIELD SPECIFIERS

```bash
probe search "function:authenticate"  # Search in function names
```

### WILDCARDS

```bash
probe search "auth*"  # Matches "auth", "authentication", "authorize", etc.
```

## RANKING ALGORITHMS

Probe uses sophisticated algorithms to rank search results:

### TF-IDF RANKING

Term Frequency-Inverse Document Frequency balances how often terms appear in a specific code block against how common they are across the codebase.

#### HOW IT WORKS

1. **Term Frequency (TF)**: How often a term appears in a code block
   ```
   TF(term, block) = (Number of times term appears in block) / (Total number of terms in block)
   ```

2. **Inverse Document Frequency (IDF)**: Measures how unique or rare a term is
   ```
   IDF(term) = ln(Total number of blocks / Number of blocks containing term)
   ```

3. **TF-IDF Score**: Combines these factors
   ```
   TF-IDF(term, block) = TF(term, block) * IDF(term)^2
   ```

Key benefits:
- Rewards matches on rare, important terms
- Penalizes common terms that appear everywhere
- Considers term frequency within each code block

### BM25 RANKING

BM25 (Best Matching 25) is an improved version of TF-IDF that addresses some of its limitations.

#### HOW IT WORKS

```
BM25(block, query) = ∑ IDF(term) * (TF(term, block) * (k1 + 1)) / (TF(term, block) + k1 * (1 - b + b * (block_length / average_block_length)))
```

Where:
- `k1` (1.2): Controls term frequency saturation
- `b` (0.75): Controls length normalization

Key benefits:
- Better handling of document length
- Diminishing returns for repeated terms
- More accurate for longer code blocks
- Improved handling of edge cases

### HYBRID RANKING

Probe's default ranking algorithm combines multiple signals for superior results.

#### HOW IT WORKS

The hybrid algorithm considers:

1. **Combined score**: Weighted combination of TF-IDF and BM25
   ```
   Combined = α * TF-IDF + (1-α) * BM25
   ```

2. **Position weights**: Terms in function names, class names, and identifiers receive higher scores

3. **Block metrics**:
   - Number of unique terms matched
   - Total matches in the block
   - Block type (methods score higher than comments)

4. **File metrics**:
   - File match rank
   - Number of unique terms in the file
   - Total matches in the file

Key benefits:
- More balanced scoring across different code structures
- Better handling of both short and long code blocks
- Prioritizes meaningful code over comments or boilerplate

### HYBRID2 RANKING

An enhanced version of the hybrid algorithm with improved relevance:
- Better normalization of scores across different metrics
- Enhanced weighting for structural elements
- Improved handling of term proximity
- More sophisticated position weighting

## PRACTICAL EXAMPLES

### FINDING ERROR HANDLING CODE

```bash
probe search "error handling try catch"
```

This search:
1. Tokenizes to: ["error", "handl", "try", "catch"]
2. Matches files containing these terms
3. Ranks results based on term frequency and importance
4. Returns complete code blocks with error handling logic

### SEARCHING FOR AUTHENTICATION FLOWS

```bash
probe search "(login OR authenticate) AND (user OR account) NOT test"
```

This complex query:
1. Finds code with either "login" or "authenticate"
2. Requires either "user" or "account" to be present
3. Excludes results containing "test"
4. Returns ranked, complete code blocks

### FINDING SPECIFIC API ENDPOINTS

```bash
probe search "function:create* api endpoint"
```

This search:
1. Targets functions starting with "create"
2. Requires "api" and "endpoint" terms
3. Returns complete function definitions
4. Ranks results with the most relevant endpoints first

### LIMITING RESULTS FOR AI INTEGRATION

```bash
probe search "database connection pool" --max-tokens 4000 --format json
```

This search:
1. Finds code related to database connection pools
2. Limits results to fit within 4000 tokens
3. Returns JSON-formatted output suitable for AI processing

## PERFORMANCE TIPS

- **Be specific**: More specific queries yield more relevant results
- **Use field specifiers**: Target specific code elements with `function:`, `class:`, etc.
- **Leverage boolean operators**: Combine terms with AND, OR, NOT for precision
- **Control result size**: Use `--max-results`, `--max-bytes`, or `--max-tokens` for large codebases
- **Session caching**: Use `--session` to avoid seeing the same code blocks repeatedly
- **Experiment with rankers**: Try different ranking algorithms for different types of searches

For more information on how Probe works internally, see [How Probe Works](how-it-works.md).
For details on code extraction, see [Code Extraction](code-extraction.md).