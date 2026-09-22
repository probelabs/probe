# Probe Server Plan

## Goal

Build `Probe Server` as a company-wide code answering system that can:

- connect to GitHub, GitLab, and other code hosts
- understand a large multi-repo corpus
- answer architecture and implementation questions across the organization
- expose that knowledge over web, API, and MCP
- preserve Probe's strengths in precise code extraction, token-aware context assembly, and optional deep semantic analysis

The key design constraint is that Probe should not become "just a centralized search UI." The server should use global retrieval for breadth, but keep Probe as the depth engine.

## Product Position

Probe today is strongest as:

- a local context engine
- an AST-aware extraction tool
- an agent-oriented code reasoning layer
- an optional LSP-enriched semantic analysis layer

Probe Server should extend that model to a company corpus by introducing:

- a configuration plane
- a sync and indexing plane
- a retrieval plane for routing
- a persistent architecture knowledge plane
- a multi-tenant answering plane

## Core Principle

Use a two-stage system:

1. Global routing over the company corpus
2. Deep Probe analysis over the shortlisted scope

This implies:

- a retrieval/index backend for fast narrowing
- Probe extraction and reranking for final evidence selection
- optional LSP only for the final shortlist or selected hot repositories

In practical terms:

- Zoekt or equivalent answers: "where should I look?"
- Probe answers: "what exactly matters there?"

## Comparison With Existing Projects

This plan is informed by three different models:

- `Probe` today
- `Sourcebot`
- `repowise`

They overlap at the product headline level, but the technical center of gravity is different for each.

### Probe Today

Probe is strongest as a live analysis engine:

- search current files directly
- extract complete AST-aware blocks
- fit results into token budgets
- optionally enrich with LSP for definitions, references, and call hierarchy

Its retrieval substrate is the code itself, not a precomputed wiki or a centralized search service.

### Sourcebot

Sourcebot is strongest as a centralized indexed search and navigation platform:

- repository synchronization from code hosts
- branch-aware indexing
- Zoekt-backed search
- web-native browsing and search
- MCP and web app over a hosted corpus

Its retrieval substrate is a centralized trigram index over mirrored repositories.

### repowise

repowise is strongest as a persistent codebase intelligence system:

- tree-sitter graph ingestion
- git-derived ownership, hotspots, and co-change signals
- LLM-generated wiki pages
- architectural decision extraction and storage
- persistent REST, MCP, and dashboard surfaces

Its retrieval substrate is primarily the generated wiki and stored intelligence layers, backed by FTS and vector search.

## Technical Comparison

### Retrieval Substrate

- `Probe`
  direct retrieval over live repository contents, with AST-aware block extraction and ranking
- `Sourcebot`
  centralized lexical retrieval over Zoekt indexes
- `repowise`
  retrieval over stored wiki pages plus FTS and semantic vector search

Implication for Probe Server:

- do not make generated docs the primary truth layer
- do not make raw Zoekt matches the final answer layer
- use global retrieval for routing, then Probe for final evidence extraction

### Freshness Model

- `Probe`
  naturally fresh, because it works on the current filesystem
- `Sourcebot`
  fresh only after sync and re-index
- `repowise`
  fresh only after sync, analysis, and wiki regeneration

Implication for Probe Server:

- preserve a path for code-grounded freshness
- keep retrieval indexes incremental
- avoid coupling all answer quality to wiki regeneration

### Semantic Depth

- `Probe`
  AST extraction plus optional LSP semantic operations
- `Sourcebot`
  search-first, with code navigation largely derived from indexed symbol heuristics
- `repowise`
  precomputed graph analysis, caller-callee structure, community detection, decision mining

Implication for Probe Server:

- Probe should own final semantic depth
- graph and git signals can be additive, but should not replace code-grounded extraction

### Persistence Model

- `Probe`
  light persistent state, optional caches, local sessions
- `Sourcebot`
  operational platform state plus mirrored repos and search indexes
- `repowise`
  heavy persistent knowledge store with wiki pages, graph, git metadata, decisions, and embeddings

Implication for Probe Server:

- persist operational and organizational state
- persist selective intelligence layers
- avoid turning the whole product into a documentation cache with code as a secondary source

### Best Technical Takeaways

From `Sourcebot`, Probe Server should borrow:

- remote code-host sync
- branch-aware corpus management
- centralized breadth retrieval
- web and MCP access over the same corpus

From `repowise`, Probe Server should borrow:

- git intelligence
- ownership and co-change signals
- explicit decision and rationale layer
- multi-repo workspace awareness

From current `Probe`, Probe Server should preserve:

- AST-aware extraction as the final answer primitive
- token-aware assembly for agents
- optional LSP-backed semantic depth
- direct code-grounded reasoning over the current source of truth

### What Probe Server Should Explicitly Avoid

Avoid the weakest failure modes of each comparison target:

- from `Sourcebot`
  returning search hits as the effective final product primitive
- from `repowise`
  making generated wiki pages the primary knowledge truth for code questions
- from both
  requiring heavyweight indexing before the system is useful in narrower local or hot-repo workflows

## Oel-Inspired Configuration Model

The current Oel setup in `../REFINE/Oel` uses a practical and effective pattern:

- a structured project catalog in `config/projects.yaml`
- a human-maintained architecture narrative in `docs/tyk-architecture.md`
- routing overrides in `config/skills.yaml`
- prompt-time composition of catalog + architecture + routing rules

That pattern should be adopted by Probe Server, but promoted from workflow-local config into a first-class product model.

### Lessons From Oel

`projects.yaml` is not just a repo list. It is a routing catalog:

- stable project IDs
- canonical repo names
- human-authored descriptions
- optional project-specific knowledge

`tyk-architecture.md` is not just documentation. It is an architecture routing guide:

- component glossary
- disambiguation rules
- common mistakes
- cross-repo inclusion guidance

`skills.yaml` binds the two:

- inject architecture
- inject routing overrides
- load projects

This separation is strong because:

- structured inventory stays reusable
- architecture prose remains easy for humans to edit
- special routing logic is explicit rather than buried inside generic prompts

## Proposed Probe Server Configuration Model

For each organization, define four authoring artifacts:

- `orgs/<org>/projects.yaml`
- `orgs/<org>/architecture.md`
- `orgs/<org>/routing.yaml`
- `orgs/<org>/scopes.yaml`

### `projects.yaml`

Structured repo and knowledge-source catalog.

Suggested shape:

```yaml
- id: gateway
  repo: acme/gateway
  host: github
  default_branch: main
  kind: service
  description: |
    Core API gateway:
    - request routing
    - auth and policy enforcement
    - upstream proxying
  tags: [edge, api, go]
  owners: [platform]
  always_include_with: [gateway-ui]
  index_mode: probe_blocks
  knowledge: |
    Include for auth, routing, middleware, and policy questions.
```

Recommended fields:

- `id`
- `repo`
- `host`
- `default_branch`
- `kind`
  examples: `service`, `frontend`, `infra`, `docs`, `knowledge-base`, `tooling`
- `description`
- `tags`
- `owners`
- `always_include_with`
- `never_route_for`
- `index_mode`
- `knowledge`

### `architecture.md`

Human-maintained organization architecture narrative.

Use it for:

- service boundaries
- control-plane and data-plane relationships
- system ownership and responsibilities
- major architectural flows
- common routing disambiguations
- "always include X for topic Y" explanations

This file should read like a high-quality internal architecture handbook, not a machine-generated dump.

### `routing.yaml`

Structured routing policy that complements the prose.

Examples:

```yaml
rules:
  - id: cloud-platform
    match:
      any_terms: ["control plane", "cloud deployments", "tenant provisioning"]
    include_projects: ["control-plane", "cluster-operator"]
    exclude_projects: ["dashboard-ui"]

  - id: bot-stack
    match:
      any_terms: ["bot", "trace id", "workflow engine", "automation"]
    include_projects: ["probe-server", "workflow-config"]
```

Use it for:

- include and exclude rules
- paired-project rules
- priority boosts
- routing disambiguation
- special treatment for knowledge-base repos

### `scopes.yaml`

Reusable search scopes for common organizational domains.

Examples:

- `frontend`
- `payments`
- `infra`
- `customer-data`
- `docs`
- `compliance`

This gives users and agents a stable way to narrow the corpus without naming repos manually.

## What Belongs In Postgres vs Git-Managed Config

Postgres is still useful, but it should not be the only source of truth for architecture knowledge.

### Postgres Should Store

- organizations, users, teams, auth, API keys
- connectors and credential references
- repo registry and sync state
- branches, revisions, index state, job history
- saved sessions, answers, citations
- scopes, tags, ownership metadata
- published and compiled routing state
- audit logs and telemetry metadata

### Git-Managed YAML/Markdown Should Store

- project descriptions written for routing
- architecture narratives
- domain-specific routing rules
- special repo handling instructions
- curated examples and policy text

### Why This Split

YAML and Markdown are better for:

- human review
- pull request workflows
- version control
- operational editing by engineering teams
- keeping prompt-oriented knowledge readable

Postgres is better for:

- runtime state
- tenancy
- indexing lifecycle
- sessions and permissions
- compiled active config

### Recommended Runtime Model

- YAML and Markdown are the authoring format
- a config loader compiles them into normalized runtime records
- Postgres stores the active compiled configuration
- retrieval and answering services consume the compiled form

## System Architecture

Probe Server should be split into a few clear services.

### 1. `probe-api`

Responsibilities:

- auth and tenancy
- REST and MCP endpoints
- session handling
- citation and answer APIs
- config management APIs

### 2. `probe-sync`

Responsibilities:

- GitHub/GitLab/Gitea/Bitbucket/local git connectors
- webhook handling
- scheduled sync
- branch and tag policy application
- bare repo mirror management

### 3. `probe-indexer`

Responsibilities:

- retrieval index maintenance
- file, repo, and symbol catalogs
- incremental updates
- block extraction caches

### 4. `probe-analyzer`

Responsibilities:

- Probe search/extract/query execution on shortlisted targets
- question-aware reranking
- token-aware context assembly
- evidence extraction for answers

### 5. `probe-lsp-workers`

Responsibilities:

- optional deep semantic analysis
- call/reference/hover enrichment
- workspace-aware semantic caches
- only for selected hot repos or shortlisted targets

### 6. `probe-web`

Responsibilities:

- company chat UI
- answer browsing
- citations and source navigation
- config and scope management
- saved research sessions

## Storage and Infrastructure

### Primary Stores

- `Postgres`
  runtime config, org data, repo metadata, jobs, sessions, citations
- `Redis` or equivalent
  job queue and distributed coordination
- object or filesystem storage
  bare repo mirrors, cached artifacts, optional extracted block caches
- retrieval index backend
  Zoekt is the current recommended starting point for breadth

### Optional Later Stores

- semantic cache store for LSP artifacts
- graph or relationship store if relationship extraction matures enough to justify it

Do not require a graph database in the first version.

## Retrieval and Analysis Pipeline

The answering path should work like this:

1. Parse the user question into intent and likely scope
2. Load architecture knowledge, routing rules, and project catalog
3. Route to likely repos, scopes, and documents
4. Run global retrieval over the routed corpus
5. Select top repos and top files or blocks
6. Run Probe extraction and reranking on the shortlist
7. Optionally run LSP enrichment for the top few targets
8. Synthesize final answer with citations and explicit confidence boundaries

This order matters. For many architecture questions, curated docs and ownership data are better starting points than source code alone.

## How Zoekt Fits

Zoekt should be used as the routing and breadth layer, not as the final Probe engine.

Good uses:

- multi-repo retrieval
- branch-aware search
- repo and file narrowing
- quick candidate generation for a large corpus

Not the final answer layer:

- final answer units should not be raw search matches
- Probe should still assemble AST-shaped, token-aware final context
- semantic deepening should remain in Probe and optional LSP layers

### Recommended Hybrid Flow

1. Probe generates one or more retrieval queries
2. Zoekt searches the full corpus
3. Router returns candidate repos, files, and revisions
4. Probe analyzes only the shortlisted items
5. Optional LSP runs only on the top evidence set

This preserves Probe's differentiation while making large-corpus retrieval practical.

## Indexing Strategy

Do not LSP-index the entire company in the first version.

Use a tiered indexing strategy:

### Tier 1: All Repositories

- git mirrors
- lexical retrieval index
- repo and file metadata

### Tier 2: Important Repositories

- AST block extraction
- symbol and block catalogs
- richer search metadata

### Tier 3: Hot Repositories

- LSP enrichment
- semantic caches
- deeper relationship information

This balances scale, freshness, and operational cost.

## What To Index

Index semantic units, not arbitrary fixed-size chunks.

Recommended indexable entities:

- repository records
- file records
- symbol definitions
- extracted semantic blocks
  functions, classes, handlers, modules
- architecture and operational docs
- ownership and domain metadata

Avoid making 512-character chunks the primary unit of retrieval. That would weaken Probe's main value proposition.

## Architecture Knowledge Plane

To answer questions about "whole company architecture," code alone is not enough.

The system should ingest and reason over:

- ADR repositories
- docs repositories
- runbooks
- OpenAPI and protobuf specs
- Kubernetes manifests and Helm charts
- ownership and service catalogs
- internal knowledge-base repos

These should be first-class sources in the routing and retrieval pipeline.

Some repositories are not code and should be treated differently. Oel already models this via entries like `customer-insights` with repo-specific guidance. Probe Server should support the same concept explicitly.

Suggested source kinds:

- `code`
- `docs`
- `knowledge_base`
- `infra`
- `api_spec`
- `ownership_catalog`

## Access Model

Probe Server should support:

- web chat for humans
- API for product integrations
- MCP for IDEs and autonomous agents

The initial product should be read-mostly:

- ask questions
- explore architecture
- locate owners and systems
- trace likely flows
- collect citations and evidence

Editing and autonomous code changes should come later, after access controls and trust boundaries are well established.

## Recommended MVP

Build the first useful version in this order:

1. Postgres-backed org, repo, connector, and job model
2. GitHub and GitLab sync workers with bare repo mirrors
3. Git-managed org config
   `projects.yaml`, `architecture.md`, `routing.yaml`, `scopes.yaml`
4. Global retrieval backend for repo and file routing
5. Probe shortlist analysis service
6. Web and MCP endpoints
7. Architecture doc ingestion and citations
8. Selective LSP support for hot repos

This is the shortest path to a useful company-wide answering engine.

## Explicit Non-Goals For V1

Do not build these first:

- full semantic graph for every repo
- embeddings as the primary retrieval layer
- per-request cloning
- per-request LSP startup for arbitrary repos
- broad autonomous editing across org repos
- a giant monolithic server process

These can be added later if real usage proves the need.

## Example Org Layout

```text
orgs/
  acme/
    projects.yaml
    architecture.md
    routing.yaml
    scopes.yaml
```

At runtime:

- the authoring files are loaded and validated
- compiled into normalized runtime config
- stored in Postgres for active use
- consumed by routing, sync, indexing, and answering services

## Summary

Probe Server should be:

- indexed for retrieval
- Probe-native for extraction and final context
- selectively semantic for deep analysis
- curated through Git-managed architecture and project config
- operationally backed by Postgres, queues, and repo mirrors

The strongest design is to combine:

- Oel's curated project and architecture model
- Zoekt-style breadth routing
- repowise-style git and decision intelligence
- Probe's AST-aware and token-aware depth engine
- Postgres as runtime state, not the only knowledge source
