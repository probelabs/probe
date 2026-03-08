# How Probe Compares to Other Code Search Tools

Probe occupies a unique position in the code search landscape. This page explains how it differs from other approaches and when each tool is the right choice.

## The Three Approaches to Code Search

Code search tools generally fall into three camps:

1. **Text-based** (grep, ripgrep) -- fast regex matching, no code understanding
2. **Embedding-based** (grepai, Octocode) -- vector similarity search, requires indexing + embedding model
3. **AST-aware + keyword** (Probe) -- structural code understanding with boolean keyword search, zero setup

Probe takes the third path. It uses tree-sitter to understand code structure (returning complete functions, classes, and structs), combined with Elasticsearch-style boolean queries and BM25 ranking. No indexing, no embedding model, no external services.

## Detailed Comparison

### Probe vs Embedding-Based Search (grepai, Octocode)

Tools like [grepai](https://github.com/yoanbernabeu/grepai) and [Octocode](https://github.com/Muvon/octocode) convert code into vector embeddings and use cosine similarity to find semantically similar code.

**Their advantage:** Natural language queries work without exact keyword matches. Searching "authentication flow" can find code named `verify_credentials`.

**Why Probe doesn't need embeddings:** When an AI agent uses Probe, the LLM is the semantic layer. It translates natural language into precise keyword queries:

```
User: "find the authentication logic"
  -> LLM generates: probe search "verify_credentials OR authenticate OR login OR auth_handler"
  -> Probe: SIMD-accelerated matching, complete AST blocks, milliseconds
```

| | Embedding tools | Probe |
|---|---|---|
| Setup | Minutes (indexing + embedding API) | Zero |
| Result unit | ~512-char text chunks (can split mid-function) | Complete AST blocks (functions, classes, structs) |
| External deps | Ollama, OpenAI, or cloud embedding API | None |
| Search latency | 100ms+ (embedding + vector lookup) | Milliseconds (SIMD pattern matching) |
| Determinism | Varies with model/index state | Same query = same results |
| Index maintenance | Re-index on code changes (or risk stale results) | No index (always current) |
| Best for | Human users typing natural language | AI agents generating precise boolean queries |

### Probe vs Code Knowledge Graphs (Stakgraph, ABCoder)

Tools like [Stakgraph](https://github.com/stakwork/stakgraph) and [ABCoder](https://github.com/cloudwego/abcoder) build structural representations of codebases -- call graphs, dependency edges, type hierarchies.

**Their advantage:** They answer structural questions: "Who calls this function?", "What implements this interface?", "What's the shortest path between these two modules?"

**Probe's advantage:** Zero setup, instant search, AST-aware output optimized for LLM consumption.

| | Graph tools | Probe |
|---|---|---|
| Call graph | Yes (function-level edges) | Planned (via LSP integration) |
| Dependency analysis | Yes (typed relationships) | Not yet |
| Code search | Limited (node name lookup) | Full-featured (boolean queries, BM25, ranking) |
| Setup | Heavy (Neo4j, batch parsing, LSP servers) | Zero |
| Token awareness | Limited | Built-in (`--max-tokens`, session dedup) |
| Real-time | Requires rebuild on changes | Always current (stateless) |

These tools are complementary. Probe finds code; graph tools map relationships.

### Probe vs LSP-Based Tools (Crabviz)

[Crabviz](https://github.com/chanhx/crabviz) uses Language Server Protocol to build interactive call graph visualizations in VS Code.

**Their advantage:** Works with any language that has an LSP server (~60+). Beautiful interactive SVG visualizations.

**Probe's advantage:** Works outside VS Code, has search capabilities, and integrates with AI agents.

| | Crabviz | Probe |
|---|---|---|
| Environment | VS Code only | CLI, MCP, SDK, any editor |
| Call graph | Yes (via LSP) | Planned |
| Search | None | Full-featured |
| AI integration | None | Full agent loop + MCP |
| Visualization | Interactive SVG with pan/zoom | Text-based outline format |

### Probe vs grep/ripgrep

| | grep/ripgrep | Probe |
|---|---|---|
| Speed | Fast | Fast (uses ripgrep + SIMD) |
| Code understanding | None (text only) | AST-aware (tree-sitter) |
| Result unit | Lines | Complete functions/classes |
| Query language | Regex | Elasticsearch-style boolean |
| Ranking | None (file order) | BM25, TF-IDF, Hybrid |
| AI integration | None | MCP, SDK, built-in agent |
| Token limits | None | `--max-tokens`, session dedup |

## Probe's Design Philosophy

1. **Zero setup, instant results.** No indexing, no embedding models, no databases. Clone a repo, search immediately.

2. **The LLM is the semantic layer.** Instead of building an embedding index for natural language queries, Probe gives the LLM a powerful query language and lets it generate precise searches. This is faster, cheaper, and more deterministic.

3. **Code is code, not text.** Every result is a complete AST block -- a full function, class, or struct. Never a broken text chunk that splits a function in half.

4. **Token-aware by design.** `--max-tokens` enforces budgets. Session dedup prevents repeating previously returned blocks. Output formats are optimized for LLM consumption.

5. **Deterministic and reproducible.** No model variance, no stale indexes, no non-deterministic similarity scores. Same query always returns the same results.

## When to Use What

| Scenario | Best tool |
|----------|-----------|
| AI agent needs code context, any repo, instantly | **Probe** |
| Human searching code with natural language, doesn't know the terms | Embedding tool (grepai, Octocode) |
| "Who calls this function?" / "What implements this interface?" | Graph tool (Stakgraph) or Probe with LSP (coming soon) |
| Visualize call graph of a module | Crabviz |
| Give an LLM structured code context with minimal tokens | **Probe** or ABCoder |
| AI-assisted git workflow (commit, review, release) | Octocode |
| Simple text search in terminal | ripgrep |
