# THE TECH BEHIND PROBE

Probe combines raw speed with code intelligence to find what matters. Here's how.

## SYSTEM ARCHITECTURE

Probe's core technology stack operates in six stages:

1. **SCAN**: Lightning-fast file identification with ripgrep
2. **PARSE**: Code structure understanding via Abstract Syntax Trees
3. **PROCESS**: Query enhancement with NLP techniques
4. **RANK**: Intelligent result prioritization
5. **EXTRACT**: Complete code block isolation
6. **FORMAT**: Clean, usable output generation

## RAPID SCANNING

The foundation of Probe's speed:

- **RIPGREP ENGINE**: Blazing-fast line scanning at core
- **PARALLEL PROCESSING**: Utilizes all CPU cores
- **SMART FILTERING**: Respects .gitignore patterns
- **STREAM PROCESSING**: Minimal memory footprint

## CODE STRUCTURE PARSING

Where Probe becomes more than just text search:

- **TREE-SITTER**: Industry-standard parsing tech
- **AST GENERATION**: Builds complete code structure map
- **LANGUAGE-SPECIFIC**: Understands each language's unique patterns
- **ROBUST HANDLING**: Works with partial or imperfect code

## QUERY INTELLIGENCE

Transforming basic searches into powerful queries:

### Tokenization

```
findUserByEmail → [find, user, by, email]
```

### Stemming

```
implementing, implementation → implement
```

### Smart Pattern Generation

- **TERM BOUNDARIES**: Understands where code tokens start/end
- **CASE HANDLING**: Works with camelCase, snake_case, etc.
- **COMPOUND HANDLING**: Breaks down compound terms intelligently

## ADVANCED RANKING

The algorithms that put the right code on top:

### TF-IDF Ranking

- **TERM FREQUENCY**: How often terms appear in a block
- **DOCUMENT FREQUENCY**: How common terms are across codebase
- **BALANCING**: Rewards rare, important terms

### BM25 Ranking

- **LENGTH NORMALIZATION**: Adjusts for code block size
- **SATURATION CONTROL**: Diminishing value for repeated terms
- **TUNABLE PARAMETERS**: k1 and b control ranking behavior

### Hybrid Approach

- **MULTI-SIGNAL**: Combines ranking algorithms
- **POSITION WEIGHTS**: Values terms in function names higher
- **NORMALIZED SCORING**: Fair comparison between methods

## BLOCK EXTRACTION

Isolating exactly the code you need:

- **NODE TARGETING**: Finds smallest complete code unit
- **FUNCTION DETECTION**: Extracts entire methods/functions
- **CLASS RECOGNITION**: Pulls complete class definitions
- **CONTEXT PRESERVATION**: Ensures code makes sense in isolation

## OUTPUT STRATEGIES

Delivering results in the most useful format:

- **MARKDOWN/SYNTAX**: Rich, readable code presentation
- **JSON**: Structured for programmatic use
- **TOKEN LIMITING**: Fits within AI context windows
- **PRIORITY HANDLING**: Most relevant results survive limits

## SEARCH FLOW EXAMPLE

Here's how a search for "error handling" works:

1. **QUERY ENHANCEMENT**:
   - Tokenize → [error, handling]
   - Stem → [error, handl]
   - Generate patterns → `\berror\b`, `\bhandl\w*\b`

2. **FILE SCAN**:
   - Ripgrep finds all potential matches
   - Builds initial candidate list

3. **STRUCTURAL ANALYSIS**:
   - Parse matching files into ASTs
   - Identify complete code blocks containing matches

4. **RESULT RANKING**:
   - Calculate relevance scores via TF-IDF/BM25
   - Sort by combined relevance metrics

5. **BLOCK EXTRACTION**:
   - Pull complete functions/methods with matches
   - Merge closely related code blocks

6. **RESULT DELIVERY**:
   - Format with syntax highlighting
   - Apply any token/size constraints
   - Deliver precisely what you need
