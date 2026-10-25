// Copies the Islands theme variables of the IntelliJ monorepo into src/islands.generated.css
// (docs/design/ui-theme.md, "Palette"). The monorepo writes the file from ManyIslandsLight/Dark.theme.json; src/index.css
// maps its `--islands-*` variables to the tokens. The copy is committed, so a build needs no monorepo.
// The header of the copy names the repository and the commit the file came from.
//
// Run, from a local checkout of the monorepo (the file at a commit, never the working tree):
//   node scripts/sync-islands-theme.mjs --monorepo ~/projects/idea-1 [--ref master]
//   INTELLIJ_MONOREPO=~/projects/idea-1 node scripts/sync-islands-theme.mjs
// or from intellij-community on GitHub at a pinned commit:
//   node scripts/sync-islands-theme.mjs --github <commit>
import { execFileSync } from "node:child_process"
import { writeFileSync } from "node:fs"
import { join } from "node:path"
import { parseArgs } from "node:util"

/** The path of the file in the monorepo; intellij-community is its `community/` directory */
const MONOREPO_PATH = "community/tools/docs-site/styles/islands.generated.css"
const COMMUNITY_PATH = MONOREPO_PATH.slice("community/".length)
const TARGET = join(import.meta.dirname, "../src/islands.generated.css")

/** `{ text, source }` of the file at `ref` of a local monorepo checkout; `source` names the last commit that changed it */
function fromMonorepo(checkout, ref) {
  const git = (...args) => execFileSync("git", ["-C", checkout, ...args], { encoding: "utf8" })
  const commit = git("log", "-1", "--format=%H", ref, "--", MONOREPO_PATH).trim()
  if (commit === "") {
    throw new Error(`${checkout} has no ${MONOREPO_PATH} at ${ref}`)
  }
  return { text: git("show", `${commit}:${MONOREPO_PATH}`), source: `ultimate ${commit}, ${MONOREPO_PATH}` }
}

/** `{ text, source }` of the file at `commit` of intellij-community on GitHub */
async function fromGitHub(commit) {
  if (!/^[0-9a-f]{40}$/.test(commit)) {
    throw new Error(`--github takes a full commit hash, not ${commit}: a branch would make the copy unrepeatable`)
  }
  const url = `https://raw.githubusercontent.com/JetBrains/intellij-community/${commit}/${COMMUNITY_PATH}`
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`${url}: ${response.status} ${response.statusText}`)
  }
  return { text: await response.text(), source: `intellij-community ${commit}, ${COMMUNITY_PATH}` }
}

/** The copy: a header with its source, then the file as it is */
export function copyOf({ text, source }) {
  if (!text.includes("--islands-")) {
    throw new Error(`${source} has no --islands-* variables`)
  }
  return `/* Copied by scripts/sync-islands-theme.mjs from\n   ${source}.\n   Do not edit: run the script again. */\n\n${text}`
}

async function main() {
  const { values } = parseArgs({
    options: { monorepo: { type: "string" }, ref: { type: "string", default: "HEAD" }, github: { type: "string" } },
  })
  const checkout = values.monorepo ?? process.env.INTELLIJ_MONOREPO
  if (values.github === undefined && checkout === undefined) {
    throw new Error("Pass --monorepo <checkout> (or INTELLIJ_MONOREPO) or --github <commit>")
  }
  const file = values.github === undefined ? fromMonorepo(checkout, values.ref) : await fromGitHub(values.github)
  writeFileSync(TARGET, copyOf(file))
  console.log(`src/islands.generated.css: ${file.source}`)
}

if (import.meta.main) {
  await main()
}
