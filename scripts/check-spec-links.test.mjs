import assert from "node:assert/strict"
import { globSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { test } from "node:test"
import { checkOldLayout, checkOpenSpec, checkSpecAnchors, checkSpecTargets, checkTestLinks, headingAnchors, openspecValidate } from "./check-spec-links.mjs"

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
    "spec/area/odd-line.spec.md:4: not a target line, `  - <path>`: # a comment",
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
    `${rules}/below.rs: its \`//!\` header names no spec (openspec/specs/<area>/<name>/spec.md) of the rule`,
    `${rules}/missing.rs: its \`//!\` header names spec/area/gone.spec.md, which does not exist`,
    `spec/area/other.spec.md: no target ../../${rules}/unlisted.rs, whose \`//!\` header names the spec`,
    `${rules}/unnamed.rs: its \`//!\` header names no spec (openspec/specs/<area>/<name>/spec.md) of the rule`,
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

test("on Windows, where the glob answers paths with `\\`, every check answers the paths with `/` as everywhere", t => {
  const rules = "crates/domain-policy/src"
  const root = repositoryOf(t, {
    [`${rules}/lib.rs`]: "pub mod listed;\n",
    [`${rules}/listed.rs`]: "//! The listed rule (spec/area/rules.spec.md).\n",
    [`${rules}/unlisted.rs`]: "//! A rule the spec doesn't list (spec/area/rules.spec.md).\n",
    "src/presenters/listed.ts": "",
    "src/presenters/unlisted.ts": "",
    "src/example.test.ts": 'test("shows the row", () => {})\n',
    "spec/area/rules.spec.md": specOf(
      [`../../${rules}/listed.rs`, "../../src/presenters/listed.ts"],
      ["- A requirement.", "  [@test] ../../src/example.test.ts (`shows the row`, `hides the row`)"].join("\n"),
    ),
  })
  const windowsGlob = (patterns, options) => globSync(patterns, options).map(path => path.replaceAll("/", "\\"))
  const expected = [
    `spec/area/rules.spec.md: no target ../../${rules}/unlisted.rs, whose \`//!\` header names the spec`,
    "src/presenters/unlisted.ts: a presenter that the `targets:` of no spec list",
  ]
  assert.deepEqual(checkSpecTargets(root), expected)
  assert.deepEqual(checkSpecTargets(root, windowsGlob), expected)
  const links = ["spec/area/rules.spec.md:9: src/example.test.ts has no test `hides the row`"]
  assert.deepEqual(checkTestLinks(root), links)
  assert.deepEqual(checkTestLinks(root, windowsGlob), links)
})

/** A spec of OpenSpec whose front matter lists `targets` and whose requirements are `requirements` */
function openSpecOf(targets, requirements) {
  return [
    "---",
    "name: Example",
    "targets:",
    ...targets.map(target => `  - ${target}`),
    "---",
    "",
    "# Example",
    "",
    "## Purpose",
    "",
    "What the example capability is for, in a sentence long enough for the strict validation.",
    "",
    "## Requirements",
    "",
    requirements,
    "",
  ].join("\n")
}

const REQUIREMENT = [
  "### Requirement: Rows: The row shows its name",
  "",
  "The row MUST show the name of its branch.",
  "",
  "#### Scenario: A branch row",
  "- **WHEN** the table lists a branch",
  "- **THEN** its row shows its name",
  "  [@test] src/example.test.ts (`shows the row`)",
].join("\n")

test("under openspec/ a [@test] path is from the root, a path from the file is a problem", t => {
  const root = repositoryOf(t, {
    "src/example.test.ts": 'test("shows the row", () => {})\n',
    "openspec/specs/area/example/spec.md": openSpecOf(["src/example.test.ts"], `${REQUIREMENT}\n  [@test] ../../../../src/example.test.ts (\`shows the row\`)`),
    "openspec/changes/add-row/specs/area/example/spec.md": "## ADDED Requirements\n\n  [@test] src/example.test.ts (`hides the row`)\n",
    "openspec/changes/archive/2026-11-09-old/specs/area/example/spec.md": "  [@test] src/gone.test.ts (`shows the row`)\n",
  })
  assert.deepEqual(checkTestLinks(root), [
    "openspec/changes/add-row/specs/area/example/spec.md:3: src/example.test.ts has no test `hides the row`",
    "openspec/specs/area/example/spec.md:23: the path ../../../../src/example.test.ts is not from the root of the repository, as every path under openspec/",
  ])
})

test("a spec of openspec/specs lists its targets from the root, and a rule names it by its path", t => {
  const rules = "crates/domain-policy/src"
  const root = repositoryOf(t, {
    "src/example.ts": "",
    [`${rules}/listed.rs`]: "//! The listed rule (openspec/specs/area/example/spec.md).\n",
    [`${rules}/unlisted.rs`]: "//! A rule the spec doesn't list (openspec/specs/area/example/spec.md).\n",
    "openspec/specs/area/example/spec.md": openSpecOf(["src/example.ts", `${rules}/listed.rs`, "../../../../src/example.ts"], REQUIREMENT),
  })
  assert.deepEqual(checkSpecTargets(root), [
    "openspec/specs/area/example/spec.md:6: the target ../../../../src/example.ts is not from the root of the repository, as every path under openspec/",
    `openspec/specs/area/example/spec.md: no target ${rules}/unlisted.rs, whose \`//!\` header names the spec`,
  ])
})

test("the anchors of the headings are those GitHub gives them, a repeated one numbered, none of fenced code", () => {
  const text = ["# Title", "### Requirement: `brd push`: Refuses ‹branch›, “quoted”", "## Repeated", "## Repeated", "```md", "## Not a heading", "```"].join("\n")
  assert.deepEqual([...headingAnchors(text)], ["title", "requirement-brd-push-refuses-branch-quoted", "repeated", "repeated-1"])
})

test("a link into a spec of OpenSpec names one of its headings or a group of its requirements", t => {
  const root = repositoryOf(t, {
    "openspec/specs/area/example/spec.md": openSpecOf([], REQUIREMENT),
    "openspec/specs/area/old/spec.md": "# Old\n\n## Requirements\n\n### Rows\n",
    "openspec/specs/area/other/spec.md": openSpecOf(
      [],
      [REQUIREMENT.replaceAll("Rows: The row shows its name", "Other: One"), "See [rows](../example/spec.md#rows) and [gone](../example/spec.md#gone)."].join("\n"),
    ),
    "docs/design/example.md": [
      "[purpose](/openspec/specs/area/example/spec.md#purpose), [requirement](../../openspec/specs/area/example/spec.md#requirement-rows-the-row-shows-its-name)",
      "[old](../../openspec/specs/area/old/spec.md#anything), [missing](../../openspec/specs/area/missing/spec.md), [plain](../../openspec/specs/area/example/spec.md)",
    ].join("\n"),
    "openspec/changes/archive/2026-11-09-old/proposal.md": "[gone](../../../specs/area/example/spec.md#gone)\n",
  })
  assert.deepEqual(checkSpecAnchors(root), [
    "docs/design/example.md:2: no file openspec/specs/area/missing/spec.md",
    "openspec/specs/area/other/spec.md:22: openspec/specs/area/example/spec.md has no heading of the anchor #gone",
  ])
})

test("openspec validate --strict runs over the specs of its format and the changes, a spec in the old format aside", t => {
  const root = repositoryOf(t, {
    "openspec/specs/area/example/spec.md": openSpecOf([], REQUIREMENT),
    "openspec/specs/area/weak/spec.md": openSpecOf([], "### Requirement: Rows: Weak\n\nThe row shows.\n"),
    "openspec/specs/area/old/spec.md": "# Old\n\n## Summary\n\nNot yet moved.\n",
  })
  assert.deepEqual(checkOpenSpec(root, globSync, openspecValidate), [
    "openspec spec area/weak: ERROR requirements.0.scenarios: Requirement must have at least one scenario",
    "openspec spec area/weak: WARNING requirements[0].scenarios: Requirement must have at least one scenario. Scenarios must use level-4 headers. Convert bullet lists into:",
    'openspec spec area/weak: WARNING requirements[0]: Requirement "Rows: Weak" should contain SHALL or MUST (RFC 2119 best practice for English specs)',
  ])
  const asked = []
  const validate = (_, args) => {
    asked.push(args.join(" "))
    return { items: [{ id: "add-row", type: "change", valid: false, issues: [] }] }
  }
  const changes = repositoryOf(t, { "openspec/changes/add-row/proposal.md": "## Why\n" })
  assert.deepEqual(checkOpenSpec(changes, globSync, validate), ["openspec change add-row: invalid"])
  assert.deepEqual(asked, ["--changes"])
  assert.deepEqual(checkOpenSpec(repositoryOf(t, { "spec/area/example.spec.md": "" }), globSync, validate), [])
})

test("no spec lies under spec/ and no file names one by its old path, a fixture of another repository and the history aside", t => {
  // built of its parts, as this file names no spec of the old layout itself
  const old = ["spec", "branch", "unapply.spec.md"].join("/")
  const files = {
    [old]: "",
    "crates/a/src/lib.rs": `//! Unapply (${old}).\n`,
    "docs/design/a.md": `x\n[unapply](../../${old}#undo)\n`,
    "src/fixture.test.ts": 'const path = "plugins/air/spec/frame/window.spec.md"\n',
    "CHANGELOG.md": `${old}\n`,
    "openspec/changes/archive/2026-11-09-a/proposal.md": `${old}\n`,
    "openspec/specs/branch/unapply/spec.md": "",
  }
  const root = repositoryOf(t, files)
  assert.deepEqual(checkOldLayout(root, Object.keys(files)), [
    `${old}: a spec of the old layout, which moved to openspec/specs/<area>/<name>/spec.md (docs/contributing/specs.md)`,
    `crates/a/src/lib.rs:1: names ${old}, a spec of the old layout: openspec/specs/<area>/<name>/spec.md`,
    `docs/design/a.md:2: names ${old}, a spec of the old layout: openspec/specs/<area>/<name>/spec.md`,
  ])
})
