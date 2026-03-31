# WhatTheFile

**WhatTheFile** is a local-first, privacy-preserving desktop search app for macOS and Windows that lets you find files by *what they mean*, not just what they're named.

Instead of remembering filenames or folder structures, you describe what you're looking for in plain language — *"the contract about the Lisbon apartment"*, *"slides where we discussed churn"*, *"photo of a whiteboard with a flowchart"* — and WhatTheFile finds it.

Under the hood, every supported file type is extracted, chunked, and embedded into a local vector index using Ollama running entirely on your machine. A vision model handles images, describing their visual content so they become searchable by meaning too. Results are ranked by blending full-text (BM25) and semantic (cosine similarity) scores.

**Supported file types at launch:** PDF, DOCX, XLSX, CSV, TXT, MD, PNG, JPG.

No cloud. No API keys. No data ever leaves the device.

---

## v1 Scope

### In Scope

- **File types:** PDF, DOCX, XLSX, CSV, TXT, MD, PNG, JPG
- **Natural language queries** parsed into structured search intent (text, media type filters, date range, scope)
- **Dual retrieval:** FTS5/BM25 + semantic vector search (Ollama `nomic-embed-text`)
- **Image search** via vision model (llava/qwen2.5vl) — visual content described as text, then embedded
- **Incremental scanning** with blake3 fingerprinting, rename/move detection, cooperative cancellation
- **Background indexing service** — keeps the index fresh after the window closes
- **Desktop UI:** search box, result grid, file preview panel, scan progress
- **Platforms:** macOS (DMG) + Windows (MSI)
- **Privacy:** fully local — all inference on-device via Ollama; nothing leaves the machine

### Out of Scope (deferred)

| Feature | Notes |
|---|---|
| Thumbnail generation | Approach not yet defined — Rust pre-generation vs. on-demand |
| Face recognition | Requires per-face embeddings and identity clustering |
| Per-file scan checkpointing | Per-scan restart on interruption is sufficient for v1 |
| Optional cloud/remote Ollama | Local-only remains the default; opt-in post-MVP |
| Linux support | Post-MVP; AppImage target |
| Audio/video indexing | Schema is forward-compatible; pipeline not planned for v1 |

---

## Future Roadmap

| Feature | Notes |
|---|---|
| Thumbnail generation | Useful for result grid (images, PDF first-page preview). Approach not defined — evaluate Rust-side pre-generation vs. on-demand. |
| Face recognition | Person-based photo search. Requires per-face embeddings and identity clustering. |
| Per-file scan checkpointing | Upgrade from per-scan to per-file resume. Worth revisiting if large collections (100k+ files) become a common case. |
| Optional cloud/remote Ollama | Allow users to opt into a remote Ollama instance or cloud provider. Local-only remains the default. |
| Linux support | Post-MVP. AppImage target. See Wayland/NVIDIA note in [architecture.md](docs/architecture.md). |
| Audio/video indexing | Schema is forward-compatible; pipeline not planned for v1. |

---

## Docs

- [docs/requirements.md](docs/requirements.md) — Functional requirements (MVP)
- [docs/architecture.md](docs/architecture.md) — System architecture, modules, stack, build & test
- [docs/conventions.md](docs/conventions.md) — Coding conventions and core principles
- [docs/technical-decisions.md](docs/technical-decisions.md) — Decision records with benchmark evidence
- [docs/PLAN.md](docs/PLAN.md) — Implementation phases
