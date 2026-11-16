## Context

The repository session resolves prefixes. Before selection the page cannot know whether prefix configuration is needed. Recent projects currently identify first use, but selection immediately removes that signal.

## Decisions

- Store optional `appSettings.onboardingStatus` as `inProgress` or `complete` through the existing Rust settings writer. Initialize after settings catch-up: recent repositories mean complete, otherwise in progress.
- Derive the visible stage from selected path and current subscription. Persist no prefix draft or duplicate path. Complete only on the accepted current subscription with a usable prefix; discard late answers after switching.
- Keep transient focus intent outside keyed welcome rows. A selection made during this session focuses the revealed prefix form; restoration at startup does not. A native picker may answer before the webview regains focus, and native focus retries a refused subscription, remounting the form: keep the request through both until the next user gesture.
- Use an ordered list with the active step identified by aria-current, a check mark and accessible completion text, wrapping repository path and an informational preview/loading row.
- Existing users and later repositories keep standalone prefix step 1. Errors preserve progress and retain their existing alert behavior; Ready to Sync stays separate.

## Verification

Focused lifecycle unit tests and Chromium/WebKit welcome flows cover persistence, completion, focus and race conditions. Full frontend/E2E checks and an isolated native check precede a lease commit and landing.
