# Import Guidelines and No Re-exports Policy

**Status:** Active  
**Updated:** 2025-08-22  
**Tags:** [imports, rust, nuxt]

## Summary

Maintain clear, explicit dependencies by preferring direct imports over re-exports. Never use `pub use` statements except for internal module organization. All Nuxt composables and Vue APIs are auto-imported.

## Context

This simplifies navigation and refactoring by making dependencies explicit. Applies to Rust backend crates and Nuxt 4 frontend.

## Principles

- Prefer direct imports over re-exports.
- Make modules public (`pub mod`) instead of re-exporting their contents.
- Do not re-export external types at crate roots (e.g., error types).
- Keep imports self-documenting; import from the exact module path.
- Limit exceptions to private, internal organization cases only.
- Frontend (Nuxt): rely on Nuxt’s auto-imports; do not manually import Vue APIs (`ref`, `computed`, etc.), Nuxt composables, or project utilities/types; in templates, refs are auto-unwrapped.

## The No Re-exports Rule

**NEVER use `pub use` statements** except for internal module organization within a single crate.

### ❌ Wrong - Re-exports

```rust
// lib.rs
pub use config::ModelConfig;
pub use generator::BranchGenerator;

// Other files
use crate::{ModelConfig, BranchGenerator}; // Uses re-exports
use model_core::{ModelConfig, BranchGenerator}; // Uses re-exports
```

### ✅ Correct - Direct imports

```rust
// lib.rs
pub mod config;
pub mod generator;

// Other files
use crate::config::ModelConfig;
use crate::generator::BranchGenerator;

// From other crates
use model_core::config::ModelConfig;
use model_core::generator::BranchGenerator;
```

## Module Structure Guidelines

### Making Modules Public

Instead of re-exporting contents, make the modules themselves public:

```rust
// ❌ Wrong
mod config;
pub use config::ModelConfig;

// ✅ Correct
pub mod config;
```

### Internal Organization

You may use `pub use` for internal module organization within a crate:

```rust
// ✅ Acceptable for internal organization
mod internal_module;
pub use internal_module::PublicType; // Only if internal_module is private
```

## Benefits of No Re-exports

### 1. Explicit Dependencies
Every import shows exactly where types come from:

```rust
use git_executor::git_command_executor::GitCommandExecutor;
use sync_types::issue_navigation::IssueNavigationConfig;
```

### 2. Better IDE Navigation
- Click "Go to Definition" goes directly to the source
- No confusion about which re-export chain to follow
- Clearer understanding of module relationships

### 3. Easier Refactoring
- Moving types doesn't break re-export chains
- Rename operations are more predictable
- Less coupling between modules

### 4. Reduced Cognitive Load
- No need to remember what's re-exported where
- Imports are self-documenting
- Less "magic" in the codebase

## Common Patterns

### Crate Structure

```rust
// lib.rs
pub mod commands;
pub mod generator;
pub mod path_provider;

// No re-exports - users import what they need:
use model_tauri::generator::ModelGeneratorState;
use model_tauri::path_provider::TauriModelPathProvider;
```

- 2025-08-22: Added Frontend (Nuxt 4) Auto-imports section; broadened scope to frontend; updated principles.
### Error Types

```rust
// ❌ Don't re-export error types
pub use anyhow::Result;

// ✅ Use them directly
use anyhow::Result;
```

### Test Utilities

```rust
// ❌ Don't re-export test utilities
pub use test_utils::{TestRepo, TestProgressReporter};

// ✅ Import directly
use test_utils::git_test_utils::TestRepo;
use test_utils::progress_reporter::TestProgressReporter;
```

## Frontend (Nuxt 4) Auto-imports (IMPORTANT!)

- ALL composables, core Vue APIs, Nuxt composables, and project utilities/types are auto-imported by Nuxt 4.
- NEVER manually import Vue APIs (`ref`, `computed`, `watch`), Nuxt composables (`useRoute`, `useState`, etc.), or registered project utilities/types.
- In Vue templates, refs are automatically unwrapped (no `.value`).

### ✅ Examples (do this)

1. **Remove re-exports** from lib.rs files
2. **Make modules public** with `pub mod`
3. **Update imports** to use full paths
4. **Add missing dependencies** if needed
5. **Test compilation** to catch missing imports

### Example Migration

Before:
```rust
// model_core/lib.rs  
pub mod config;

// Other crate
use model_core::config::ModelConfig;
```

After:
```rust
// model_core/lib.rs  
pub mod config;

// Other crate
use model_core::config::ModelConfig;
```

## Tools and Enforcement

- Use `pnpm lint:backend` to catch unused imports and run clippy
- Use `pnpm lint:frontend` to lint and type-check TypeScript/Vue and validate auto-import usage
- IDE warnings for unused re-exports
- Code reviews should flag any new `pub use` statements
- Automated tests ensure all imports work correctly

## Exceptions

The only acceptable use of `pub use` is for internal module organization where the source module is private:

```rust
// ✅ Acceptable - internal_types is private
mod internal_types;
pub use internal_types::PublicInterface;
```

Even then, prefer making the module public when possible.