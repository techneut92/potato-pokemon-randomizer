# 0006. Errors, logging, progress and cancellation

- Status: Accepted

## Decision

- **Errors:** each library crate has its own `thiserror` enum (`FormatError`, `RomError`, `SettingsError`,
  `RandomizeError`), with the source error chained. Binaries use `anyhow` at the top level. The desktop app maps
  errors to `ApiError { code, message }` so the UI can choose a message per `code`.
- **Expected outcomes are not errors:** an unsupported or encrypted ROM is a value
  (`Detection::Unsupported { reason }`), not a panic or an `Err` that callers have to string-match.
- **Logging:** `tracing` in libraries; `tracing-subscriber` only in `ppr-cli` and `ppr-desktop`. The randomization
  log for the user is a separate, structured `RandomizationLog` built by the pipeline and rendered to text, not
  `tracing` output.
- **Progress:** the pipeline reports through a `ProgressSink` trait (Observer pattern). The CLI implements it with
  a progress bar and the desktop app with a Tauri channel.
- **Cancellation:** a `CancelToken` (`Arc<AtomicBool>`) is checked between steps and inside long loops. A cancelled
  run returns `RandomizeError::Cancelled` and never writes a partial ROM. Output is written to a temporary file in
  the target directory and renamed into place when the run succeeds.
- **No `unsafe`** (`unsafe_code = "forbid"` in the workspace lints) and no panics on untrusted input. Parsers
  return errors, and fuzzing checks that they do.
