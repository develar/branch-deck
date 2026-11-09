// Checks the links of the specs and the design docs to the code:
// - every `[@test]` link is a line of its own, `[@test] <path> (`name`, `name`)`, its path names a test file and every
//   name a test in it: a TypeScript test by its title (a string literal), a Rust test by its function. A path that
//   starts with `./` or `../` is from the file, any other from the root; under `openspec/` every path is from the root,
//   as `openspec archive` copies the text of a delta into the main spec two directories up;
// - every spec lists its `targets:` in its front matter, each once, a path of a file that exists: from the root for a
//   spec of `openspec/specs/<area>/<name>/spec.md`, `../../` for one of `spec/<area>/<name>.spec.md`;
// - every rule of the domain (`crates/domain-policy/src/<rule>.rs`) names its spec in its `//!` header, and every spec
//   it names lists it as a target;
// - every presenter of the page (`src/presenters/*.ts`) is a target of a spec;
// - a link into a spec of the OpenSpec format names one of its headings by the anchor GitHub gives it, or a group of
//   its requirements (`#refusals` for the requirements `### Requirement: Refusals: …`);
// - `openspec validate --strict` passes for every spec of the OpenSpec format and every change;
// - AGENTS.md keeps within its budget, which every session of every agent reads whole (Codex reads no more than 32 KiB).
// A renamed, moved or deleted test or file fails here, not in a reader. Every path is one from the root with `/` on
// every platform, as the links, the targets and the `//!` headers name a file (`globSync` answers `\` on Windows).
// Run: `pnpm lint:specs`.
import { execFileSync } from "node:child_process"
import { existsSync, globSync, readFileSync, statSync } from "node:fs"
import { join, posix } from "node:path"

const LINK = /^\s*\[@test\] (\S+) \(((?:`[^`]+`(?:, )?)+)\)$/
const RULES = "crates/domain-policy/src/*.rs"
const PRESENTERS = "src/presenters/*.ts"
const AGENTS_BUDGET = 32 * 1024
const SPEC_NAME = /\b(?:spec\/[a-z0-9-]+\/[a-z0-9-]+\.spec\.md|openspec\/specs\/(?:[a-z0-9-]+\/)+spec\.md)\b/g
/** The specs, in either format */
const SPECS = ["spec/**/*.spec.md", "openspec/specs/**/spec.md"]
/** The files of prose whose `[@test]` lines and links are checked; an archived change is history, as CHANGELOG.md */
const DOCS = ["spec/**/*.md", "openspec/**/*.md", "docs/**/*.md"]
const ARCHIVE = "openspec/changes/archive/"
/** A requirement of the OpenSpec format, the mark of a spec in it */
const REQUIREMENT = /^### Requirement: /m
/** A markdown link to a spec of OpenSpec, `[text](<path>/spec.md#anchor)` */
const SPEC_LINK = /\]\(([^)\s#]*\/spec\.md)(?:#([^)\s]*))?\)/g

/** The files under `root` that `patterns` match (`glob`: `globSync`), by their paths from `root` with `/`, sorted */
function filesOf(root, patterns, glob) {
  return glob(patterns, { cwd: root })
    .map(path => path.replaceAll("\\", "/"))
    .toSorted()
}

/** The files of prose under `root` whose links are checked, without the archived changes */
function docsOf(root, glob, patterns = DOCS) {
  return filesOf(root, patterns, glob).filter(doc => !doc.startsWith(ARCHIVE))
}

/** Whether every path the file at `path` names is from the root: a file under `openspec/` */
function underOpenSpec(path) {
  return path.startsWith("openspec/")
}

/** The file a link of `doc` names by `path`, from the root with `/`: `./` and `../` from `doc`, any other from the root */
function linkedFile(doc, path) {
  return path.startsWith("./") || path.startsWith("../") ? posix.join(posix.dirname(doc), path) : posix.normalize(path)
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
  for (const doc of docsOf(root, glob)) {
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
      if (underOpenSpec(doc) && link[1].startsWith(".")) {
        problems.push(`${at}: the path ${link[1]} is not from the root of the repository, as every path under openspec/`)
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
      problems.push(`${spec}:${index + 1}: not a target line, \`  - <path>\`: ${lines[index].trim()}`)
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

/** The problem of the path of a target of `spec`, or none: from the root under openspec/, `../../` under spec/ */
function targetPathProblem(spec, path) {
  if (underOpenSpec(spec)) {
    return path.startsWith(".") ? `the target ${path} is not from the root of the repository, as every path under openspec/` : null
  }
  return path.startsWith("../../") ? null : `the target ${path} is no \`../../\` path from the root of the repository`
}

/** How `spec` names `file` as its target */
function targetName(spec, file) {
  return underOpenSpec(spec) ? file : `../../${file}`
}

/**
 * The problems of the `targets:` of the specs under `root`: a spec without them, a target that is no path of a file
 * (from the root under openspec/, `../../` under spec/), one listed twice; a rule of `crates/domain-policy/src/` whose
 * `//!` header names no spec, or a spec that lists it not; a presenter of `src/presenters/` that no spec lists. `glob`
 * lists the files
 */
export function checkSpecTargets(root, glob = globSync) {
  const problems = []
  /** The specs that list a file, by its path from `root` */
  const listedBy = new Map()
  for (const spec of filesOf(root, SPECS, glob)) {
    const found = targetsOf(spec, readFileSync(join(root, spec), "utf8"))
    if (found.problem !== undefined) {
      problems.push(found.problem)
      continue
    }
    problems.push(...found.problems)
    const seen = new Set()
    for (const { path, line } of found.targets) {
      const at = `${spec}:${line}`
      const pathProblem = targetPathProblem(spec, path)
      if (pathProblem !== null) {
        problems.push(`${at}: ${pathProblem}`)
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
      problems.push(`${rule}: its \`//!\` header names no spec (openspec/specs/<area>/<name>/spec.md) of the rule`)
    }
    for (const spec of named) {
      if (!existsSync(join(root, spec))) {
        problems.push(`${rule}: its \`//!\` header names ${spec}, which does not exist`)
      } else if (!listedBy.get(rule)?.has(spec)) {
        problems.push(`${spec}: no target ${targetName(spec, rule)}, whose \`//!\` header names the spec`)
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

/**
 * The anchor GitHub gives a heading (github-slugger): lowercase, every character but a letter, a mark, a number, `_`,
 * `-` and a space dropped, every space a `-`
 */
export function headingSlug(heading) {
  return heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc}\- ]/gu, "")
    .replaceAll(" ", "-")
}

/** The anchors of the headings of `text` outside its fenced code, a repeated one with the `-1`, `-2` GitHub gives it */
export function headingAnchors(text) {
  const anchors = new Set()
  const counts = new Map()
  let fence = null
  for (const line of text.split(/\r?\n/)) {
    const marker = /^\s*(`{3,}|~{3,})/.exec(line)?.[1]
    if (marker !== undefined) {
      if (fence === null) {
        fence = marker
      } else if (marker[0] === fence[0] && marker.length >= fence.length) {
        fence = null
      }
      continue
    }
    const heading = fence === null ? /^#{1,6} (.+)$/.exec(line) : null
    if (heading === null) {
      continue
    }
    const slug = headingSlug(heading[1].replace(/\s+#+\s*$/, ""))
    const count = counts.get(slug) ?? 0
    counts.set(slug, count + 1)
    anchors.add(count === 0 ? slug : `${slug}-${count}`)
  }
  return anchors
}

/** Whether `anchor` names one of `anchors` or a group of requirements: `refusals` for `requirement-refusals-…` */
export function namesHeading(anchors, anchor) {
  return anchors.has(anchor) || [...anchors].some(slug => slug.startsWith(`requirement-${anchor}-`))
}

/**
 * The problems of the links of the files of prose under `root` (the specs, `docs/`, AGENTS.md, the READMEs) into a spec
 * of OpenSpec: a file that is gone; for a spec of its format, an anchor that names none of its headings and no group of
 * its requirements. `glob` lists the files
 */
export function checkSpecAnchors(root, glob = globSync) {
  const anchorsOf = new Map()
  const problems = []
  for (const doc of docsOf(root, glob, [...DOCS, "AGENTS.md", "README.md", "crates/*/README.md", "crates/*/cases/README.md"])) {
    readFileSync(join(root, doc), "utf8")
      .split(/\r?\n/)
      .forEach((line, index) => {
        for (const [, path, anchor] of line.matchAll(SPEC_LINK)) {
          const file = path.startsWith("/") ? path.slice(1) : linkedFile(doc, path.startsWith(".") ? path : `./${path}`)
          if (!file.startsWith("openspec/specs/")) {
            continue
          }
          const at = `${doc}:${index + 1}`
          if (!existsSync(join(root, file))) {
            problems.push(`${at}: no file ${file}`)
            continue
          }
          if (!anchorsOf.has(file)) {
            const text = readFileSync(join(root, file), "utf8")
            anchorsOf.set(file, REQUIREMENT.test(text) ? headingAnchors(text) : null)
          }
          const anchors = anchorsOf.get(file)
          if (anchor !== undefined && anchors !== null && !namesHeading(anchors, anchor)) {
            problems.push(`${at}: ${file} has no heading of the anchor #${anchor}`)
          }
        }
      })
  }
  return problems
}

/** The problem of an AGENTS.md under `root` over its budget, or none */
export function checkAgentsBudget(root) {
  const bytes = statSync(join(root, "AGENTS.md")).size
  return bytes > AGENTS_BUDGET ? [`AGENTS.md: ${bytes} bytes, over its budget of ${AGENTS_BUDGET}: one line per rule there, the detail in the doc it links`] : []
}

/** The report of `openspec validate <args> --strict --json` in `root`, by the binary of the devDependency of this repository */
export function openspecValidate(root, args) {
  const windows = process.platform === "win32"
  const binary = join(import.meta.dirname, "..", "node_modules", ".bin", windows ? "openspec.cmd" : "openspec")
  try {
    return JSON.parse(
      execFileSync(binary, ["validate", ...args, "--strict", "--json", "--no-interactive"], {
        cwd: root,
        encoding: "utf8",
        env: { ...process.env, OPENSPEC_TELEMETRY: "0", OPENSPEC_NO_UPDATE_CHECK: "1" },
        shell: windows,
        stdio: ["ignore", "pipe", "pipe"],
      }),
    )
  } catch (error) {
    // an invalid item exits 1 with its report on stdout
    if (typeof error.stdout !== "string" || error.stdout.trim() === "") {
      throw error
    }
    return JSON.parse(error.stdout)
  }
}

/**
 * The problems `openspec validate --strict` reports of the specs under `openspec/specs/` of its format (a spec still in
 * the old one aside while the specs move), of the changes under `openspec/changes/` and of the archived ones (a task not
 * done). `validate` answers the report of its arguments
 */
export function checkOpenSpec(root, glob = globSync, validate = openspecValidate) {
  if (!existsSync(join(root, "openspec"))) {
    return []
  }
  const ofFormat = new Set(
    filesOf(root, "openspec/specs/**/spec.md", glob)
      .filter(spec => REQUIREMENT.test(readFileSync(join(root, spec), "utf8")))
      .map(spec => spec.slice("openspec/specs/".length, -"/spec.md".length)),
  )
  const problems = []
  const report = (answer, keep) => {
    for (const item of (answer.items ?? []).filter(keep)) {
      const issues = item.issues ?? []
      for (const issue of issues) {
        const line = issue.line === undefined ? "" : `:${issue.line}`
        problems.push(`openspec ${item.type} ${item.id}: ${issue.level} ${issue.path}${line}: ${issue.message.split("\n")[0]}`)
      }
      if (!item.valid && issues.length === 0) {
        problems.push(`openspec ${item.type} ${item.id}: invalid`)
      }
    }
  }
  if (ofFormat.size > 0) {
    report(validate(root, ["--specs"]), item => ofFormat.has(item.id))
  }
  if (filesOf(root, "openspec/changes/*/proposal.md", glob).length > 0) {
    report(validate(root, ["--changes"]), () => true)
  }
  if (filesOf(root, `${ARCHIVE}*/proposal.md`, glob).length > 0) {
    report(validate(root, ["--archived"]), () => true)
  }
  return problems
}

if (import.meta.main) {
  const root = join(import.meta.dirname, "..")
  const problems = [...checkTestLinks(root), ...checkSpecTargets(root), ...checkSpecAnchors(root), ...checkOpenSpec(root), ...checkAgentsBudget(root)]
  if (problems.length > 0) {
    console.error(problems.join("\n"))
    console.error(
      `${problems.length} broken link(s) of the specs: a [@test] link names a test that exists, a target a file, a rule its spec, a presenter is a target, a link into a spec its heading, openspec validate --strict passes, AGENTS.md within its budget (scripts/check-spec-links.mjs)`,
    )
    process.exit(1)
  }
}
