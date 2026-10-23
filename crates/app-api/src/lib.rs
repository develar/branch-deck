//! The adapters' crate: what the app (`src-tauri`) and the e2e server (`test-server`) share between a command and the
//! cores, so both answer the same. Only those two depend on it, and it reaches no Tauri (`architecture_test.rs` of
//! `bd-cli`). For now it holds only `api_error`, the one conversion of a core's error into the `ApiError` of a command
//! (docs/design/rust-domain-authority.md, "Ошибки").

pub mod api_error;
