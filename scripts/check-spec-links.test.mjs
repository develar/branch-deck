import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { test } from "node:test"
import { checkSpecTargets, checkTestLinks } from "./check-spec-links.mjs"

/** A repository of `files` (path → text) in a temporary directory, removed after the test */
function repositoryOf(t, files) {
  const root = mkdtempSync(join(tmpdir(), "spec-links-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, dirname(path)), { recursive: true })
    writeFileSync(join(root, path), text)
  }
  return root
}

/** A spec whose front matter lists `targets` */
function specOf(targets, body = "- A requirement.") {
  return ["---", "name: Example", "targets:", ...targets.map(target => `  - ${target}`), "---", "", body, ""].join("\n")
}

test("a [@test] link names a file and the tests it declares, on a line of its own", t => {
  const root = repositoryOf(t, {
    "src/example.test.ts": 'test("shows the row", () => {})\n',
    "crates/example/src/lib_test.rs": "#[test]\nfn keeps_the_row() {}\n",
    "spec/area/example.spec.md": specOf(
      ["../../src/example.test.ts"],
      [
        "- A requirement.",
        "  [@test] ../../src/example.test.ts (`shows the row`, `hides the row`)",
        "  [@test] ../../crates/example/src/lib_test.rs (`keeps_the_row`)",
        "  [@test] ../../src/missing.test.ts (`shows the row`)",
        "- Another one. [@test] ../../src/example.test.ts (`shows the row`)",
      ].join("\n"),
    ),
  })
  assert.deepEqual(checkTestLinks(root), [
    "spec/area/example.spec.md:8: src/example.test.ts has no test `hides the row`",
    "spec/area/example.spec.md:10: no file src/missing.test.ts",
    "spec/area/example.spec.md:11: not a link of its own line, `[@test] <path> (`name`, …)`: - Another one. [@test] ../../src/example.test.ts (`shows the row`)",
  ])
})

test("every spec lists its targets once, each a ../../ path of a file that exists", t => {
  const root = repositoryOf(t, {
    "src/example.ts": "",
    "src/dir/file.ts": "",
    "spec/area/good.spec.md": specOf(["../../src/example.ts"]),
    "spec/area/bad.spec.md": specOf(["../../src/example.ts", "../../src/example.ts", "../../src/gone.ts", "src/example.ts", "../../src/dir"]),
    "spec/area/no-front-matter.spec.md": "# Example\n",
    "spec/area/no-targets.spec.md": "---\nname: Example\n---\n",
    "spec/area/empty-targets.spec.md": "---\nname: Example\ntargets:\nstyle: plain-1\n---\n",
    "spec/area/odd-line.spec.md": "---\ntargets:\n  - ../../src/example.ts\n  # a comment\n---\n",
  })
  assert.deepEqual(checkSpecTargets(root), [
    "spec/area/bad.spec.md:5: lists ../../src/example.ts twice",
    "spec/area/bad.spec.md:6: no file src/gone.ts",
    "spec/area/bad.spec.md:7: the target src/example.ts is no `../../` path from the root of the repository",
    "spec/area/bad.spec.md:8: src/dir is no file",
    "spec/area/empty-targets.spec.md: its `targets:` list no file",
    "spec/area/no-front-matter.spec.md: no front matter (`---` … `---`)",
    "spec/area/no-targets.spec.md: its front matter lists no `targets:`",
    "spec/area/odd-line.spec.md:4: not a target line, `  - ../../<path>`: # a comment",
  ])
})

test("a rule of domain-policy names its specs in its //! header, and each of them lists it", t => {
  const rules = "crates/domain-policy/src"
  const root = repositoryOf(t, {
    [`${rules}/lib.rs`]: "pub mod listed;\n",
    [`${rules}/listed.rs`]: "//! The listed rule (spec/area/rules.spec.md).\n\npub fn rule() {}\n",
    [`${rules}/listed_test.rs`]: "#[test]\nfn decides() {}\n",
    [`${rules}/unlisted.rs`]: "//! A rule of two specs (spec/area/rules.spec.md, spec/area/other.spec.md).\n",
    [`${rules}/unnamed.rs`]: "//! A rule that names no spec.\n",
    [`${rules}/below.rs`]: "//! A rule whose spec is named below its header.\n\n// spec/area/rules.spec.md\n",
    [`${rules}/missing.rs`]: "//! A rule of a spec that is gone (spec/area/gone.spec.md).\n",
    "spec/area/rules.spec.md": specOf([`../../${rules}/listed.rs`, `../../${rules}/unlisted.rs`, `../../${rules}/below.rs`]),
    "spec/area/other.spec.md": specOf([`../../${rules}/lib.rs`]),
  })
  assert.deepEqual(checkSpecTargets(root), [
    `${rules}/below.rs: its \`//!\` header names no spec (spec/<area>/<name>.spec.md) of the rule`,
    `${rules}/missing.rs: its \`//!\` header names spec/area/gone.spec.md, which does not exist`,
    `spec/area/other.spec.md: no target ../../${rules}/unlisted.rs, whose \`//!\` header names the spec`,
    `${rules}/unnamed.rs: its \`//!\` header names no spec (spec/<area>/<name>.spec.md) of the rule`,
  ])
})

test("every presenter of the page is a target of a spec, its tests aside", t => {
  const root = repositoryOf(t, {
    "src/presenters/listed.ts": "export const words = 1\n",
    "src/presenters/listed.test.ts": "",
    "src/presenters/unlisted.ts": "export const words = 2\n",
    "src/presenters/unlisted.test.ts": "",
    "spec/area/example.spec.md": specOf(["../../src/presenters/listed.ts"]),
  })
  assert.deepEqual(checkSpecTargets(root), ["src/presenters/unlisted.ts: a presenter that the `targets:` of no spec list"])
})
