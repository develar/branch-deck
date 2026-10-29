// Checks the links of the specs and the design docs to the code:
// - every `[@test]` link is a line of its own, `[@test] <path> (`name`, `name`)`, its path names a test file and every
//   name a test in it: a TypeScript test by its title (a string literal), a Rust test by its function;
// - every spec lists its `targets:` in its front matter, each once, a `../../` path of a file that exists;
// - every rule of the domain (`crates/domain-policy/src/<rule>.rs`) names its spec in its `//!` header, and every spec
//   it names lists it as a target;
// - every presenter of the page (`src/presenters/*.ts`) is a target of a spec;
// - AGENTS.md keeps within its budget, which every session of every agent reads whole (Codex reads no more than 32 KiB).
// A renamed, moved or deleted test or file fails here, not in a reader. Every path is one from the root with `/` on
// every platform, as the links, the targets and the `//!` headers name a file (`globSync` answers `\` on Windows).
// Run: `pnpm lint:specs`.
import { existsSync, globSync, readFileSync, statSync } from "node:fs"
import { join, posix } from "node:path"

const LINK = /^\s*\[@test\] (\S+) \(((?:`[^`]+`(?:, )?)+)\)$/
const RULES = "crates/domain-policy/src/*.rs"
const PRESENTERS = "src/presenters/*.ts"
const AGENTS_BUDGET = 32 * 1024
const SPEC_NAME = /\bspec\/[a-z0-9-]+\/[a-z0-9-]+\.spec\.md\b/g

/** The files under `root` that `patterns` match (`glob`: `globSync`), by their paths from `root` with `/`, sorted */
function filesOf(root, patterns, glob) {
  return glob(patterns, { cwd: root })
    .map(path => path.replaceAll("\\", "/"))
    .toSorted()
}

/** The file a link of `doc` names by `path` (`../../src/x.ts`), from the root with `/` */
function linkedFile(doc, path) {
  return posix.join(posix.dirname(doc), path)
}

/** Whether the file declares the test: a Rust function, else a title in any string quote, escaped as the source writes it */
function declares(path, source, name) {
  if (path.endsWith(".rs")) {
    return new RegExp(`\\bfn ${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*[(<]`).test(source)
  }
  return ['"', "'", "`"].some(quote => source.includes(`${quote}${name.replaceAll(quote, `\\${quote}`)}${quote}`))
}

/** The problems of the `[@test]` links of the specs and the design docs under `root`; `glob` lists their files */
export function checkTestLinks(root, glob = globSync) {
  const sources = new Map()
  const sourceOf = path => {
    if (!sources.has(path)) {
      sources.set(path, readFileSync(path, "utf8"))
    }
    return sources.get(path)
  }
  const problems = []
  for (const doc of filesOf(root, ["spec/**/*.md", "docs/**/*.md"], glob)) {
    const lines = readFileSync(join(root, doc), "utf8").split(/\r?\n/)
    lines.forEach((line, index) => {
      if (!line.includes("[@test]")) {
        return
      }
      const at = `${doc}:${index + 1}`
      const link = LINK.exec(line)
      if (link === null) {
        problems.push(`${at}: not a link of its own line, \`[@test] <path> (\`name\`, …)\`: ${line.trim().slice(0, 120)}`)
        return
      }
      const file = linkedFile(doc, link[1])
      const path = join(root, file)
      if (!existsSync(path)) {
        problems.push(`${at}: no file ${file}`)
        return
      }
      for (const [, name] of link[2].matchAll(/`([^`]+)`/g)) {
        if (!declares(path, sourceOf(path), name)) {
          problems.push(`${at}: ${file} has no test \`${name}\``)
        }
      }
    })
  }
  return problems
}

/** The `targets:` of a spec's front matter, each with its line, or the problem of a spec that has none */
function targetsOf(spec, text) {
  const lines = text.split(/\r?\n/)
  const end = lines[0] === "---" ? lines.indexOf("---", 1) : -1
  if (end < 0) {
    return { problem: `${spec}: no front matter (\`---\` … \`---\`)` }
  }
  const start = lines.findIndex((line, index) => index < end && line === "targets:")
  if (start < 0) {
    return { problem: `${spec}: its front matter lists no \`targets:\`` }
  }
  const targets = []
  const problems = []
  for (let index = start + 1; index < end && /^\s/.test(lines[index]); index++) {
    const item = /^\s+- (\S+)$/.exec(lines[index])
    if (item === null) {
      problems.push(`${spec}:${index + 1}: not a target line, \`  - ../../<path>\`: ${lines[index].trim()}`)
    } else {
      targets.push({ path: item[1], line: index + 1 })
    }
  }
  if (targets.length === 0 && problems.length === 0) {
    problems.push(`${spec}: its \`targets:\` list no file`)
  }
  return { targets, problems }
}

/** The leading `//!` lines of a Rust file */
function moduleHeader(source) {
  const header = []
  for (const line of source.split(/\r?\n/)) {
    if (!line.startsWith("//!")) {
      break
    }
    header.push(line)
  }
  return header.join("\n")
}

/**
 * The problems of the `targets:` of the specs under `root`: a spec without them, a target that is no `../../` path of
 * a file, one listed twice; a rule of `crates/domain-policy/src/` whose `//!` header names no spec, or a spec that
 * lists it not; a presenter of `src/presenters/` that no spec lists. `glob` lists the files
 */
export function checkSpecTargets(root, glob = globSync) {
  const problems = []
  /** The specs that list a file, by its path from `root` */
  const listedBy = new Map()
  for (const spec of filesOf(root, "spec/**/*.spec.md", glob)) {
    const found = targetsOf(spec, readFileSync(join(root, spec), "utf8"))
    if (found.problem !== undefined) {
      problems.push(found.problem)
      continue
    }
    problems.push(...found.problems)
    const seen = new Set()
    for (const { path, line } of found.targets) {
      const at = `${spec}:${line}`
      if (!path.startsWith("../../")) {
        problems.push(`${at}: the target ${path} is no \`../../\` path from the root of the repository`)
        continue
      }
      const file = linkedFile(spec, path)
      if (seen.has(file)) {
        problems.push(`${at}: lists ${path} twice`)
        continue
      }
      seen.add(file)
      if (!existsSync(join(root, file))) {
        problems.push(`${at}: no file ${file}`)
        continue
      }
      if (!statSync(join(root, file)).isFile()) {
        problems.push(`${at}: ${file} is no file`)
        continue
      }
      if (!listedBy.has(file)) {
        listedBy.set(file, new Set())
      }
      listedBy.get(file).add(spec)
    }
  }
  for (const rule of filesOf(root, RULES, glob)) {
    if (rule.endsWith("/lib.rs") || rule.endsWith("_test.rs")) {
      continue
    }
    const named = new Set(moduleHeader(readFileSync(join(root, rule), "utf8")).match(SPEC_NAME) ?? [])
    if (named.size === 0) {
      problems.push(`${rule}: its \`//!\` header names no spec (spec/<area>/<name>.spec.md) of the rule`)
    }
    for (const spec of named) {
      if (!existsSync(join(root, spec))) {
        problems.push(`${rule}: its \`//!\` header names ${spec}, which does not exist`)
      } else if (!listedBy.get(rule)?.has(spec)) {
        problems.push(`${spec}: no target ../../${rule}, whose \`//!\` header names the spec`)
      }
    }
  }
  for (const presenter of filesOf(root, PRESENTERS, glob)) {
    if (!presenter.endsWith(".test.ts") && !listedBy.has(presenter)) {
      problems.push(`${presenter}: a presenter that the \`targets:\` of no spec list`)
    }
  }
  return problems
}

/** The problem of an AGENTS.md under `root` over its budget, or none */
export function checkAgentsBudget(root) {
  const bytes = statSync(join(root, "AGENTS.md")).size
  return bytes > AGENTS_BUDGET ? [`AGENTS.md: ${bytes} bytes, over its budget of ${AGENTS_BUDGET}: one line per rule there, the detail in the doc it links`] : []
}

if (import.meta.main) {
  const root = join(import.meta.dirname, "..")
  const problems = [...checkTestLinks(root), ...checkSpecTargets(root), ...checkAgentsBudget(root)]
  if (problems.length > 0) {
    console.error(problems.join("\n"))
    console.error(
      `${problems.length} broken link(s) of the specs: a [@test] link names a test that exists, a target a file, a rule its spec, a presenter is a target, AGENTS.md within its budget (scripts/check-spec-links.mjs)`,
    )
    process.exit(1)
  }
}
