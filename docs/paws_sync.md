# Syncing back with forge

This document notes all the major things to consider when syncing back with forge upstream.

## General Guidelines
1.  **Fork Awareness:** This is a fork renamed to `paws`. Configuration files, env variables, and binary names differ.
2.  **Filter Changes:** Only add fixes, new features, and improvements.
    *   Prefer upstream implementation if feature parity exists.
    *   **SKIP** behavioral tests that rely on upstream-specific output.
    *   **SKIP** branding changes (links to forgecode.dev, issue templates).
    *   **SKIP** features that conflict with Paws core (e.g., specific UI flows if Paws has diverged).
3.  **Conflict Resolution:** Understand where merge conflicts arise (usually `Cargo.toml`, `README`, branding).
4.  **Documentation:** Document every migration in `docs/migrations/`.

## Notes for Agents

### Critical Paws vs Forge Differences
- **Binary Name:** `paws` vs `forge`.
- **Config Directory:** `~/.paws` vs `~/.forge` (check specific implementation).
- **Repo Layer:** Paws may have removed or altered dependencies (e.g., `async-openai`).

### Key Learnings from 2026-01-15 Sync
- **Tool Renaming:** Upstream renamed `search` tool to `fs_search`. Update all references in agents/config.
- **Workspace Terminology:** Upstream renamed `codebase` to `workspace` in domain types. This is a breaking change.
- **Streaming:** Upstream enabled streaming by default. Paws UI must support this or explicitly disable it.
- **ZSH/Shell Integration:** `forge` specific shell commands need adaptation to `paws`.
- **Dependencies:** `async-openai` was removed in upstream favor of direct HTTP.
- **Spinner/UI:** Significant changes to spinner and UI rendering in upstream. Handle with care.

## Migration History
- [2026-01-15 Sync](./migrations/2026-01-15-sync.md)
