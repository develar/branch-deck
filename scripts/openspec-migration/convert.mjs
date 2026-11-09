// The conversion of a spec to the OpenSpec format (docs/contributing/specs.md), one spec at a time:
//   node scripts/openspec-migration/convert.mjs extract <spec> [--rev <rev>]
//     the old spec as JSON: its front matter, title, status, sections, and its Requirements by id (`G<g>.B<b>` a bullet,
//     `G<g>.P<p>` a paragraph of prose of a group, `X<x>` a block of test links outside Requirements);
//   node scripts/openspec-migration/convert.mjs verify <spec> [--old <rev>] [--map <map.json>]
//     the problems of the converted spec against the old one at <rev> (HEAD by default): nothing of its front matter,
//     its other sections and its test links lost, the structure and limits of `openspec validate --strict`, the groups
//     in their order, every code span, quoted string, link and measure of the old rules kept, every old id mapped;
//   node scripts/openspec-migration/convert.mjs assemble <spec> --parts <dir> [--rev <rev>]
//     writes the converted spec from the old one at <rev> and the parts of <dir>: `## Summary` becomes `## Purpose`
//     (`purpose.md` when it had none), `concepts.md` becomes `## Concepts` before Requirements, the parts `<nn>*.md`
//     in the order of their names become the body of `## Requirements`, a section of requirements outside it
//     (`## Code owners`) goes, and every other line stays as it is.
// <spec> is `openspec/specs/<area>/<name>/spec.md`. Deleted once every spec is converted.
import { execFileSync } from "node:child_process"
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs"
import { join, posix } from "node:path"
import { headingSlug, openspecValidate } from "../check-spec-links.mjs"

const root = join(import.meta.dirname, "..", "..")
const TEST = /^\s*\[@test\] (\S+) \(((?:`[^`]+`(?:, )?)+)\)\s*$/

/** The text of `path` at `rev`, or at its old place `spec/<area>/<name>.spec.md` before the move */
function textAt(rev, path) {
  const old = path.replace(/^openspec\/specs\/([^/]+)\/([^/]+)\/spec\.md$/, "spec/$1/$2.spec.md")
  for (const candidate of [path, old]) {
    try {
      return {
        text: execFileSync("git", ["show", `${rev}:${candidate}`], { cwd: root, encoding: "utf8", maxBuffer: 1 << 26, stdio: ["ignore", "pipe", "ignore"] }),
        path: candidate,
      }
    } catch {
      // not there at this revision
    }
  }
  throw new Error(`${path} is not at ${rev}`)
}

/** The test pairs `path name` of a test line, the path from the root (`../../x` of the old place of a spec is `x`) */
function testPairs(line, from) {
  const link = TEST.exec(line)
  if (link === null) {
    return null
  }
  const path = link[1].startsWith("../") || link[1].startsWith("./") ? posix.normalize(posix.join(posix.dirname(from), link[1])) : link[1]
  return [...link[2].matchAll(/`([^`]+)`/g)].map(([, name]) => `${path} ${name}`)
}

/** The lines of `text` split into front matter, title, status lines and `##` sections, fenced code kept whole */
function parse(text) {
  const lines = text.replace(/\r\n/g, "\n").split("\n")
  const end = lines[0] === "---" ? lines.indexOf("---", 1) : -1
  const front = end > 0 ? lines.slice(1, end) : []
  const sections = []
  const head = []
  let fence = false
  for (const line of lines.slice(end + 1)) {
    if (/^\s*(```|~~~)/.test(line)) {
      fence = !fence
    }
    if (!fence && line.startsWith("## ")) {
      sections.push({ title: line.slice(3).trim(), lines: [] })
    } else if (sections.length === 0) {
      head.push(line)
    } else {
      sections.at(-1).lines.push(line)
    }
  }
  return { front, head, sections }
}

/** The front matter as `key → value` (the list of `targets:` as its items, from the root) */
function frontOf(front, from) {
  const fields = new Map()
  let key = null
  for (const line of front) {
    const field = /^([a-z]+):\s?(.*)$/.exec(line)
    if (field !== null) {
      key = field[1]
      fields.set(key, field[2] === "" ? [] : field[2])
    } else if (key !== null && Array.isArray(fields.get(key))) {
      const item = /^\s+- (\S+)$/.exec(line)
      if (item !== null) {
        const path = item[1].startsWith("../") ? posix.normalize(posix.join(posix.dirname(from), item[1])) : item[1]
        fields.get(key).push(path)
      }
    }
  }
  return fields
}

/** The groups of the old Requirements (or of a section of requirements, `## Code owners`): bullets and prose by id */
function oldGroups(section, from, groupBase = 0) {
  const groups = []
  let item = null
  let fence = false
  const close = () => {
    item = null
  }
  for (const line of section.lines) {
    if (/^\s*(```|~~~)/.test(line)) {
      fence = !fence
    }
    if (!fence && line.startsWith("### ")) {
      groups.push({ id: `G${groupBase + groups.length + 1}`, title: line.slice(4).trim(), items: [] })
      close()
      continue
    }
    if (groups.length === 0) {
      if (line.trim() === "") {
        continue
      }
      groups.push({ id: `G${groupBase + 1}`, title: "", items: [] })
    }
    const group = groups.at(-1)
    if (!fence && line.startsWith("- ")) {
      item = { id: `${group.id}.B${group.items.filter(it => it.kind === "bullet").length + 1}`, kind: "bullet", lines: [line], tests: [] }
      group.items.push(item)
      continue
    }
    if (line.trim() === "" && !fence) {
      if (item?.kind === "prose") {
        close()
      }
      continue
    }
    const pairs = fence ? null : testPairs(line, from)
    if (item === null) {
      item = { id: `${group.id}.P${group.items.filter(it => it.kind === "prose").length + 1}`, kind: "prose", lines: [], tests: [] }
      group.items.push(item)
    }
    item.lines.push(line)
    if (pairs !== null) {
      item.tests.push(...pairs)
    }
  }
  return groups
}

/** The old spec at `rev` as the converter reads it */
export function extract(path, rev = "HEAD") {
  const at = textAt(rev, path)
  const { front, head, sections } = parse(at.text)
  const groups = []
  const outside = []
  for (const section of sections) {
    if (section.title.toLowerCase() === "requirements") {
      groups.push(...oldGroups(section, at.path, groups.length))
    } else if (section.lines.some(line => line.startsWith("### ")) && section.lines.some(line => TEST.test(line))) {
      // a section of requirements outside Requirements (`## Code owners`): its groups fold into Requirements
      groups.push(...oldGroups(section, at.path, groups.length).map(group => ({ ...group, title: group.title === "" ? section.title : group.title, section: section.title })))
    } else {
      section.lines.forEach((line, index) => {
        const pairs = testPairs(line, at.path)
        if (pairs !== null) {
          outside.push({ id: `X${outside.length + 1}`, section: section.title, line: index, tests: pairs })
        }
      })
    }
  }
  return { path, from: at.path, front: Object.fromEntries(frontOf(front, at.path)), head, sections: sections.map(section => section.title), groups, outside }
}

/** The requirements of a converted spec: name, group, text lines, scenarios with their steps and test pairs */
function newRequirements(section, problems) {
  const requirements = []
  let fence = false
  for (const line of section.lines) {
    if (/^\s*(```|~~~)/.test(line)) {
      fence = !fence
    }
    const requirement = fence ? null : /^### Requirement: (.+)$/.exec(line)
    const scenario = fence ? null : /^#### Scenario: (.+)$/.exec(line)
    if (requirement !== null) {
      requirements.push({ name: requirement[1].trim(), text: [], scenarios: [], line })
    } else if (scenario !== null) {
      if (requirements.length === 0) {
        problems.push(`a scenario before the first requirement: ${line}`)
        continue
      }
      requirements.at(-1).scenarios.push({ name: scenario[1].trim(), lines: [] })
    } else if (!fence && /^#{1,6} /.test(line)) {
      problems.push(`a heading of Requirements that is no requirement and no scenario: ${line}`)
    } else if (requirements.length === 0) {
      if (line.trim() !== "") {
        problems.push(`text before the first requirement: ${line.slice(0, 80)}`)
      }
    } else {
      const last = requirements.at(-1)
      ;(last.scenarios.length === 0 ? last.text : last.scenarios.at(-1).lines).push(line)
    }
  }
  return requirements
}

/** The code spans, quoted strings, link targets and measures of `text`, each with its count */
function tokensOf(text) {
  const counts = new Map()
  const add = token => counts.set(token, (counts.get(token) ?? 0) + 1)
  // a line wrapped anew changes no token: the lines of the text are joined first
  const plain = text.replace(/\s*\n\s*/g, " ").replace(/`[^`]+`/g, span => (add(span), " "))
  for (const [match] of plain.matchAll(/"[^"\n]{1,120}"|“[^”\n]{1,120}”|‹[^›\n]{1,60}›/g)) {
    add(match)
  }
  for (const [, target] of plain.matchAll(/\]\(([^)\s]+)\)/g)) {
    add(`(${target})`)
  }
  for (const [match] of plain.matchAll(
    /\b\d+(?:[.,]\d+)?\s?(?:ms|s|min|h|KB|KiB|MB|MiB|GB|GiB|%|px|days?|hours?|minutes?|seconds?|commits?|lines?|characters?|processes|process)\b/g,
  )) {
    add(match.replace(/\s+/, " "))
  }
  return counts
}

/** Words of a name or text folded for comparison */
const fold = text => text.toLowerCase().replace(/\s+/g, " ").trim()

/** The problems of the converted spec at `path` against its old text at `rev` */
export function verify(path, rev = "HEAD", mapPath = null) {
  const problems = []
  const old = extract(path, rev)
  const oldAt = textAt(rev, path)
  const text = readFileSync(join(root, path), "utf8")
  const now = parse(text)
  const front = Object.fromEntries(frontOf(now.front, path))
  for (const key of new Set([...Object.keys(old.front), ...Object.keys(front)])) {
    if (JSON.stringify(old.front[key]) !== JSON.stringify(front[key])) {
      problems.push(`front matter \`${key}\` changed: ${JSON.stringify(old.front[key])?.slice(0, 120)} → ${JSON.stringify(front[key])?.slice(0, 120)}`)
    }
  }
  if (now.head.join("\n").trim() !== old.head.join("\n").trim()) {
    problems.push("the title or the Status and Date lines changed")
  }
  const oldSections = parse(oldAt.text).sections
  const section = title => now.sections.find(it => it.title.toLowerCase() === title.toLowerCase())
  const purpose = section("Purpose")
  const summary = oldSections.find(it => it.title === "Summary")
  if (purpose === undefined) {
    problems.push("no `## Purpose`")
  } else if (summary !== undefined && fold(purpose.lines.join(" ")) !== fold(summary.lines.join(" "))) {
    problems.push("`## Purpose` is not the old Summary")
  }
  const foldedInto = new Set(old.groups.filter(group => group.section !== undefined).map(group => group.section))
  const withoutTests = lines => fold(lines.filter(line => !TEST.test(line)).join("\n"))
  for (const oldSection of oldSections) {
    if (["summary", "requirements"].includes(oldSection.title.toLowerCase()) || foldedInto.has(oldSection.title)) {
      continue
    }
    const kept = section(oldSection.title)
    if (kept === undefined) {
      problems.push(`the section \`## ${oldSection.title}\` is gone`)
    } else if (withoutTests(kept.lines) !== withoutTests(oldSection.lines)) {
      problems.push(`the section \`## ${oldSection.title}\` changed`)
    }
  }
  for (const extra of now.sections) {
    if (!["purpose", "requirements", "concepts"].includes(extra.title.toLowerCase()) && !oldSections.some(it => it.title === extra.title)) {
      problems.push(`a new section \`## ${extra.title}\``)
    }
  }
  const order = now.sections.map(it => it.title.toLowerCase())
  if (order.indexOf("purpose") > order.indexOf("requirements")) {
    problems.push("`## Purpose` must come before `## Requirements`")
  }
  const requirementsSection = section("Requirements")
  if (requirementsSection === undefined) {
    problems.push("no `## Requirements`")
    return problems
  }
  const requirements = newRequirements(requirementsSection, problems)
  // names and anchors
  const names = new Map()
  for (const requirement of requirements) {
    const slug = headingSlug(`Requirement: ${requirement.name}`)
    if (names.has(slug)) {
      problems.push(`two requirements of the anchor #${slug}: ${requirement.name}`)
    }
    names.set(slug, requirement)
    const scenarioNames = new Set()
    for (const scenario of requirement.scenarios) {
      if (scenarioNames.has(fold(scenario.name))) {
        problems.push(`${requirement.name}: two scenarios "${scenario.name}"`)
      }
      scenarioNames.add(fold(scenario.name))
    }
  }
  // groups: every old group of rules, in order, contiguous; a requirement of no old group is a problem
  const groupTitles = old.groups.map(group => group.title)
  const groupOf = name =>
    groupTitles.filter(title => title !== "" && name.startsWith(`${title}: `)).sort((a, b) => b.length - a.length)[0] ?? (groupTitles.includes("") ? "" : undefined)
  const sequence = []
  for (const requirement of requirements) {
    const group = groupOf(requirement.name)
    if (group === undefined) {
      problems.push(`the requirement "${requirement.name}" names no old group as its prefix (\`<Group>: <behaviour>\`)`)
      continue
    }
    if (sequence.at(-1) !== group) {
      if (sequence.includes(group)) {
        problems.push(`the requirements of the group "${group}" are not together: "${requirement.name}"`)
      }
      sequence.push(group)
    }
  }
  const expected = groupTitles.filter(title => old.groups.find(group => group.title === title).items.some(item => item.kind === "bullet"))
  const seen = sequence.filter(group => expected.includes(group))
  if (JSON.stringify(seen) !== JSON.stringify(expected)) {
    problems.push(`the groups are not the old ones in their order: ${JSON.stringify(seen)} ≠ ${JSON.stringify(expected)}`)
  }
  // texts and scenarios
  for (const requirement of requirements) {
    const body = []
    let fence = false
    for (const line of requirement.text) {
      if (/^\s*(```|~~~)/.test(line)) {
        fence = !fence
        continue
      }
      if (!fence && line.trim() !== "" && !/^\*\*[^*]+\*\*:/.test(line.trim())) {
        body.push(line.trim())
      }
    }
    const joined = body.join("\n")
    if (joined.length > 500) {
      problems.push(`${requirement.name}: its text has ${joined.length} characters, over 500`)
    }
    if (!/\b(SHALL|MUST)\b/.test(joined)) {
      problems.push(`${requirement.name}: its text has no SHALL or MUST`)
    }
    if (requirement.text.some(line => TEST.test(line) || line.includes("[@test]"))) {
      problems.push(`${requirement.name}: a test link in its text, not in a scenario`)
    }
    if (body.some(line => line.startsWith("|"))) {
      problems.push(`${requirement.name}: a table in its text, not in a scenario`)
    }
    if (requirement.scenarios.length === 0) {
      problems.push(`${requirement.name}: no scenario`)
    }
    for (const scenario of requirement.scenarios) {
      const steps = scenario.lines.join("\n")
      if (!/^- \*\*(WHEN|GIVEN)\*\*/m.test(steps) || !/^- \*\*THEN\*\*/m.test(steps)) {
        problems.push(`${requirement.name} / ${scenario.name}: no \`- **WHEN**\` (or GIVEN) and \`- **THEN**\` step`)
      }
    }
  }
  // test pairs: the same set, no pair cited fewer times
  const count = pairs => pairs.reduce((map, pair) => map.set(pair, (map.get(pair) ?? 0) + 1), new Map())
  const oldPairs = count([...old.groups.flatMap(group => group.items.flatMap(item => item.tests)), ...old.outside.flatMap(block => block.tests)])
  const nowPairs = count(text.split("\n").flatMap(line => testPairs(line, path) ?? []))
  for (const [pair, times] of oldPairs) {
    if (!nowPairs.has(pair)) {
      problems.push(`the test link ${pair} is gone`)
    } else if (nowPairs.get(pair) < times) {
      problems.push(`the test link ${pair} is cited ${nowPairs.get(pair)} times, ${times} before`)
    }
  }
  for (const pair of nowPairs.keys()) {
    if (!oldPairs.has(pair)) {
      problems.push(`a new test link ${pair}`)
    }
  }
  // tokens of the old rules, kept at least as often
  const map = mapPath === null ? null : JSON.parse(readFileSync(mapPath, "utf8"))
  const dropped = new Set((map?.dropped ?? []).map(entry => entry.token))
  const oldRules = old.groups.flatMap(group => group.items.flatMap(item => item.lines.filter(line => !TEST.test(line)))).join("\n")
  const newRules = [...requirementsSection.lines, ...(section("Concepts")?.lines ?? [])].filter(line => !TEST.test(line)).join("\n")
  const nowTokens = tokensOf(newRules)
  for (const [token, times] of tokensOf(oldRules)) {
    if ((nowTokens.get(token) ?? 0) < times && !dropped.has(token)) {
      problems.push(`the token ${token} appears ${nowTokens.get(token) ?? 0} times, ${times} before`)
    }
  }
  // the map: every old id mapped, every requirement from one
  if (map !== null) {
    const ids = old.groups.flatMap(group => group.items.map(item => item.id)).concat(old.outside.map(block => block.id))
    for (const id of ids) {
      if (!(id in (map.ids ?? {}))) {
        problems.push(`the map has no entry of ${id}`)
      }
    }
    const mapped = new Set(Object.values(map.ids ?? {}).flat())
    for (const requirement of requirements) {
      if (!mapped.has(requirement.name)) {
        problems.push(`the requirement "${requirement.name}" comes from no old id of the map`)
      }
    }
    for (const name of mapped) {
      if (!requirements.some(requirement => requirement.name === name) && name !== "Concepts") {
        problems.push(`the map names "${name}", which is no requirement`)
      }
    }
  }
  // openspec itself
  const id = path.slice("openspec/specs/".length, -"/spec.md".length)
  const report = openspecValidate(root, ["--specs"])
  for (const item of (report.items ?? []).filter(item => item.id === id)) {
    for (const issue of item.issues ?? []) {
      problems.push(`openspec ${issue.level} ${issue.path}: ${issue.message.split("\n")[0]}`)
    }
  }
  return problems
}

/** The converted spec of `path` from its old text at `rev` and the parts of `dir` (see the head of this file) */
export function assemble(path, dir, rev = "HEAD") {
  const at = textAt(rev, path)
  const part = name => (existsSync(join(dir, name)) ? readFileSync(join(dir, name), "utf8").trim() : null)
  const parts = readdirSync(dir)
    .filter(name => /^\d+.*\.md$/.test(name))
    .sort()
    .map(name => readFileSync(join(dir, name), "utf8").trim())
  if (parts.length === 0) {
    throw new Error(`no parts <nn>*.md in ${dir}`)
  }
  const folded = new Set(
    extract(path, rev)
      .groups.filter(group => group.section !== undefined)
      .map(group => group.section),
  )
  const out = []
  let fence = false
  let skip = false
  let purpose = false
  for (const line of at.text.split("\n")) {
    if (/^\s*(```|~~~)/.test(line)) {
      fence = !fence
    }
    const title = !fence && line.startsWith("## ") ? line.slice(3).trim() : null
    if (title === null) {
      if (!skip) {
        out.push(line)
      }
      continue
    }
    skip = false
    if (title === "Summary") {
      out.push("## Purpose")
      purpose = true
    } else if (title.toLowerCase() === "requirements") {
      if (!purpose && part("purpose.md") !== null) {
        out.push("## Purpose", "", part("purpose.md"), "")
        purpose = true
      }
      if (part("concepts.md") !== null) {
        out.push("## Concepts", "", part("concepts.md"), "")
      }
      out.push("## Requirements", "", parts.join("\n\n"), "")
      skip = true
    } else if (folded.has(title)) {
      skip = true
    } else {
      out.push(line)
    }
  }
  writeFileSync(join(root, path), out.join("\n"))
}

if (import.meta.main) {
  const [command, path, ...rest] = process.argv.slice(2)
  const option = name => {
    const at = rest.indexOf(name)
    return at < 0 ? null : rest[at + 1]
  }
  if (command === "extract") {
    console.log(JSON.stringify(extract(path, option("--rev") ?? "HEAD"), null, 1))
  } else if (command === "verify") {
    const problems = verify(path, option("--old") ?? "HEAD", option("--map"))
    if (problems.length > 0) {
      console.error(problems.join("\n"))
      console.error(`${problems.length} problem(s) of ${path}`)
      process.exit(1)
    }
    console.log(`${path}: converted, nothing lost`)
  } else if (command === "assemble") {
    assemble(path, option("--parts"), option("--rev") ?? "HEAD")
    console.log(`${path}: assembled`)
  } else {
    console.error("usage: convert.mjs extract|verify|assemble <openspec/specs/<area>/<name>/spec.md> [--rev|--old <rev>] [--map <map.json>]")
    process.exit(2)
  }
}
