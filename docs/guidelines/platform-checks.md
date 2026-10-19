# Checking Linux and Windows (Docker, Parallels)

**Status:** Active  
**Updated:** 2026-10-19  
**Tags:** [linux, windows, ci, cfg]

## Summary

CI runs clippy and the tests on macOS, Linux and Windows. A check on this Mac compiles only the macOS side of every
`#[cfg]` and runs only the macOS branch of every `cfg!`, so a change with platform code is checked on the other two
before it lands. Clippy for Windows from the Mac (`--target x86_64-pc-windows-msvc`) does not build: the C of
tree-sitter finds no Windows headers.
- **A platform branch in a test** asserts on every platform: an `if cfg!(target_os = "macos")` has an `else` that
  checks what the other platforms do (a `worktree add` where no APFS clone is), never a branch that asserts nothing.
  A helper or import that only `#[cfg(unix)]` tests use carries `#[cfg(unix)]` too, or Windows clippy fails on it.
- **Linux**: OrbStack Docker (`orb start` when `docker` cannot connect), the arm64 image `bd-linux-build` (ubuntu:26.04,
  rustup without a toolchain), the volumes `bd-linux-cargo` at `/root/.cargo/registry` and `bd-linux-target` at
  `/target` (`CARGO_TARGET_DIR`), the worktree at `/src`. Its git is 2.53, below the minimum: build git 2.55 from the
  kernel.org tarball (`env -u CARGO_TARGET_DIR make prefix=/usr/local NO_TCLTK=1 NO_PERL=1 NO_PYTHON=1 all install`,
  with `RUSTUP_TOOLCHAIN` the toolchain of `rust-toolchain.toml`: git builds a Rust part of its own), take nextest
  from `https://get.nexte.st/latest/linux-arm`, and run the tests as a user that is not root, as CI does: root reads
  a `0o000` file.
- **Windows**: the Parallels VM "Windows 11" (ARM64, 8 cores; `prlctl resume`, then `prlctl suspend`), the Mac home as
  `Z:`. Rust, nextest, git, MSVC Build Tools and LLVM are under `C:\dev\tools`. A `.bat` sets
  `CARGO_HOME=C:\dev\tools\cargo`, `RUSTUP_HOME=C:\dev\tools\rustup`, `CARGO_TARGET_DIR=C:\dev\bd-target` and `PATH`
  (`C:\dev\tools\cargo\bin`, `C:\dev\tools\git\cmd`, `C:\Program Files\LLVM\bin`), copies the worktree with
  `robocopy Z:\<path> C:\dev\bd /MIR /XD target node_modules .git dist /XF .git`, writes an empty `dist\index.html`
  for `src-tauri` and runs cargo with its output in a file under `Z:`; start it with
  `prlctl exec "Windows 11" --current-user cmd /c Z:\<path>.bat`. `/XF .git` matters: the `.git` of a lease is a file
  that names a Mac path, and a copied one fails every git whose cwd is in the copy ("not a git repository: (NULL)").
  The full suite takes about 19 minutes there.
- **Commands**: the clippy of CI, `cargo clippy --workspace --all-targets --features branch-deck/auto-update -- -D
  warnings`, then `cargo nextest run --workspace` (or `-E 'test(/…/)'` for the tests of the change).
