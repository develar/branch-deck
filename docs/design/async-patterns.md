# Async Patterns and spawn_blocking

**Status:** Implemented  
**Updated:** 2025-08-23  
**Tags:** [async, performance, tauri]

## Problem

Tauri commands must be async to avoid blocking the UI thread, but git operations are inherently blocking I/O using synchronous system calls. Running blocking operations directly in async functions blocks the async worker threads, starving the runtime and degrading performance.

The challenge: How to bridge async Tauri commands with synchronous git operations without blocking async threads or adding unnecessary complexity?

## Solution Overview

Use `spawn_blocking` to move all blocking I/O to dedicated thread pools. Tauri commands become thin async wrappers that immediately delegate to synchronous core functions running on blocking threads. This maintains clean separation between async coordination and blocking execution.

## Design

### Architecture

```mermaid
flowchart TD
    subgraph "Frontend"
        A[JavaScript Call]
    end
    
    subgraph "Tauri Async Runtime"
        B[Async Command Handler]
        C[spawn_blocking]
    end
    
    subgraph "Blocking Thread Pool"
        D[Sync Core Function]
        E[Git Operations]
    end
    
    A -->|IPC| B
    B -->|Move to blocking pool| C
    C --> D
    D --> E
    E -->|Result| D
    D -->|Result| C
    C -->|Await| B
    B -->|Response| A
    
    style B fill:#f9f,stroke:#333,stroke-width:2px
    style D fill:#9f9,stroke:#333,stroke-width:2px
```

### Thread Pool Architecture

```mermaid
flowchart LR
    subgraph "Async Worker Threads"
        A1[Worker 1]
        A2[Worker 2]
        A3[Worker N]
    end
    
    subgraph "Blocking Thread Pool"
        B1[Blocking 1]
        B2[Blocking 2]
        B3[Blocking 512]
    end
    
    A1 -->|spawn_blocking| B1
    A2 -->|spawn_blocking| B2
    A3 -->|spawn_blocking| B3
    
    style A1 fill:#f9f
    style A2 fill:#f9f
    style A3 fill:#f9f
    style B1 fill:#9f9
    style B2 fill:#9f9
    style B3 fill:#9f9
```

### Key Decisions

1. **Decision**: Synchronous GitCommandExecutor
   **Rationale**: Git operations are blocking; no benefit from async complexity

2. **Decision**: spawn_blocking in every Tauri command that runs git or reads files
   **Rationale**: Consistent pattern, prevents async thread blocking. A command that talks HTTP (Patronus,
   `src-tauri/src/commands/patronus.rs`) awaits the request on the async runtime with one shared `reqwest::Client`,
   and only its git part (which repository) goes through `run_blocking`.

3. **Decision**: Separate core functions from command handlers
   **Rationale**: Core logic is testable without async runtime

4. **Decision**: Clone state for 'static lifetime
   **Rationale**: spawn_blocking requires 'static, State is not 'static

5. **Decision**: JoinSet.spawn_blocking for parallel operations
   **Rationale**: Direct spawning without double-async wrapper

### Implementation Notes

Pattern structure:
- **Tauri Command**: Async wrapper with `#[tauri::command]` (see `src-tauri/src/commands/`)
- **Core Function**: Synchronous implementation with actual logic
- **State Cloning**: Clone `State<T>` before moving to blocking thread
- **Error Mapping**: Convert JoinError to user-friendly message

Parallel operations:
- Use `JoinSet::spawn_blocking` directly
- Avoid `spawn(async { spawn_blocking(...) })` double-spawning
- Semaphore for concurrency limiting when needed (`max_parallel_git_processes()` in `git_executor::concurrency`)

Sync pipeline (`sync_core::sync::sync_branches`):
- Phase 1, one `spawn_blocking`: issue config, baseline, commit grouping; sends `IssueNavigationConfig`,
  `UnassignedCommits` and `BranchesGrouped` (also when empty) before any per-branch work starts, so the
  frontend always knows the branch set before per-branch events arrive
- Phase 2, concurrently: integration detection in one `spawn_blocking`, and branch processing as
  semaphore-bounded `JoinSet::spawn_blocking` tasks; joined with `tokio::join!` so one failure doesn't orphan the other

### Operation spans

Every run of `run_blocking` (`src-tauri/src/blocking.rs`) is an operation with an id of its own
(`sync_utils::operation_id::OperationId`, a ULID: 48 bits of milliseconds, 80 random bits, so the ids of the app and
of every `bd` process never collide and sort by time):
- The span `operation{operation_id}` is created on the async side, a child of the current span, the `#[instrument]`
  span of the command (every Tauri command has one, its `State`, `AppHandle` and window arguments skipped, a long text
  such as a pull request body or a sub-window's data skipped for a field or two). The blocking closure enters it on its
  pool thread, where `Span::current()` would be empty: a `spawn_blocking` does not carry the caller's span.
- An error leaves with the id: `CommandError.operation_id` (an error of an inner run keeps its own), which the page
  records with its Activity entry (`operationId`) and in its log line of the error.
- `tracing_log` (`src-tauri/src/tracing_log.rs`) makes the spans reach the log at all: without a subscriber the `log`
  feature of `tracing` hands an event to `log` without its spans. The subscriber formats each event with its spans,
  `get_pull_requests{…}:operation{operation_id=…}: message`, plus a line when a span starts (`FmtSpan::NEW`, as
  `tracing` wrote one per span before), and gives the line to `log` with the event's level and target, which
  `tauri_plugin_log` writes. The devtools build keeps the subscriber of its plugin.
- `bd` opens the root span `operation{op, operation_id}` in `bd_cli::run` before the dispatch, `op` the words of the
  command (`lease get`); its `-v` lines carry it. Work a core hands to another thread (`JoinSet`, a scoped thread) runs
  outside the span unless the core enters it there.


## Alternatives Considered

- **Async Process Spawning (tokio::process::Command)**:
  - Why not: While tokio::process::Command provides async process spawning, git operations themselves are still blocking I/O. Using async process adds complexity without performance benefit over std::process::Command with spawn_blocking

- **Blocking in Async Functions**:
  - Why not: Blocks async worker threads, ruins performance

- **Separate Thread Pool Management**:
  - Why not: Tokio's blocking pool is well-tuned and sufficient

- **Actor Model with Channels**:
  - Why not: Adds complexity without performance benefit

## Performance Considerations

- **Constraint**: Limited async worker threads (typically CPU cores * 2)
- **Solution**: Never block async threads, use blocking pool (max 512 threads)
- **Measurement**: Git operations move off async threads in <1μs

Thread pool characteristics:
- Async workers: Must never block, handle coordination only
- Blocking pool: Grows on demand up to 512 threads
- spawn_blocking overhead: ~1-5μs per call
- Parallel speedup: Near-linear for independent git operations

## References

- Implementation: `src-tauri/src/commands/` - All Tauri commands
- Core functions: Synchronous functions called from commands
- GitCommandExecutor: `crates/git-executor/src/git_command_executor.rs`
- Tests: Much simpler without async - see any `#[test]` vs `#[tokio::test]`
- Related: [Tokio spawn_blocking docs](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)