# Symbol Dependencies

**Status:** Implemented (`branchdeck.symbols on` by default)
**Updated:** 2026-10-17
**Tags:** [dependencies, tree-sitter, bd, check, suggest, conflict-fix]

## Problem

Branch Deck decides that commits belong together, or that one branch must stack on another, from text: a commit
depends on an older one when its hunks touch the lines that commit added (`dependencies.rs`), clusters are scored by
issue keys, scopes and shared paths (`bd suggest`), and the dangling-reference check finds the uses of a removed
declaration by per-language regular expressions. git's three-way merge is the only exact oracle, and it sees lines,
not names: a commit that calls a function another commit added in another file merges cleanly into a branch without
it, and the branch fails only when it is compiled.

The regular expressions also see too much. Regrouping 217 commits of the IntelliJ checkout (2026-10-17), the check
reported a Rust `pub(crate) use …::ControlOwner` as the removal of `ControlOwner` (four false uses), a word in a KDoc
comment as a use of a removed member, and a local `val turnId` as a use of a private property of another class. The
false references became repair edges of `bd suggest`, closed a cycle, and stacked a branch on the wrong parent; a
proposed `bd move-hunk` would have broken a branch.

## Solution Overview

`crates/code-symbols` summarizes one source file on tree-sitter (Kotlin, Java, Rust, TypeScript and JavaScript): its
package, its imports (Rust `use` and `pub use`, Kotlin aliases, TypeScript imports and re-exports), its declarations
with their containers and reach, and its references, without comments, strings (but with interpolations), locally
bound names, labels and keys. Of Kotlin and Java it also holds the parameter types of each function, whether a member
overrides and whether a subtype may override it, the supertypes of each type, per `override` member one reference
to the member it overrides in each supertype of the class around it, and per call the shape of its arguments (named,
spread, a text, a number, a boolean, a lambda or another expression) and whether a lambda, an object expression or an
anonymous class around it may bring a receiver of its own. A summary depends only on the bytes of the blob, so it is
cached by blob id.

Two consumers use the summaries. The dangling-reference check keeps finding candidate lines as before and only
subtracts the ones the tree places in a comment, a string or a local scope, or whose name an import still binds, and
the line of an `override` member. And
`symbol_dependencies` adds edges of a new class, `Order`, to the line dependencies: a commit that uses a name another
commit of the range adds, with evidence that the use means that declaration, or overrides a member another commit
adds, also a new overload of a name its type had, which a call reaches too when its arguments select it.

## Design

### Architecture

```mermaid
flowchart TD
    D[diff_scan: one log -p -U0 --raw --no-abbrev] -->|blob ids, hunks| S[symbol_scan]
    R[module_roots: one cat-file of the trees at HEAD] --> S
    S -->|cache misses + Cargo.toml + tsconfig| C[one cat-file --batch of the blobs]
    C --> P[code_symbols::extract::summarize, one thread per core]
    P -->|not truncated| O[(object cache: blob_summary)]
    O --> S
    S --> G[symbol_dependencies: Order edges, entities]
    G --> B[bd commits: via, symbols]
    G --> F[conflict fix, stack independence]
    G --> U[bd suggest: Order edges, entity features]
    S2[check session: blobs of the trials] --> M[SyntaxMemo, memory only] --> X[dangling references: subtraction]
```

### Key Decisions

1. **Decision**: an edge by symbols is of its own class, `Order`, not a line dependency.
   **Rationale**: a line dependency means the two commits can't be cherry-picked apart without a conflict, and a
   trial build refutes a wrong one. A symbol dependency means the later commit doesn't compile without the earlier
   one; a stack or the order of two Safe Pushes satisfies it, and `merge-tree` can never refute it. Joining units by
   it would fuse every issue that uses a shared helper into one branch.

2. **Decision**: the index is the declarations the range adds or changes, not the repository.
   **Rationale**: only those can make one commit of the range need another, and they are a few hundred even in a
   monorepo of 1.4 million files; no build, no language server and no index of the tree is needed. A member key a
   commit changes brings the overloads its file had before the range along (decision 7).

3. **Decision**: a use links only with evidence: an import or alias, a qualified name, the same file, the same Rust
   module, a wildcard import or the same package and module for a type, or an override in a subtype whose supertype
   resolves (`override`, decision 8). In the same file a name that a type other than the declaration's own qualifies
   (`Registry.register(id)` in a `register` of the file) means that type's member, not the file's; a type by its
   capital, the convention of every language here, unless the file declares a value of that name
   (`DEFAULT.register(id)` of a `val DEFAULT`, an `INSTANCE`, an enum constant). A name in capitals that the file only
   imports counts as a type (`JBUI`, or a constant): an import doesn't say which it is.
   **Rationale**: a Kotlin call without a qualifier may mean an implicit receiver's member, an extension or a function
   of the package; only a compiler knows. A wrong hard edge costs more than a missed one, which the trial build and
   `--build` still catch.

4. **Decision**: the dangling-reference check subtracts by the tree and adds nothing.
   **Rationale**: its recall comes from matching names in any position (a Kotlin extension call, a Java accessor); the
   tree is good at saying what is not a use (a comment, a string, a local), and bad at saying everything that is.

5. **Decision**: a table-driven walk of the tree, not tree-sitter queries.
   **Rationale**: shadowing needs a walk of the scopes anyway, the shipped `tags.scm` files are too thin, and a test
   checks every node kind of the tables against the grammar, so a grammar upgrade can't silently break a rule.

6. **Decision**: `tree-sitter-kotlin-sg`, and one TSX grammar for TypeScript and JavaScript.
   **Rationale**: the maintained Kotlin grammar (the fwcd main, republished by ast-grep); `-ng` is dormant and parses
   far worse, and both export the same C symbol. TSX parses TypeScript and JavaScript but `<T>expr` casts, which make
   a file degraded and send it back to the lines; one grammar less is about 1 MB of binary.

7. **Decision**: a key (namespace, containers and name) holds entries, not one declaration. An entry is one
   declaration of the key, its identity the kind, the extension receiver and the parameter types (the last names of
   each type, `?` kept, and the dimensions of an array, a vararg counted as one). Its origin is the baseline, or the
   commit that created it or changed its form, with the entries that commit removed as its predecessors: one removed
   and one added is the same entry in a new form. A use with a shape (an override, a call by its arguments) looks at
   the entries that accept it at its commit; for each, the latest step back along its predecessors at which no form
   before accepted the use and the form after did names the commit it needs. It links only when that is one older
   commit for all of them: the baseline, its own commit or two commits are no edge. A call's arguments fit an entry
   when the positional ones fill its parameters in order (a vararg, the last, the rest), the named ones parameters of
   their names, a last lambda the last parameter or its place, and the parameters left have a default or are a
   vararg; a text, a number or a boolean fits no parameter of another of these families (`String` and
   `CharSequence`, the number types and `Number`, `Boolean`) and no class, interface, object or enum that the member's
   file declares and doesn't shadow by an import of its name. A spread, a named argument before a positional one, a
   vararg before the last parameter and an entry without a signature (a type, a property) accept any call. The
   arguments tell the overloads apart only of a type without a supertype but `Any` or `Object`, not for a name every
   class has (`toString`), and not for a bare name that an import brings in when its file shows something that may
   take it first: an enclosed call (in a lambda, an anonymous function, an object expression, an anonymous class or a
   local declaration, which the summary marks), a declaration of its name in the file, another import that may bring
   the name (a second one of it, a wildcard import, in Kotlin a function of the default imports), or a type or a
   Kotlin extension around it with a supertype or a receiver of its own. A use that tells no overloads apart (a type,
   a value, a call of a type with a supertype) needs the key: the commit that added it, the oldest origin of the
   entries that accept it, and none when one is of the baseline. In the file of a key of the baseline a use means it
   only where it can mean only a member of its type: a bare name, `this` or `super` whose innermost type is that type,
   not in an enclosed call nor in a Kotlin member extension (a function, or an accessor of a property, with a receiver
   of its own), and in Kotlin without a function or value of its name in the receivers further out (a type around it,
   the companion of one) or at the top level of the file, without a supertype of any of these receivers, and without
   a function of another file that the file may bring in by the name: an import of the name, a wildcard import, or a
   function of the default imports (`is_default_function`, the common ones: `error`, `require`, `println`, `listOf`,
   `let`).
   **Rationale**: a new overload of a name its type already declares, `buttons(paneId: String)` beside
   `buttons(window: Window)`, is the declaration an override needs (the Safe Push of IJAI-4074 that failed on
   2026-10-17), but a key of name only found it on both sides of the file, so it was neither added nor looked up.
   "Accepts" errs on purpose toward accepting where a wrong match only adds an origin (decision 8 says where for an
   override): a form that accepts a use it in fact rejects takes the step further back, or keeps an old overload a
   candidate, toward no edge, while a wrong rejection makes one. So only a literal judges a type, and only against a
   family no compiler converts it across (Java widens an `int` to a `double`, so a number fits every number type) or
   a class of the member's file, which a literal never is: `buttons("x")` selects `buttons(paneId: String)`, not
   `buttons(window: Window)` with `Window` of the same file. A supertype may declare overloads that the range doesn't
   show (`Base.buttons(Long)` takes `buttons(1)`), and every class has those of `Any` or `Object`, so such a call
   needs the key as before the entries: the call of a name with an overload of the baseline is no edge, and a call of
   a name whose overloads two commits of the range add needs the first. Where the arguments decide, a call that the
   overloads of two commits both accept is no edge, though the key alone linked it to the first. Once a call can
   select a new overload of a key of the baseline, a use in its file that means a member of another receiver would
   link to it, `list.add(0, item)` of a `MutableList` beside a new `Lanes.add(index, item)`, and so would
   `buildList { add(0, item) }` in the type, whose lambda's receiver is the list, an `add(0, item)` in an object
   expression or a Java double-brace initializer that extends `ArrayList`, or in a member extension
   `fun MutableList<String>.fill()` of the type, whose extension receiver comes before the type, or a `foo(1)` in a
   Kotlin inner class whose old `foo(x: String)` rejects it, which Kotlin then gives to the outer class's
   `foo(x: Int)`. Past the receivers Kotlin falls back to the top level: the functions the file declares or imports by
   name, those of its package, of its wildcard imports and of the default imports, so `error("boom")` beside an old
   member `error(code: Int)` is `kotlin.error`, and `add(0, "x")` in a file with `import q.add` is that one. So such
   a key takes a use of its file only where nothing can come before its type: no receiver the summary doesn't show (an
   enclosed call, a member extension) and no other declaration of the name, import or supertype that the call falls
   back to. A key the range adds keeps the same file as evidence anywhere in it, as before (a bare call in an
   extension of its type at the top level). The same holds for a bare name an import brings in: a method `add(int)`
   the class inherits from `Base` comes before `import static p.Util.add` (JLS 6.4.1), a member of a Kotlin implicit
   receiver before an import, and Java weighs every static import of a name together (`import static p.Util.add`
   shadows the `add` of `import static q.Other.*` only for the same signature, so `Other.add(int)` takes `add(1)`),
   while Kotlin falls back from the import to the wildcard and the default imports. A wildcard import counts in
   either language, as the summary doesn't mark a static one. What the range doesn't show stays a limit: an extension
   of the member's type declared elsewhere (`fun Registry.register(id: Int)`) takes a call that the old member
   overloads reject, and so does a top-level function of the package in another file for a bare name, or a function
   of the default imports that the list leaves out, so such a call links to a new overload it doesn't need.
   Only the members of a JVM type: all their overloads are in the file of the type, which the commit that changes them
   changes too. A top-level function may have overloads in files the range doesn't change, so its key, as every key of
   Rust and TypeScript, is only present or absent; each file of the range that adds it holds its own entries, so a call
   of an overload that two files add is ambiguous, and a removal drops the entries of its file (the whole key of a
   member). Entries change only on clean sides: a declaration the parser recovered around (`in_error`) or a degraded
   file may be misread, and a misread form would look new; nor do they change when two declarations of one side are the
   same entry (`stamp(java.sql.Date)` beside `stamp(java.util.Date)`), which can't be matched across the sides. A use
   its own commit's declarations may mean is no edge either: the rule before dropped them and linked a use that a type
   of its own commit shadows to a wildcard import. A bare name outside the type of a member its own commit adds or
   changes is no use of that member, though: only an implicit receiver, which the file doesn't show, makes it one, and
   counting it would cost the edge of the function of that name the file imports.

8. **Decision**: a Kotlin `override` or Java `@Override` member is a use, `override`, of the member it overrides in a
   supertype of the class around it: a subclass, an object expression, an anonymous class, an enum entry's body.
   **Rationale**: an override of nothing doesn't compile, so a resolved supertype is evidence as strong as
   `Type.member` with a resolved type. The member must be an instance member a subtype may override (`open`,
   `abstract` or of an interface, never `final` or private) with the same receiver and parameter types, must not be
   one every class has from `Any` or `Object` (`equals`, `hashCode`, `toString`, `clone`, `finalize`), and must
   override nothing itself: an override of an override compiles without the commit that adds it, because the
   supertype of its type has the member. Where the member's type has no supertypes, the override must mean one of its
   members, each overload of which is an entry, so the parameter types match generously (a type parameter fits any
   type, two types of the same last names are the same): a wrong match only adds an origin, which costs an edge and
   never adds one. Where it has supertypes, whose members are unknown, only the same form counts: every type known,
   each name resolved by its file's imports, its own types or its package to the same type. Two names neither file
   resolves are the same when equal only in one package, or when they are of the common default imports (`String`,
   `int`, `StringBuilder`, `IllegalStateException`, Kotlin's `HashMap`; a list of the types a project rarely names
   anew, so `Module` or `Result` is left out): in two packages each may mean a type of its own package or of a
   wildcard import, also when both files have the same wildcard import, so such a name is unknown and the override is
   no edge. The supertype resolves by the same file, its full path, an import (an alias, a nested type) or the same
   package in the same module without a type of that name in the file, never by a wildcard import, and its path must
   be the containers of the member. A type of the name nested in a declaration around the override (`Outer.Base`
   inside `Outer`) may be the one it means, and so may, in Java, a member type that a class around the overriding
   class inherits: no edge. Kotlin also reaches the nested classes of a supertype by
   their names; that is left out, because an object expression in a class with supertypes is everywhere and such a
   shadow is rare. A class with several supertypes, a function type among them, is no edge: its member may come from
   any, and the members of a type outside the range are unknown. Nor is a Java method of a class with supertypes,
   which may override one without `@Override`. An override in a one-line object expression, which the grammar reads
   as a call with a trailing lambda, or in a declaration the grammar misread overrides nothing of the class around
   it. An override that an earlier commit removed is a removed use: the commit that then removes the member it
   overrides depends on that one (`removes_after`).

### Implementation Notes

- `crates/code-symbols`: `summarize`, `FileSummary`, `extractor_version` (the rules and the grammars' node types);
  `Spec::inheritance` of Kotlin and Java: the signatures, `overrides` and `overridable`, the supertypes, and the
  `Role::Override` references with their `Shape::Override`; `Spec::call_shape` of Kotlin and Java: the `Shape::Call`
  of a call's arguments on the reference of its callee, `enclosed` by the depth of the stack of supertypes
  (`Walk::enclosed`).
- `crates/sync-core/src/symbol_scan.rs`: the mode (`branchdeck.symbols`), the summaries through the cache, the crate
  names and the TypeScript aliases.
- `crates/sync-core/src/module_roots.rs`: the nearest directory with a build file (Kotlin `internal`, Rust crates,
  TypeScript packages).
- `crates/sync-core/src/symbol_dependencies.rs`: the `Order` edges with their evidence, the entries of a member key
  and their origins (`member_entries`, `origins_of`), the overrides (`resolve_override`, `fits`, `scope_may_shadow`),
  the calls (`calls_tell_apart`, `accepts_call`, `fits_literal`, `inherits`, `import_decides`), the same-file rule
  (`names_another_type`, and `of_baseline` with `means_its_type` for a key of the baseline; `receivers` and
  `imports_of_function` with `is_default_function` for both), the removals that need the removal of the uses, the
  entities each commit changes.
- `crates/sync-core/src/dangling_references.rs`: `SyntaxMemo` and the subtraction; an `override` line is no use
  (`on_syntax`).
- `crates/sync-core/src/dependencies.rs`: `with_symbols` merges the edges into the line dependencies (`Dependency::lines`,
  `symbols`) for `bd commits`.
- `crates/sync-core/src/check.rs`: `CheckSession::symbol_graph`, read once per session with `on`, for the conflict fix
  (`dependents_in`, `movable`), `missing_declarations` (`crates/sync-core/src/missing_declarations.rs`) and
  `stack_independence` (`Builds.uses`).
- `crates/bd-cli/src/clustering.rs`: `ClusterInput.symbol_dependencies`, an order of the clusters, never a join;
  `crates/bd-cli/src/suggest.rs`: the `symbol_dependency` warning.

## Alternatives Considered

- **SCIP, rust-analyzer, Kythe, scip-java**: exact names. Why not: each needs a build of the user's project (a Gradle
  or Bazel model, minutes to hours on a monorepo) that Branch Deck can't run or wait for.
- **GitHub stack-graphs**: name resolution on tree-sitter. Why not: archived in 2025 and never had Kotlin.
- **sem-core**: entity diffs and an impact graph in Rust. Why not: its API breaks every few releases, and it resolves
  neither Kotlin nor Java imports.
- **Regular expressions per language** (as before): Why not: they can't tell a comment, a string or a local from a use.
- **The entities two commits change as features of P** (the functions and types around their hunks): Why not: in the
  IDF-weighted Jaccard of `bd suggest` a shared function adds 0.1–0.2 to P, short of the threshold, and commits of
  one function already share its file. `bd commits --symbol-stats` still counts the pairs (`entity_pairs`).

## Measurements

Round 1, 2026-10-17, release build, a machine at a load average of 30–60 (wall times are noisy; the CPU times agree):

| Range | Commits | Blobs (MiB) | Read cold / warm | Parse | Graph | Edges by lines / symbols / both / symbols only |
|---|---|---|---|---|---|---|
| IntelliJ checkout, the IJAI air work | 225 | 2647 (52.6: Kotlin 1827, Rust 629, TS 169, Java 22) | 4.8 s / 0.15–0.29 s | 0.64 s | 0.56–0.61 s | 891 / 738 / 454 / 284 |
| branch-deck, its last 200 commits | 200 | 3453 (33.8: Rust 1261, TS 2183) | 1.06 s / 0.37 s | 0.41 s | 0.69 s | 1286 / 654 / 529 / 125 |

- No file of either range is degraded or cut by the budget; 45 error bytes in one Kotlin file of branch-deck.
- The symbols-only edges of the IntelliJ range by evidence: same file 177, removal after the removal of the uses 38,
  import 48, qualified 11, type of the same package and module 10. A read of the sample found each a real compile
  dependency: an import of what the older commit adds, a registry line that names an object the older commit declares
  in the same file, a removal of `AirJourneySuites` after the commits that moved the suites off it.
- Peak memory: `bd commits --brief` alone takes 530 MB on the IntelliJ checkout (the mapped object cache); the scan adds
  about 160 MB cold and nothing warm.
- The dangling-reference check on the IntelliJ range at 7dfac19d: by the lines 6 confirmed references (`ControlOwner`
  four times behind a `pub(crate) use`, `scenarios` in a KDoc, the local `turnId`) and two inverted stacks without a
  fix; with the subtraction none, and no other reference changed. The current IntelliJ grouping (28 branches) and
  branch-deck's 200 commits: the same references either way.
- Precision of the edges by symbols only (the IntelliJ range at 7dfac19d, 246 without the removals): for 101 the name
  occurs nowhere in the scope before the commit that adds it (`git grep -w` at its parent), so the use can mean only
  that declaration; a read of 40 of the other 145 (the name occurs in a `BUILD.bazel`, an `.iml`, another crate's
  file, or is an import whose path names the adding module) found no false one either: 141 of 141, a Wilson lower
  bound of 0.97. No compiler checked them.
- Round 2, `bd suggest` on the same range: with the symbols on and off the same plan, `check.ok`, no repair round;
  the edges by symbols are all satisfied by the stacks the lines already make. Warm, the symbols add 0.5–0.7 s of
  CPU (+20–28 %).
- Binary: the four grammars add 9.4 MiB to the app (Kotlin 6.2, TSX 2.0, Rust 1.6, Java 0.9; the app is 24.3 MB, was
  15.0); the user accepted it.
- Round 3, 2026-10-17, the overload entries and the `override` evidence (decisions 7 and 8), release build, a heavily
  loaded machine:
  - The repro of the IJAI-4074 failure: an override of a new overload, of a new name, through an import and in the same
    file each make exactly one edge by symbols; a call of an overload added in the same file was still none in this
    build, before calls carried their arguments (decision 7 now links it). On the kept
    override-overload case `bd check` is `ok` with one `missing_declarations` (`buttons`, evidence `override`, the
    override of the new `paneId` overload), stacks on or off; `bd suggest` with stacks off keeps the plan and warns
    `symbol_dependency` ("Safe Push ABC-1 first, or turn on stacked branches"), with `branchdeck.stacks true` it stacks
    XYZ-2 on ABC-1, and the check of that plan is `ok`.
  - The IntelliJ range at 1d7255a6 (60 commits on 596ffa8e): 0 edges by symbols, as before; the file-overlap edges, the
    `bd commits` and `--brief` output, `bd check` (`ok`, 16 branches, no `missing_declarations`, no dangling
    reference) and the `bd suggest` plan are the same as the old build's. The range has no edge by symbols to compare,
    so it says little about precision.
  - Of its overrides 120 (reference, entry) pairs reach `resolve_override`, 9 distinct references pass the kind, the
    overridability and the reach; one is skipped for its class of 5 supertypes. A read of each found its missing edge right:
    the member is of the baseline or of the overriding commit (`getButtonManager` of `ToolWindowUnregisterTest`, after
    the user's fixup, overrides only the overload of the baseline: the precision the fix needs, on the real case); the
    skip loses nothing.
  - The changed `extractor_version` invalidates the cached summaries: the first run parses all 429 again (0.62 s read,
    0.61 s parse); warm 429 cached, graph 11–14 ms (was 90); entities 937 and pairs 19 unchanged.
  - The IntelliJ range 86be3572..7dfac19d (217 commits, the air work of round 1), without the cache (the extractor
    versions differ), against bd 0.17.0; each new and each lost link (from, to, name, file of the use) read in the
    commits. With the override entries (decision 8): 3601 links by symbols (was 3604), 728 edges by symbols (445 both,
    283 symbols only; was 732: 448, 284). The 60 new links are all right: 45 `override`, among them
    `getButtonManager(paneId)` of IJAI-4074 on IJPL-257337, the one new symbols-only edge and the failure of
    2026-10-17; 14 top-level names that several files of a package or module declare, of which a key held one site,
    the last file's (a Rust key has no crate name, so `testing::run_stream` of one crate hid another's); one
    `LaneIdentity::new` that `names_another_type` tells from the file's `Stream::new`. Of the 63 lost, 45 were false:
    31 a name its own commit declares too (a split, a rename, a move), which the rule before dropped and linked to
    another member of the file, and 14 by `names_another_type` (`Vec::new`, `Box::new` to the file's `fn new`;
    `Script::default()`, which removes the two false symbols-only edges of `default`). 18 were right, each in an edge
    that keeps 2–40 other links: a module or function of another commit beside a field or method of the name that its
    own commit adds (`registry::start`, `use super::services::codex`).
  - The same range with the call shapes (decision 7): 3596 links, 729 edges (446 both, 283 symbols only). Against
    0.17.0 the same 60 new links; against the build before 4 more, all right: `lane`, `codex` and `call` (a bare name
    outside the type of a member its own commit adds is no use of it) and `agents`, a removal after the removal of
    `document.agents`, which the read above judged false. Lost against the build before 9, all false: 7 calls whose
    own commit changes the signature of a private function and calls the new form, which the arguments tell from the
    older commit's (`runProgram`, `executeAuthored`), and 2 `findings.run(…)` of a type of their own commit. Against
    0.17.0 53 of the 68 lost were false; the 15 right ones are each in an edge that keeps 6–40 other links. The edges
    against 0.17.0: one right symbols-only edge more, the two false ones fewer, 3 by lines only instead of both (their
    lost links false), 1 both instead of lines; the same `units`. New links right: 60 of 60, and 4 of 4.
  - `bd check` on that range is byte for byte the same in all three builds (today's `refs/branch-deck/*` break the
    grouping of the clone: 16 conflicts, 15 blocked, IJAI-4074 among them, so no `missing_declarations` is reached).
    `bd suggest` differs from 0.17.0's in one place, the same in both builds: `air-lane-document` stacks on
    IJPL-257337 ("because a branch above needs both"), by the right edge of IJAI-4074, though the whole stack now
    waits on a platform branch for one test commit; `check.ok`. At a low load the three builds take the same time:
    about 1.4 s wall, 130 ms read, 350–380 ms parse, 410–460 ms graph.
  - With the call shapes each of the five repro cases makes exactly one edge by symbols: the call of an overload added
    in the same file links with `same_file`, and `bd check` reports it as the override case. On the real pair (the
    IntelliJ checkout at f0596b09) `bd check` names IJAI-4074's `getButtonManager` (evidence `override`, declared in
    IJPL-257337), the one edge by symbols of 18 in `bd commits --brief`; both outputs byte for byte those of the build
    before.

### The gate

| Measure | Threshold | Measured |
|---|---|---|
| Precision of the edges by symbols only | ≥ 0.95 | 141 / 141 (by `git grep` and reading, no compiler) |
| `bd suggest` plan with them | not worse | the same plan, `check.ok`, no repair round |
| Degraded Kotlin files | ≤ 3 % | 0 of 1827 |
| Symbol phase, warm | ≤ 0.3 s | 0.15 s read + 0.5 s graph |
| `bd suggest` slowdown, warm | ≤ 25 % | +20–28 % CPU |
| Peak memory | ≤ +250 MB | +160 MB cold, nothing warm |
| Binary | accepted by the user | +9.4 MiB |

The two times miss their thresholds, and the measured range gains nothing in `bd suggest`; the user chose `on` as the
default anyway (2026-10-17): the precision holds, no plan got worse, and the cost is a fraction of a second warm. `on`
adds the conflict fix by the edges, `missing_declarations` of `bd check`, the `symbol_dependency` warnings and the
stacking of `bd suggest`, and the uses of the Safe Push lead. `git config branchdeck.symbols report` keeps only what
decides nothing (`bd commits` names the edges, the dangling-reference check subtracts by the trees); `off` reads no
summary.

## References

- [Git Operations](git-operations.md): "Symbol summaries", "Dangling references (check)"
- [Symbol Dependencies spec](../../spec/commit/symbol-dependencies.spec.md)
