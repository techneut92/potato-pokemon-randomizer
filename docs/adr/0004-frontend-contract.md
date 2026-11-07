# 0004. Frontend contract: Tauri commands, events and generated types

- Status: Accepted

## Context

The Svelte frontend already talks to a `Backend` interface (`src/lib/backend.ts`) with a mock implementation.
The Rust types must not drift from the TypeScript ones.

## Decision

- `TauriBackend` implements the existing `Backend` interface with `invoke()` (Adapter pattern). The mock stays for
  the browser dev server and for UI tests.
- Commands in `ppr-desktop` are thin. They translate arguments, call `ppr-service`, and map errors to a
  serializable `ApiError { code, message }`. No business logic lives in the command layer.
- Progress goes over a Tauri `Channel<Progress>` passed to `randomize`, and cancellation is a separate
  `cancel_run` command. Both are scoped to a run ID.
- Types that cross the boundary derive `serde` and `ts_rs::TS`, and are exported to
  `src/lib/generated/`. A test regenerates them and CI fails if the committed files differ. The Rust types are
  the single source of truth.
- File access goes through the Tauri dialog plugin. The frontend never builds file paths itself.

## Consequences

- Renaming a field in Rust breaks `pnpm check` instead of failing at runtime.
- Option labels, tooltips and UI rules stay in the frontend (`defs.ts`, `OptionEngine`). The backend only knows
  the `Settings` model, and the UI's option state is mapped to it in one place.
