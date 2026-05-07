# WhatTheFile — Project Documentation

This directory is the project's single source of truth (SST). Each fact lives in **exactly one** doc; everything else references it.

When you change a fact, update its owner — never duplicate.

---

## SST Ownership Map

| Doc | Owns | Cadence of change |
|---|---|---|
| [requirements.md](requirements.md) | What the product does for the user. Locked principles. Acceptance criteria. | Stable; rarely changes |
| [architecture.md](architecture.md) | Module boundaries, data flow, IPC contract, runtime data locations, build & test commands | Changes when structure changes |
| [db-schema.md](db-schema.md) | SQLite tables, DDL, key patterns (`index_marker`, fingerprint move-detection, KNN syntax) | Changes when schema changes |
| [conventions.md](conventions.md) | Coding rules, error handling, TDD discipline, testing strategy (CI vs. local pre-push), coverage targets, eval workflow | Changes when team norms change |
| [technical-decisions.md](technical-decisions.md) | Every locked decision with rationale and benchmark / audit evidence. Provisional decisions flagged until eval validates | Append-only; deprecate, never overwrite |
| [PLAN.md](PLAN.md) | Living roadmap — phases, current status, next steps. Acceptance criteria mirrored from requirements for phase exit reference | Changes constantly |
| [IMPLEMENTATION.md](IMPLEMENTATION.md) | Phase D rework — ordered commit sequence with scope, tests, validation gates, and resumption guide for fresh sessions | Updated after each commit lands |

---

## How to use this layout

- **"Why did we choose X?"** → `technical-decisions.md`
- **"What's the v1 scope?"** → `requirements.md`
- **"What are we doing this week?"** → `PLAN.md` (Current Status section)
- **"How do modules connect?"** → `architecture.md`
- **"What's the schema?"** → `db-schema.md`
- **"How do we test?"** → `conventions.md`

If you find the same fact in two docs, **the one not listed as owner is wrong** — fix the duplicate, link to the owner.

`technical-blueprint-v1.md` was deleted on 2026-04-29; its content was merged into `technical-decisions.md` and `PLAN.md`.
