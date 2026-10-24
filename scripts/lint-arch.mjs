// The architecture rules of the page that no type checks (`pnpm lint:arch`). The page acts on the code of an error
// (`ApiError.code`, `getErrorCode` of src/lib/errors.ts), never on its words, which the backend may change:
// - `message-control`: the words of an error compared (`===`, `!==`, `==`, `!=`, except with null or undefined),
//   searched (`includes`, `startsWith`, `endsWith`, `match`, `indexOf`, a regex `test`) or switched on. The words are
//   `.message` (also `["message"]`), `getErrorDetails(…)`, `describeError(…)`, `String(error)`, and a name bound to
//   one of them (`const { message } = result.error`, `const text = getErrorDetails(error)`);
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

// the words of an error: the whole receiver of `.message` (`result.error.message`), so `typeof error.message ===
// "string"` is a check of the type; the helpers that answer the message; an error as a string
const WORDS = [
  String.raw`(?<![\w$.?])[\w$][\w$.?]*\.message\b(?!\s*\()`,
  String.raw`(?<![\w$.])(?:getErrorDetails|describeError)\([^()]*\)`,
  String.raw`(?<![\w$.])String\(\s*[\w$.?]*(?:[eE]rror|\berr|\be|[fF]ailure|[rR]ejection|[rR]eason|[cC]ause)\s*\)`,
]
const SEARCH = String.raw`\??\.(?:includes|startsWith|endsWith|match|matchAll|indexOf|lastIndexOf|search)\(`
// a name bound to the words: `const { message } = result.error`, `const { message: text } = error`, `const text =
// error.message`, `const text = getErrorDetails(error)`
const DESTRUCTURED = /\{([^{}]*)\}(?=[\s}\]]*=(?![=>]))/g
const ASSIGNED = new RegExp(String.raw`\b(?:const|let|var)\s+([\w$]+)\s*(?::[^=]+)?=\s*(?:${WORDS.join("|")})\s*(?:[;\n]|$)`, "g")
// a type of a destructured field (`{ message: string }`), never a name
const TYPE_NAMES = new Set(["string", "number", "boolean", "unknown", "any", "never"])
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

/** The names `text` binds to the words of an error, as alternatives of a pattern */
function wordNames(text) {
  const names = new Set()
  for (const [, fields] of text.matchAll(DESTRUCTURED)) {
    const field = /(?:^|,)\s*message\s*(?::\s*([\w$]+))?\s*(?:=[^,]*)?(?:,|$)/.exec(fields)
    if (field !== null && !TYPE_NAMES.has(field[1] ?? "")) {
      names.add(field[1] ?? "message")
    }
  }
  for (const [, name] of text.matchAll(ASSIGNED)) {
    names.add(name)
  }
  return [...names].map(name => String.raw`(?<![\w$.?])${name.replaceAll("$", "\\$")}\b(?!\s*[(:])`)
}

/** The pattern of a branch on the words of an error, in `text`: the words compared, searched, tested or switched on */
function messageControl(text) {
  const words = `(?:${[...WORDS, ...wordNames(text)].join("|")})`
  const notNull = String.raw`(?!\s*(?:null|undefined)\b)`
  return new RegExp(
    [
      String.raw`(?<!typeof\s+)${words}\s*[!=]=?=(?!=)\s*${notNull}`,
      String.raw`(?<![!=<>])[!=]=?=\s*${words}(?!\s*[(.\w])`,
      `${words}${SEARCH}`,
      String.raw`\.test\(\s*${words}\s*\)`,
      String.raw`\bswitch\s*\(\s*${words}\s*\)`,
    ].join("|"),
  )
}

/** `error["message"]` as `error.message`, so one pattern reads both */
function withDotMessage(text) {
  return text.replace(/(\?\.)?\[\s*(["'`])message\2\s*\]/g, (_, optional) => (optional === undefined ? ".message" : "?.message"))
}

/** The findings of one source: `{ file, line, rule, text }` */
export function findProblems(file, source) {
  const text = withDotMessage(withoutComments(source))
  const lines = text.split("\n")
  const problems = []
  const controlled = messageControl(text)
  lines.forEach((line, index) => {
    const at = { file, line: index + 1, text: source.split("\n")[index].trim() }
    if (controlled.test(line)) {
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
  // `/` on every platform, as the allowlist names a file
  for (const file of globSync(SOURCES, { cwd: root })
    .map(path => path.replaceAll("\\", "/"))
    .toSorted()) {
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
