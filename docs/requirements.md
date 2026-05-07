# Functional Requirements (MVP)

## 1. Goal

Build a local-first desktop app that indexes user-selected folders (including NAS paths) in read-only mode and lets users find files using natural-language queries.

MVP user outcome:
- User searches in natural language.
- App returns relevant files.
- User can quickly inspect results via thumbnail grid and preview.

## 2. Target User

Personal users.

## 3. Product Principles

- Local-first behavior.
- Read-only over source folders.
- No source-file mutation (no rename/move/write on user files).
- Clear privacy and activity visibility.
- Responsive query UX even on large result sets.

## 4. Functional Scope

### 4.1 Included Features

- Mandatory onboarding before normal usage.
- Folder selection during onboarding with both quick presets and custom folder picker.
- "Skip for now" action with clear warning that search quality depends on indexing setup.
- Natural-language search using one global query box.
- Exact filename/path search in the same input.
- File-level results (not chunk-level UI).
- Result tile/list includes file path.
- Clicking a result opens the file.
- Search results displayed in thumbnail grid.
- Quick preview pane and space-key preview behavior.
- Background indexing progress visible to user.
- Indexing controls: pause, stop, reindex selected folder.
- Retry controls and detailed error list (dev-oriented in MVP).
- Limited-mode warning when online-dependent capability is unavailable.
- Privacy/trust controls: include/exclude scope editing, delete all indexed data, visible activity/audit log.

### 4.2 File Types

`pdf`, `docx`, `xlsx`, `xls`, `xlsm`, `csv`, `txt`, `md`, `png`, `jpg`/`jpeg`, `webp`

Spreadsheet behavior: file-level retrieval.

### 4.3 Result Behavior

Duplicates are listed separately in search results.

### 4.4 Responsiveness Requirements

- Search UI must remain responsive for very broad queries (including thousands of matches).
- Results must be presented progressively (not all rendered at once).

## 5. Onboarding and Consent

- Onboarding is always present.
- User must select at least one scope (preset and/or custom) for expected search behavior.
- App must provide a clear privacy notice during onboarding.
- App must communicate local-vs-cloud behavior clearly when that choice is available.

## 6. Privacy and Data Handling

- Indexing is read-only over source folders.
- All app-managed writes must be restricted to app-owned data directories.
- User must be able to see what the app indexed/processed via activity logs.
- User must be able to stop indexing and delete indexed data.

## 7. Platform and Packaging

- MVP target platforms: macOS and Windows.
- Linux packaging is out of scope for MVP.

## 8. Extensibility

- Architecture must support adding audio/video later without breaking the existing indexed data model.
- OCR and text handling must prioritize `pt-BR` and `en` while keeping multilingual expansion possible. Cross-language semantic matching is delegated to the multilingual embedding model — no translation dictionary.

## 9. Out of Scope (MVP)

- `pptx` support (phase 2).
- Advanced KMS collaboration features.
- Manual sorting controls in UI.
- Search history.

## 10. Success Criteria

Primary: search accuracy from the user's perspective.

Secondary:
- User can inspect and open relevant files quickly.
- User keeps control over indexing scope and privacy controls.
- Large result sets do not freeze or block the interface.
