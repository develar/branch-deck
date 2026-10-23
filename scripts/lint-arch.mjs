// The architecture rules of the page that no type checks (`pnpm lint:arch`). The page acts on the code of an error
// (`ApiError.code`, `getErrorCode` of src/lib/errors.ts), never on its words, which the backend may change:
// - `message-control`: `.message` compared (`===`, `!==`, `==`, `!=`, except with null or undefined) or searched
//   (`includes`, `startsWith`, `endsWith`, `match`, `indexOf`, a regex `test`);
// - `error-kind`: `CommandErrorKind` or the `kind` of an error (`error.kind`, `result.error.kind`), the type the
//   error had before `ApiError`; the `kind` of a tagged union (a robot's failure, a diff row) is no error;
// - `route-error-code`: a `routeCommandError` of an e2e test without the code its error answers.
// Scanned: src/**/*.ts(x) and tests/e2e/**/*.ts, comments left out. An exception is an entry of ALLOWLIST with its
// file, its rule and the reason it holds; an entry that matches nothing fails, so the list never outlives its code.
import { globSync, readFileSync } from "node:fs"
import { join } from "node:path"

/** `{ file, rule, reason }`: a finding of `rule` in `file` that holds, and why */
export const ALLOWLIST = []

const SOURCES = ["src/**/*.{ts,tsx}", "tests/e2e/**/*.ts"]

// the whole receiver (`result.error.message`), so `typeof error.message === "string"` is a check of the type
const COMPARED = /(?<![\w$.?])(?<!typeof\s+)[\w$][\w$.?]*\.message\s*[!=]=?=(?!=)\s*(?!\s*(?:null|undefined)\b)/
const COMPARED_TO = /(?<![!=<>])[!=]=?=\s*[\w$][\w$.?]*\.message\b(?!\s*[(.\w])/
const SEARCHED = /\.message\??\.(?:includes|startsWith|endsWith|match|matchAll|indexOf|lastIndexOf|search)\(/
const TESTED = /\.test\(\s*[\w$.?]*\.message\s*\)/
const ERROR_KIND = /\bCommandErrorKind\b|\b(?:error|err|apiError|rejection)\??\.kind\b|\.error\??\.kind\b/
const ROUTE_ERROR = /\brouteCommandError\(/g

/** The text of a source without its comments: a line or block comment becomes spaces, so columns and lines stay */
function withoutComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\/|(?<![:"'`\w])\/\/[^\n]*/g, comment => comment.replace(/[^\n]/g, " "))
}

/** The arguments of the call whose `(` is at `open`: the text up to its closing parenthesis */
function argumentsAt(text, open) {
  let depth = 0
  for (let index = open; index < text.length; index++) {
    if (text[index] === "(") {
      depth++
    } else if (text[index] === ")" && --depth === 0) {
      return text.slice(open + 1, index)
    }
  }
  return text.slice(open + 1)
}

/** The findings of one source: `{ file, line, rule, text }` */
export function findProblems(file, source) {
  const text = withoutComments(source)
  const lines = text.split("\n")
  const problems = []
  lines.forEach((line, index) => {
    const at = { file, line: index + 1, text: source.split("\n")[index].trim() }
    if (COMPARED.test(line) || COMPARED_TO.test(line) || SEARCHED.test(line) || TESTED.test(line)) {
      problems.push({ ...at, rule: "message-control" })
    }
    if (ERROR_KIND.test(line)) {
      problems.push({ ...at, rule: "error-kind" })
    }
  })
  for (const call of text.matchAll(ROUTE_ERROR)) {
    // the declaration of the helper
    if (/function\s+$/.test(text.slice(0, call.index))) {
      continue
    }
    if (!/\bcode\s*:/.test(argumentsAt(text, call.index + call[0].length - 1))) {
      const line = text.slice(0, call.index).split("\n").length
      problems.push({ file, line, rule: "route-error-code", text: source.split("\n")[line - 1].trim() })
    }
  }
  return problems
}

/** The findings of the repository at `root` that the allowlist does not hold, and the entries that match nothing */
export function lintArchitecture(root, allowlist = ALLOWLIST) {
  const used = new Set()
  const problems = []
  for (const file of globSync(SOURCES, { cwd: root }).toSorted()) {
    for (const problem of findProblems(file, readFileSync(join(root, file), "utf8"))) {
      const allowed = allowlist.findIndex(entry => entry.file === problem.file && entry.rule === problem.rule)
      if (allowed === -1) {
        problems.push(`${problem.file}:${problem.line}: ${problem.rule}: ${problem.text}`)
      } else {
        used.add(allowed)
      }
    }
  }
  allowlist.forEach((entry, index) => {
    if (typeof entry.reason !== "string" || entry.reason.trim() === "") {
      problems.push(`ALLOWLIST: ${entry.file} (${entry.rule}) gives no reason`)
    } else if (!used.has(index)) {
      problems.push(`ALLOWLIST: ${entry.file} (${entry.rule}) matches nothing: remove it`)
    }
  })
  return problems
}

if (import.meta.main) {
  const problems = lintArchitecture(join(import.meta.dirname, ".."))
  if (problems.length > 0) {
    console.error(problems.join("\n"))
    console.error(`${problems.length} architecture problem(s): the page acts on the code of an error, never on its words (scripts/lint-arch.mjs)`)
    process.exit(1)
  }
}
