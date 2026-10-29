// Checks the `[@test]` links of the specs and the design docs: every link is a line of its own,
// `[@test] <path> (`name`, `name`)`, its path names a test file and every name a test in it: a TypeScript test by its
// title (a string literal), a Rust test by its function. A renamed or deleted test fails here, not in a reader.
// Also the budget of AGENTS.md, which every session of every agent reads whole (Codex reads no more than 32 KiB).
// Run: `pnpm lint:specs`.
import { existsSync, globSync, readFileSync, statSync } from "node:fs"
import { dirname, join, relative } from "node:path"

const root = join(import.meta.dirname, "..")
const LINK = /^\s*\[@test\] (\S+) \(((?:`[^`]+`(?:, )?)+)\)$/
const sources = new Map()
const problems = []

/** The text of a test file, read once */
function sourceOf(path) {
  if (!sources.has(path)) {
    sources.set(path, readFileSync(path, "utf8"))
  }
  return sources.get(path)
}

/** Whether the file declares the test: a Rust function, else a title in any string quote, escaped as the source writes it */
function declares(path, source, name) {
  if (path.endsWith(".rs")) {
    return new RegExp(`\\bfn ${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*[(<]`).test(source)
  }
  return ['"', "'", "`"].some(quote => source.includes(`${quote}${name.replaceAll(quote, `\\${quote}`)}${quote}`))
}

for (const doc of globSync(["spec/**/*.md", "docs/**/*.md"], { cwd: root })) {
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
    const path = join(root, dirname(doc), link[1])
    if (!existsSync(path)) {
      problems.push(`${at}: no file ${relative(root, path)}`)
      return
    }
    for (const [, name] of link[2].matchAll(/`([^`]+)`/g)) {
      if (!declares(path, sourceOf(path), name)) {
        problems.push(`${at}: ${relative(root, path)} has no test \`${name}\``)
      }
    }
  })
}

const AGENTS_BUDGET = 32 * 1024
const agentsBytes = statSync(join(root, "AGENTS.md")).size
if (agentsBytes > AGENTS_BUDGET) {
  problems.push(`AGENTS.md: ${agentsBytes} bytes, over its budget of ${AGENTS_BUDGET}: one line per rule there, the detail in the doc it links`)
}

if (problems.length > 0) {
  console.error(problems.join("\n"))
  console.error(`${problems.length} problem(s) of the [@test] links and AGENTS.md`)
  process.exit(1)
}
