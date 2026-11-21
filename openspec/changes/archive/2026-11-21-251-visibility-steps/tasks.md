## 1. One rule of Rust's visibility

- [x] 1.1 `rust_syntax::visibility` in psi-syntax (accio), `Reach::Up` and `Reach::Depth` of code-symbols, the revision
  of psi-syntax here; `resolve_rust` reaches by `Visibility::reaches_path`

## 2. Steps of a reach

- [x] 2.1 `reach_changes` and `follow_entries` give a key only present or absent a step of its reach (`binds_one`);
  the older form judges a function of Rust and TypeScript; a glob beside another glob or a prelude name accepts
- [x] 2.2 a narrower reach needs the commits that removed the uses it forbids (`removed_uses` with `still`)

## 3. `brd check`

- [x] 3.1 `change_file` of a missing declaration; `inverted_stack` of a missing declaration with `needs` and the fixes
  `move_to_parent` and `move_hunk`, proven by the missing declarations of the trial
- [x] 3.2 a session of `with_rewrite` reads the summaries of the blobs of its rewrite
