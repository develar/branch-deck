// The architecture rules of the page that no type checks (`pnpm lint:arch`). The page acts on the code of an error
// (`ApiError.code`, `getErrorCode` of src/lib/errors.ts), never on its words, which the backend may change:
// - `message-control`: the words of an error compared (`===`, `!==`, `==`, `!=`, except with null or undefined),
//   searched (`includes`, `startsWith`, `endsWith`, `match`, `indexOf`, a regex `test`) or switched on. The words are
//   `.message` (also `["message"]`), `getErrorDetails(…)`, `describeError(…)`, `String(error)`, and a name bound to
//   one of them (`const { message } = result.error`, `const text = getErrorDetails(error)`);
// - `error-kind`: `CommandErrorKind` or the `kind` of an error (`error.kind`, `result.error.kind`), the type the
//   error had before `ApiError`; the `kind` of a tagged union (a robot's failure, a diff row) is no error;
// - `route-error-code`: a `routeCommandError` of an e2e test without the code its error answers.
// The facts of the remote services are the session's (docs/design/rust-domain-authority.md, "Факты"), asked by its
// scheduler on its clock; no store or hook polls on a timer of its own:
// - `poll-timer`: a `setTimeout` or a `setInterval` in src/stores or src/hooks (their tests aside); a timer of the UI (a
//   debounce, a hover intent, the tick of the relative times) is an entry of ALLOWLIST that says so.
// The rules of the domain are Rust's (`domain-policy`, docs/design/rust-domain-authority.md, s8-policy): the page presents
// what the view decides and decides nothing itself:
// - `rule-module`: a rule module whose rule moved into Rust comes back (`RULE_MODULES`), or src/lib or src/hooks (their
//   tests aside) exports a function or a constant named as a rule: `can…`, `needs…`, `allows…`, `…Blocker`,
//   `…Decision`, `is…Blocked`; a formatter (`format…`) words a reason Rust gave and decides nothing.
// Scanned: src/**/*.ts(x) and tests/e2e/**/*.ts, comments left out. An exception is an entry of ALLOWLIST with its
// file, its rule and the reason it holds; an entry that matches nothing fails, so the list never outlives its code.
import { globSync, readFileSync } from "node:fs"
import { join } from "node:path"

/** `{ file, rule, reason }`: a finding of `rule` in `file` that holds, and why */
export const ALLOWLIST = [
  { file: "src/hooks/useNow.ts", rule: "poll-timer", reason: "the tick of the relative times and of the time of a running robot: no request" },
  { file: "src/stores/repository.ts", rule: "poll-timer", reason: "the branch prefix of the selected repository loads once the switch rested 50 ms: a debounce" },
  { file: "src/stores/update-project.ts", rule: "poll-timer", reason: "the bar of an Update Project that ended stays for END_HOLD_MS: no request" },
  { file: "src/stores/file-stream.ts", rule: "poll-timer", reason: "the streamed texts of a file are applied in batches of BATCH_MS: a render budget, no request" },
  {
    file: "src/stores/setup.ts",
    rule: "poll-timer",
    reason: "while Settings → Tools shows them, the CLIs of the machine are read every TOOLS_READ_INTERVAL_MS: no fact of a repository's session",
  },
  { file: "src/stores/activity.ts", rule: "poll-timer", reason: "the undo of an Activity entry is offered for UNDO_WINDOW_MS: no request" },
  { file: "src/hooks/useBranchSuggestions.ts", rule: "poll-timer", reason: "the names are suggested once typing rests: a debounce of the input" },
  { file: "src/hooks/useStackPathPointer.ts", rule: "poll-timer", reason: "the hover intent of a stack path: no request" },
  { file: "src/hooks/useReviewerSearch.ts", rule: "poll-timer", reason: "the reviewers are searched once typing rests: a debounce of the input" },
  { file: "src/hooks/usePreviewHotkeys.ts", rule: "poll-timer", reason: "the dwell of the hover intent before a preview: no request" },
  { file: "src/hooks/useCopyToClipboard.ts", rule: "poll-timer", reason: "the Copied state for 2 s: no request" },
  { file: "src/hooks/useCommitDrag.ts", rule: "poll-timer", reason: "the drag image goes once the browser took it: no request" },
  { file: "src/hooks/useAutoUpdate.ts", rule: "poll-timer", reason: "one check for updates shortly after the start of the app, never again by a timer" },
  { file: "src/hooks/useMovePreview.ts", rule: "poll-timer", reason: "the dry run of a move waits for the selection to rest: a debounce" },
  {
    file: "src/lib/patronus.ts",
    rule: "rule-module",
    reason:
      "needsRobotAttention and its branch and stack forms: the failed robots the landing overview, Attention and the Dock badge count, until the robots overview is a part of the view (s9)",
  },
  {
    file: "src/hooks/useCommitReviewActions.ts",
    rule: "rule-module",
    reason: "commitReviewBlocker: the review of the commit rows the user selected, a selection of the page and no action of a subject Rust offers",
  },
]

const SOURCES = ["src/**/*.{ts,tsx}", "tests/e2e/**/*.ts"]

/** The rule modules of the page whose rules Rust decides now (`domain-policy`): deleted, they don't come back */
export const RULE_MODULES = ["src/lib/branch-next-step.ts", "src/lib/landing-node.ts", "src/lib/safe-push-decision.ts"]
// the export of a function or a constant named as a rule, its name the group
const RULE_EXPORT = /^\s*export\s+(?:async\s+)?(?:function\s*\*?\s*|const\s+|let\s+)((?:can|needs|allows)[A-Z][\w$]*|(?!format)[\w$]*(?:Blocker|Decision)|is[\w$]*Blocked)\b/
/** The sources whose exports a rule name refuses: the modules and the hooks of the page, their tests aside */
const RULED = /^src\/(?:lib|hooks)\/(?!.*\.test\.tsx?$)/

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
const TIMER = /(?<![\w$])(?:window\.)?set(?:Timeout|Interval)\(/
/** The sources whose timers are polls unless they say otherwise: the stores and the hooks, their tests aside */
const TIMED = /^src\/(?:stores|hooks)\/(?!.*\.test\.tsx?$)/

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
  if (RULE_MODULES.includes(file)) {
    problems.push({ file, line: 1, rule: "rule-module", text: "a rule module whose rule Rust decides (domain-policy)" })
  }
  lines.forEach((line, index) => {
    const at = { file, line: index + 1, text: source.split("\n")[index].trim() }
    if (controlled.test(line)) {
      problems.push({ ...at, rule: "message-control" })
    }
    if (ERROR_KIND.test(line)) {
      problems.push({ ...at, rule: "error-kind" })
    }
    if (TIMED.test(file) && TIMER.test(line)) {
      problems.push({ ...at, rule: "poll-timer" })
    }
    if (RULED.test(file) && RULE_EXPORT.test(line)) {
      problems.push({ ...at, rule: "rule-module" })
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
    console.error(
      `${problems.length} architecture problem(s): the page acts on the code of an error, never on its words, polls on no timer of its own and decides no rule of the domain (scripts/lint-arch.mjs)`,
    )
    process.exit(1)
  }
}
