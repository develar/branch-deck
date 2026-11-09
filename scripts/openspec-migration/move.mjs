// The move of the specs to OpenSpec (docs/contributing/specs.md), once, on any HEAD of master: `node
// scripts/openspec-migration/move.mjs`. Every `spec/<area>/<name>.spec.md` goes to `openspec/specs/<area>/<name>/spec.md`
// by `git mv`; in it, the `targets:` and the `[@test]` paths become paths from the root, its markdown links are
// recomputed for its new place, and a spec it names by its file alone by the new path. Every other tracked text file
// but CHANGELOG.md names a spec by its new path: `spec/<area>/<name>.spec.md` becomes `openspec/specs/<area>/<name>/spec.md`
// with its prefix, its anchor and its quotes. A test fixture of another repository (`spec/window.spec.md`) is no spec of
// this one and stays.
import { execFileSync } from "node:child_process"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, join, posix } from "node:path"

const root = join(import.meta.dirname, "..", "..")
const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 1 << 28 })

/** The new path of every spec, by its old one */
const moved = new Map(
  git("ls-files", "-z", "spec/")
    .split("\0")
    .filter(path => /^spec\/[a-z0-9-]+\/[a-z0-9-]+\.spec\.md$/.test(path))
    .map(path => [path, path.replace(/^spec\/([a-z0-9-]+)\/([a-z0-9-]+)\.spec\.md$/, "openspec/specs/$1/$2/spec.md")]),
)
/** The new path of a spec by its file name alone, when one spec has it */
const byName = new Map()
for (const [old, path] of moved) {
  const name = posix.basename(old)
  byName.set(name, byName.has(name) ? null : path)
}
/** The files outside spec/ that name a spec by its file name alone in their prose, not as a fixture */
const NAMED_ALONE = [
  "crates/bd-remote/src/scenario_test.rs",
  "crates/domain-policy/cases/safe-push.json",
  "crates/domain-types/src/sign_in.rs",
  "crates/review-plan/src/steps.rs",
  "docs/design/rust-domain-authority.md",
  "docs/design/ui-theme.md",
  "src/lib/sign-in-run.ts",
  "src/model/auth-facts.ts",
  "src/stores/auth.ts",
]
/** A path of a spec of this repository, from the root or after `../`: `plugins/air/spec/x/y.spec.md` is another's */
const OLD_PATH = /(?<![\w-]|[\w-]\/)spec\/([a-z0-9-]+)\/([a-z0-9-]+)\.spec\.md\b/g
/** `text` with every path of a spec that moved replaced by its new path */
const renamed = text => text.replace(OLD_PATH, (match, area, name) => (moved.has(`spec/${area}/${name}.spec.md`) ? `openspec/specs/${area}/${name}/spec.md` : match))
const ALONE = /(?<![/\w-])([a-z0-9-]+\.spec\.md)\b/g

/** `path` of a markdown link of the file `from` (old place) as the file `to` (new place) names it */
function relink(from, to, path) {
  if (/^(?:[a-z]+:|#|\/)/.test(path)) {
    return path
  }
  const [file, anchor] = path.split(/(?=#)/)
  if (file === "") {
    return path
  }
  const target = posix.normalize(posix.join(posix.dirname(from), file))
  return `${posix.relative(posix.dirname(to), moved.get(target) ?? target)}${anchor ?? ""}`
}

/** The text of the spec `old`, moved to `path` */
function movedSpec(old, path, text) {
  const lines = text.split("\n")
  const end = lines[0] === "---" ? lines.indexOf("---", 1) : -1
  let fence = false
  return lines
    .map((line, index) => {
      if (index < end) {
        return line.replace(/^(\s+- )(\.\.\/\S+)$/, (_, mark, target) => mark + posix.join(posix.dirname(old), target))
      }
      if (/^\s*```/.test(line)) {
        fence = !fence
      }
      const test = /^(\s*\[@test\] )(\S+)( .*)$/.exec(line)
      if (test !== null) {
        return test[1] + posix.join(posix.dirname(old), test[2]) + test[3]
      }
      if (fence) {
        return renamed(line)
      }
      // the links of the prose, not the examples of code spans (`[text](url)`)
      const relinked = line
        .split(/(`[^`]*`)/)
        .map((part, at) =>
          at % 2 === 1
            ? part
            : part
                .replace(/(\]\()([^)\s]+)(\))/g, (_, open, target, close) => open + relink(old, path, target) + close)
                .replace(/^(\[[^\]]+\]: )(\S+)/, (_, open, target) => open + relink(old, path, target)),
        )
        .join("")
      return renamed(relinked).replace(/(\]\([^)]*\)|`[^`]*`)|(?<![/\w-])([a-z0-9-]+\.spec\.md)\b/g, (match, kept, name) =>
        kept !== undefined ? kept.replace(ALONE, alone => byName.get(alone) ?? alone) : (byName.get(name) ?? match),
      )
    })
    .join("\n")
}

for (const [old, path] of moved) {
  const text = readFileSync(join(root, old), "utf8")
  mkdirSync(join(root, dirname(path)), { recursive: true })
  git("mv", old, path)
  writeFileSync(join(root, path), movedSpec(old, path, text))
}

const BINARY = /\.(?:png|jpe?g|gif|webp|ico|icns|woff2?|ttf|otf|zip|gz|pdf|car|mp4|webm)$/i
for (const file of git("ls-files", "-z").split("\0")) {
  if (file === "" || file === "CHANGELOG.md" || file.startsWith("openspec/") || BINARY.test(file)) {
    continue
  }
  let text
  try {
    text = readFileSync(join(root, file), "utf8")
  } catch {
    continue
  }
  let next = renamed(text)
  if (NAMED_ALONE.includes(file)) {
    next = next.replace(ALONE, name => byName.get(name) ?? name)
  }
  if (next !== text) {
    writeFileSync(join(root, file), next)
  }
}
console.log(`moved ${moved.size} specs`)
