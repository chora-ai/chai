# Epic: RAG with Vector Database

**Summary** — Enable the assistant to pull knowledge from a local knowledge base backed by a vector database (pgvector) and a dedicated embedding model (Ollama or LM Studio), so the orchestrator can use retrieved context for chat and completion.

**Status** — Proposed (not implemented).

---

## Goal

Provide a **local-first** retrieval-augmented flow: content (e.g. from an Obsidian vault or other markdown sources) is embedded by a **worker embedding model** (Ollama, LM Studio, or another service) and stored in a **vector database using pgvector**. The **orchestrator** (main chat/completion model) can request relevant chunks via tools: **index build** (when to (re)build the knowledge base) and **query** (retrieve top-k chunks for a query). The worker model is responsible for both index-time and query-time embedding; the orchestrator consumes retrieved context to answer.

## Current State

- **Skills and tools** — Notes and vault access are provided by skills (e.g. notesmd, obsidian) that expose tools (search, create, read). The model decides when to call these tools and receives results in the conversation; there is no semantic search or pre-indexed vector store.
- **Embeddings** — Not used. Ollama and LM Studio support embedding models but the endpoints are not yet integrated.
- **Orchestration** — The [orchestration epic](EPIC_ORCHESTRATION.md) is not yet implemented. The RAG flow would use a worker (embedding model) for index build and query embedding; the orchestrator requests retrieval and consumes the result.

## Scope

- **In scope:** Vector store using **pgvector** (local Postgres or compatible) for portability and future Supabase compatibility; integration with at least one local embedding API (Ollama, LM Studio, or another service) for the worker embedding model; pipeline to embed documents and index them; retrieval (query → embed → similarity search → top-k chunks); tools for the agent: **index build** (e.g. `index_knowledge_base`) and **query** (e.g. `query_knowledge_base`); config for sources, chunking, embedding backend/model, and DB connection.
- **Out of scope:** Implementing the full orchestrator–worker loop (see [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md)); replacing or removing existing note/vault skills (they can coexist). Supabase integration (using same pgvector schema and usage patterns).

## Dependencies

- **API alignment** — Embedding endpoints are backend-specific (Ollama, LM Studio, etc). See [EPIC_API_ALIGNMENT.md](EPIC_API_ALIGNMENT.md) and reference documentation for supported or planned backend services; existing backend clients should be extended to support embedding (no need to create a separate client for embedding).
- **Orchestration** — This epic would ideally follow or overlap with [EPIC_ORCHESTRATION.md](EPIC_ORCHESTRATION.md). The worker embedding model (a model provided by Ollama, LM Studio, etc) handles index build and query embedding; the orchestrator (or single model) calls tools to build the index and to query the knowledge base and consumes retrieved context.

## Requirements

- [ ] **Vector store (pgvector)** — Postgres with pgvector (or compatible) to store document chunk embeddings and metadata; support for similarity search (query vector → top-k). Schema and usage patterns should remain compatible with Supabase for a future follow-up.
- [ ] **Embedding support on existing backends** — Extend existing backend clients (Ollama, LM Studio, or other) to support embedding: call each provider’s embed endpoint and expose a common “embed(texts) → vectors” interface (e.g. trait or shared type); config for which backend and model to use for the worker embedding model. This worker model is used for both index build and query embedding.
- [ ] **Indexing pipeline** — Ingest documents from configured sources (e.g. markdown from vault or workspace), chunk them (size/overlap strategy), embed chunks via the (extended) backend client, and write to the vector store. Triggered by an **index-build** tool (e.g. (re)build knowledge base), not necessarily automatic.
- [ ] **Retrieval** — Given a query string, embed the query with the same worker model, run similarity search in the vector store, return top-k chunks (and optional metadata) in a form the agent can consume (e.g. tool result).
- [ ] **Agent tools** — (1) **Index build** — Tool (e.g. `index_knowledge_base`) to (re)build the knowledge base from configured sources (when to run is up to the user or model). (2) **Query** — Tool (e.g. `query_knowledge_base`) so the main assistant can request retrieval and receive chunks for its reply.
- [ ] **Config and sources** — Config for knowledge-base source path(s), chunking options, embedding backend/model, and vector store connection (local Postgres); document sources to index (e.g. Obsidian vault path, workspace directory).

## Technical Reference

### Definitions

- **Knowledge base** — A collection of documents (e.g. markdown files) that have been chunked, embedded with the worker embedding model, and stored in the vector database (pgvector) so they can be retrieved by semantic similarity to a query.
- **Worker embedding model** — The model used for embeddings (a model provided by Ollama, LM Studio, etc). It is responsible for **index build** (embedding document chunks) and **query embedding** (embedding the user query for retrieval). It is not the main chat/orchestrator model.
- **Retrieval** — Embed the query with the worker model, search the vector store for nearest vectors (e.g. cosine or L2), return the corresponding chunks (and optional metadata such as source path).

### Vector DB vs Skill-Based Search

| Aspect | Skill-based (e.g. obsidian search) | Vector DB + embeddings |
|--------|-------------------------------------|-------------------------|
| **Match type** | Lexical (keywords, grep-style) | Semantic (meaning similarity) |
| **Scale** | Tool call per query; scans or searches on demand | Pre-indexed; lookup is O(log n) or similar |
| **Context** | Model decides what to search and how to phrase it | Query tool returns top-k relevant chunks |
| **Best for** | Exact names, known terms, structured ops (create, update) | “Find anything related to X,” long-tail questions, vague queries |

Both can coexist: skills remain for CRUD and explicit search; the knowledge base adds semantic retrieval that the orchestrator can use when a question benefits from “everything relevant” rather than a single tool result.

### Advantages of This Epic

- **Local-first** — Primary implementation runs on the user’s machine or self-hosted Postgres; privacy and offline use. Supabase integration can follow using the same pgvector approach.
- **Portable and Supabase-compatible** — Using pgvector keeps schema and usage patterns aligned with Supabase so a follow-up “use Supabase as vector store” feature is straightforward.
- **Right tool per job** — Worker embedding model handles similarity; the main model stays focused on reasoning and chat.
- **Reuse of existing stacks** — Ollama and LM Studio (or other services) provide the embedding model; no separate third-party embedding service required.
- **Scalability** — As the vault or workspace grows, semantic search over an index scales better than repeatedly calling search tools.
- **Consistency with orchestration** — Index build and query both use the worker (embedding model + vector store); the orchestrator calls tools and consumes results.

### Implementation Notes

- **pgvector** — Use Postgres with the pgvector extension for storage and similarity search. Local Postgres (or a compatible server) for the initial implementation; same schema and patterns allow a later Supabase-backed option.
- **Embedding on existing clients** — Extend the existing Ollama and LM Studio clients to call their embed endpoints. Expose a common interface (e.g. trait) such as `embed(texts) → vectors` so the indexing and retrieval pipeline can use any backend that supports embedding.
- **Chunking** — Chunk size and overlap affect recall and context size; start with a simple strategy (e.g. fixed token or character windows with overlap) and make it configurable.
- **Index build** — Triggered via a tool (e.g. “rebuild knowledge base” or “sync knowledge base”), not necessarily on a schedule or file watcher; the user or model decides when to run it. This keeps the first version simple and avoids blocking the agent.
- **Source of documents** — Could be the same paths used by Obsidian/notesmd skills (vault or notes root), or a separate config (e.g. `knowledgeBase.sources`). Reusing vault path keeps config simple and allows the same content to be both tool-searchable and semantically retrievable.

---

## Open Questions for the Epic

These can be resolved during design or early implementation:

1. **LM Studio embedding API** — Exact endpoint and request/response shape for supported backend services (Ollama, LM Studio, etc); document in the appropriate reference document once confirmed.
2. **Chunking strategy** — Default chunk size (tokens or characters), overlap, and whether to respect markdown structure (e.g. by heading) or use fixed windows.
3. **Refresh strategy** — Whether to add later: on-demand only (current), file watcher, or periodic scan; how to handle large vaults or other knowledge base sources without blocking the agent.
4. **Multi-source** — Single knowledge base per workspace vs multiple named bases (e.g. “vault” vs “docs”) with different sources and optional per-query scope.
5. **Orchestrator coupling** — Whether retrieval is always implemented as a tool call, or whether a “retrieval worker” in the orchestration epic is the sole entry point (and the tool is a thin wrapper that delegates to that worker).
