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
// The operations are the session's (docs/design/operations.md): Rust admits, joins and queues every one of them and lays
// them over the actions of the view; the page only knows which of its surfaces started one (`src/stores/indicators.ts`):
// - `operations-store`: the deleted registry of the page comes back (`src/stores/operations.ts`), or src/stores or
//   src/lib (their tests aside) exports an admission of its own: `runOperation`, `getOperationBlocker`, `findSame`,
//   `operationItem`, `whenRewritesDone`, `whileSyncing`, `setSessionStep`, or a function that admits, queues or joins
//   an operation (`admit…`, `queue…Operation`, `join…Operation`).
// The main window reads nothing before its subscription (docs/design/rust-domain-authority.md, s13-warm-start): the start
// injects the frame of its session, the view names the prefix, the subscription answers a missing git with its code,
// and the session syncs a repository synced before as it opens:
// - `startup-read`: a command in the start of the main window (`src/main.tsx`, `src/router.ts`) or in the store of the
//   selected repository (`src/stores/repository.ts`) but those the user asks for (`USER_COMMANDS`), a read of the
//   prefix outside the Settings window (`commands.resolveBranchPrefix`), or an open sync of the page
//   (`syncBranches("open")`), in src (its tests aside).
// The page calls the engine only through its intents (docs/design/rust-domain-authority.md, s14-fence); a command of the
// app (`app(…)` of crates/app-api/src/commands.rs) has no plan and one calling module:
// - `intent-call`: an `engine` command of the registry (`commands.execute`, `commands.cancelOperation`) called outside
//   `src/model/intents.ts`, or an `app(…)` command called outside its module of `APP_COMMAND_CALLERS`, in src (its tests
//   aside); an entry of `APP_COMMAND_CALLERS` that names no `app(…)` command or whose module doesn't call it fails too.
// A presenter words what the caller selected of the view (src/presenters):
// - `presenter-state`: a `.getState()` of a store, or an import of a value of a store module (`@/stores/…`,
//   `@/model/store`: its getters read the store, `getRowPart`, `getPullRequest`, `getOperationWords`), in src/presenters
//   (their tests aside): a presenter imports a store for its types only, the caller passes the part it selected.
// A run in the repository of its press (docs/design/rust-domain-authority.md, "Конкурентность"): the key of the selected
// project is read before the run waits for anything, never after the user may have selected another repository:
// - `late-project-read`: a read of the selected project (`getSelectedProject`, `getSelectedRepositoryPath`,
//   `getDiffRepositoryPath`, `getEffectiveBranchPrefix`, `getFullBranchName`, `getFullStackBranchName`,
//   `vcsRequestFactory.`, `repositoryStore.getState()`) after the first `await` of an async function, its operand
//   included, in src (its tests aside): the function captures the key before it.
// Scanned: src/**/*.ts(x) and tests/e2e/**/*.ts, comments left out. An exception is an entry of ALLOWLIST with its
// file, its rule and the reason it holds, only of a timer of the UI (`poll-timer`, `EXCEPTED_RULES`): every other rule
// holds without exceptions; an entry that matches nothing fails, so the list never outlives its code.
import { globSync, readFileSync } from "node:fs"
import { join } from "node:path"

/** `{ file, rule, reason }`: a finding of `rule` in `file` that holds, and why */
export const ALLOWLIST = [
  { file: "src/hooks/useNow.ts", rule: "poll-timer", reason: "the tick of the relative times and of the time of a running robot: no request" },
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
]

const SOURCES = ["src/**/*.{ts,tsx}", "tests/e2e/**/*.ts"]
/** The rules an entry of ALLOWLIST may except: a timer of the UI, which `poll-timer` cannot tell from a poll */
const EXCEPTED_RULES = new Set(["poll-timer"])

/** The rule modules of the page whose rules Rust decides now (`domain-policy`): deleted, they don't come back */
export const RULE_MODULES = ["src/lib/branch-next-step.ts", "src/lib/landing-node.ts", "src/lib/safe-push-decision.ts"]
/** The registry of the operations the page kept until the session admitted every one of them: deleted, it doesn't come back */
export const OPERATIONS_MODULE = "src/stores/operations.ts"
// the export of an admission of the page, its name the group
const ADMISSION_EXPORT =
  /^\s*export\s+(?:async\s+)?(?:function\s*\*?\s*|const\s+|let\s+)(runOperation|getOperationBlocker|findSame|operationItem|whenRewritesDone|whileSyncing|setSessionStep|admit[\w$]*|(?:queue|join)[\w$]*Operation)\b/
/** The sources whose exports an admission name refuses: the stores and the modules of the page, their tests aside */
const ADMITTING = /^src\/(?:stores|lib)\/(?!.*\.test\.tsx?$)/
// the export of a function or a constant named as a rule, its name the group
const RULE_EXPORT = /^\s*export\s+(?:async\s+)?(?:function\s*\*?\s*|const\s+|let\s+)((?:can|needs|allows)[A-Z][\w$]*|(?!format)[\w$]*(?:Blocker|Decision)|is[\w$]*Blocked)\b/
/** The sources whose exports a rule name refuses: the modules and the hooks of the page, their tests aside */
const RULED = /^src\/(?:lib|hooks)\/(?!.*\.test\.tsx?$)/

/** The start of the main window and the store of the selected repository: they ask no command at start */
export const STARTUP_MODULES = ["src/main.tsx", "src/router.ts", "src/stores/repository.ts"]
/** The commands of `STARTUP_MODULES` that the user asks for (Browse), never asked at start */
const USER_COMMANDS = new Set(["browseRepository"])
const COMMAND_CALL = /\bcommands\.([\w$]+)/g
// the reads the start of the page made before its subscription, deleted in s13: the prefix (only Settings reads it,
// the view names it for the main window) and the open sync (the session runs it as it opens)
const PREFIX_READ = /\bcommands\.resolveBranchPrefix\b/
const OPEN_SYNC = /\bsyncBranches\(\s*(["'`])open\1/
/** The sources whose start a read refuses: the page, its tests aside */
const STARTING = /^src\/(?!.*\.test\.tsx?$)/
/** The window that reads the prefix of its own: Settings */
const SETTINGS = /^src\/components\/settings\//

/** The module of the page that calls the engine (`engine` of the registry): the intents of a repository */
export const INTENTS_MODULE = "src/model/intents.ts"
/** The one module of the page that calls each `app(…)` command of the registry (crates/app-api/src/commands.rs) */
export const APP_COMMAND_CALLERS = {
  cancel_symbol_search: "src/hooks/useSymbolNavigation.ts",
  cancel_conflict_fix: "src/stores/conflict-fix.ts",
  set_window_focus: "src/model/store.ts",
  sync: "src/model/store.ts",
  link_pull_request: "src/stores/reviews.ts",
  unlink_pull_request: "src/stores/reviews.ts",
  sign_in: "src/lib/sign-in-run.ts",
  cancel_sign_in: "src/lib/sign-in-run.ts",
  sign_out: "src/lib/sign-in-run.ts",
  record_activity: "src/stores/activity.ts",
  clear_activity: "src/stores/activity.ts",
  set_setting: "src/stores/settings/persistent-store.ts",
  set_repository_config: "src/hooks/useConfigWrite.ts",
  open_sub_window: "src/lib/window-management.ts",
  sub_window_loaded: "src/lib/sub-window.ts",
  sub_window_drawn: "src/lib/sub-window.ts",
  sub_window_ready: "src/lib/sub-window.ts",
  show_window_menu: "src/lib/title-bar-mouse.ts",
  browse_repository: "src/stores/repository.ts",
  set_auto_sync_menu_checked: "src/hooks/useMenuSync.ts",
  restart_app: "src/components/settings/AppearancePage.tsx",
  check_for_updates: "src/hooks/useAutoUpdate.ts",
  install_update: "src/hooks/useAutoUpdate.ts",
  install_agent_cli: "src/hooks/useInstallAgentCli.ts",
  run_cli_tool_action: "src/stores/cli-tools.ts",
  cancel_cli_tool_action: "src/stores/cli-tools.ts",
}
/** The table of the registry (`commands_json_test.rs` writes it from `COMMANDS`): the class of each command */
const REGISTRY = "crates/app-api/commands.json"
/** The sources whose calls of the registry the classes refuse: the page, its tests aside */
const CALLING = /^src\/(?!.*\.test\.tsx?$)/
// a call of a command of the bindings, across lines as Oxfmt breaks a chain (`commands\n  .setRepositoryConfig(…)`)
const BINDING_CALL = /\bcommands\s*\.\s*([\w$]+)/g

/** The commands of the registry at `root` by the name the bindings give them (`signIn`): `{ name, class }` */
export function readRegistry(root) {
  let table
  try {
    table = JSON.parse(readFileSync(join(root, REGISTRY), "utf8"))
  } catch {
    return new Map()
  }
  return new Map(table.map(command => [command.name.replace(/_([a-z0-9])/g, (_, letter) => letter.toUpperCase()), { name: command.name, class: command.class }]))
}

/** The calls of the commands of `registry` in `text`: `{ command, line }`, the command as the registry names it */
function registryCalls(text, registry) {
  return [...text.matchAll(BINDING_CALL)].flatMap(call => {
    const command = registry.get(call[1])
    return command === undefined ? [] : [{ command, line: text.slice(0, call.index + call[0].length).split("\n").length }]
  })
}

/** The sources whose presenters word what the caller selected: src/presenters, their tests aside */
const PRESENTING = /^src\/presenters\/(?!.*\.test\.tsx?$)/
const STATE_READ = /\.getState\s*\(/
// an import of a value of a store module, over lines: its getters read the store; `import type` reads nothing
const STORE_IMPORT = /(?<![\w$])import\s+(?!type\s)[^;"'`]*?\bfrom\s*["'](?:@\/stores\/[^"']+|@\/model\/store)["']/g

/** The sources whose runs read the selected project before they wait: the page, its tests aside */
const RUNNING = /^src\/(?!.*\.test\.tsx?$)/
// a read of the selected project, of its path, its prefix, the full names of its branches or its requests
const PROJECT_READ =
  /(?<![\w$])(?:getSelectedProject|getSelectedRepositoryPath|getDiffRepositoryPath|getEffectiveBranchPrefix|getFullBranchName|getFullStackBranchName)\s*\(|(?<![\w$])vcsRequestFactory\s*\.|(?<![\w$])repositoryStore\s*\.\s*getState\s*\(/g
// a token of an async function: `async function`, `async (…) =>`, `async x =>`, `async method(…) {`
const ASYNC = /(?<![\w$.])async(?![\w$])/g
const AWAIT = /(?<![\w$.])await(?![\w$])/g

/** `text` with the contents of its quoted strings blanked (a template keeps its code), so a brace or a word in one counts for nothing */
function withoutStrings(text) {
  return text.replace(/"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'/g, quoted => quoted[0] + " ".repeat(quoted.length - 2) + quoted[0])
}

const OPENING = { "(": ")", "[": "]", "{": "}", "<": ">" }

/** The index after the bracket that closes the one at `open` (`(`, `[`, `{`, `<`), or -1 */
function afterClosing(code, open) {
  const close = OPENING[code[open]]
  let depth = 0
  for (let index = open; index < code.length; index++) {
    if (code[index] === "=" && code[index + 1] === ">") {
      index++
    } else if (code[index] === code[open]) {
      depth++
    } else if (code[index] === close && --depth === 0) {
      return index + 1
    }
  }
  return -1
}

function skipSpace(code, index) {
  while (index < code.length && /\s/.test(code[index])) {
    index++
  }
  return index
}

/** The body `{ start, end }` of the block at `open` (`{`): the text between its braces */
function blockAt(code, open) {
  const end = afterClosing(code, open)
  return end === -1 ? null : { start: open + 1, end: end - 1 }
}

/** The body of an arrow whose `=>` ends at `index`: its block, else its expression up to where it ends */
function arrowBody(code, index) {
  const start = skipSpace(code, index)
  if (code[start] === "{") {
    return blockAt(code, start)
  }
  let depth = 0
  for (let at = start; at < code.length; at++) {
    const char = code[at]
    if ("([{".includes(char)) {
      depth++
    } else if (")]}".includes(char) && --depth < 0) {
      return { start, end: at }
    } else if (depth === 0 && (char === "," || char === ";")) {
      return { start, end: at }
    } else if (depth === 0 && char === "\n") {
      // a line of its own ends the expression, one that goes on (`.then(…)`, `?? x`, an operator at the end) doesn't
      const next = code[skipSpace(code, at)] ?? ""
      const last = code.slice(start, at).trimEnd().at(-1) ?? ""
      if (!".?:&|+-*/%=<>".includes(next) && !"=>&|+-*/%?:(,".includes(last)) {
        return { start, end: at }
      }
    }
  }
  return { start, end: code.length }
}

/** The body after the parameters of a function that end at `index`: past its return type to its `{` (`arrow`: to `=>`) */
function bodyAfterParameters(code, index, arrow) {
  let at = skipSpace(code, index)
  if (code[at] !== ":") {
    return arrow ? (code.startsWith("=>", at) ? arrowBody(code, at + 2) : null) : code[at] === "{" ? blockAt(code, at) : null
  }
  let depth = 0
  for (at++; at < code.length; at++) {
    if (depth === 0 && arrow && code.startsWith("=>", at)) {
      return arrowBody(code, at + 2)
    }
    if (depth === 0 && !arrow && code[at] === "{") {
      return blockAt(code, at)
    }
    if (code.startsWith("=>", at)) {
      at++
    } else if ("([{<".includes(code[at])) {
      depth++
    } else if (")]}>".includes(code[at]) && --depth < 0) {
      return null
    }
  }
  return null
}

/** The body of the async function whose `async` ends at `index`, or null when no function follows */
function asyncBodyAfter(code, index) {
  let at = skipSpace(code, index)
  let arrow = true
  if (/^function(?![\w$])/.test(code.slice(at, at + 9))) {
    arrow = false
    at = skipSpace(code, at + "function".length)
    at = code[at] === "*" ? skipSpace(code, at + 1) : at
    at = skipSpace(code, at + (/^[\w$]*/.exec(code.slice(at))?.[0].length ?? 0))
  } else {
    const name = /^[\w$]+/.exec(code.slice(at))?.[0]
    if (name !== undefined) {
      const after = skipSpace(code, at + name.length)
      if (code.startsWith("=>", after)) {
        return arrowBody(code, after + 2)
      }
      // a method: `async load(…) {`
      arrow = false
      at = after
    }
  }
  if (code[at] === "<") {
    at = skipSpace(code, afterClosing(code, at))
  }
  if (code[at] !== "(") {
    return null
  }
  const parameters = afterClosing(code, at)
  return parameters === -1 ? null : bodyAfterParameters(code, parameters, arrow)
}

/** The indexes in `code` of the reads of the selected project after the first `await` of an async function */
function lateProjectReads(code) {
  const bodies = [...code.matchAll(ASYNC)].flatMap(token => asyncBodyAfter(code, token.index + token[0].length) ?? [])
  const reads = new Set()
  for (const body of bodies) {
    const nested = bodies.filter(other => other !== body && other.start >= body.start && other.end <= body.end)
    const text = code.slice(0, body.end)
    AWAIT.lastIndex = body.start
    let first = AWAIT.exec(text)
    while (first !== null && nested.some(other => first.index >= other.start && first.index < other.end)) {
      first = AWAIT.exec(text)
    }
    if (first === null) {
      continue
    }
    for (const read of text.slice(first.index).matchAll(PROJECT_READ)) {
      reads.add(first.index + read.index)
    }
  }
  return [...reads].toSorted((a, b) => a - b)
}

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

/** Whether `line` of `file` reads at the start of the main window: a command of its start, the prefix, an open sync */
function startupRead(file, line) {
  if (STARTUP_MODULES.includes(file) && [...line.matchAll(COMMAND_CALL)].some(([, command]) => !USER_COMMANDS.has(command))) {
    return true
  }
  return (!SETTINGS.test(file) && PREFIX_READ.test(line)) || OPEN_SYNC.test(line)
}

/** The registry of this repository: the default of `findProblems` */
const OWN_REGISTRY = readRegistry(join(import.meta.dirname, ".."))

/** Whether `file` may call `command` of the registry (`{ name, class }`): the engine from the intents, a command of the app from its module */
function callAllowed(file, command, callers) {
  switch (command.class) {
    case "engine":
      return file === INTENTS_MODULE
    case "app":
      return callers[command.name] === file
    default:
      return true
  }
}

/**
 * The findings of one source: `{ file, line, rule, text }`; `registry` (`readRegistry`) and `callers`
 * (`APP_COMMAND_CALLERS`) say who calls a command
 */
export function findProblems(file, source, { registry = OWN_REGISTRY, callers = APP_COMMAND_CALLERS } = {}) {
  const text = withDotMessage(withoutComments(source))
  const lines = text.split("\n")
  const sourceLines = source.split("\n")
  const problems = []
  const controlled = messageControl(text)
  if (RULE_MODULES.includes(file)) {
    problems.push({ file, line: 1, rule: "rule-module", text: "a rule module whose rule Rust decides (domain-policy)" })
  }
  if (file === OPERATIONS_MODULE) {
    problems.push({ file, line: 1, rule: "operations-store", text: "the registry of the page: the session admits every operation (RepositoryView.operations)" })
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
    if (ADMITTING.test(file) && ADMISSION_EXPORT.test(line)) {
      problems.push({ ...at, rule: "operations-store" })
    }
    if (STARTING.test(file) && startupRead(file, line)) {
      problems.push({ ...at, rule: "startup-read" })
    }
    if (PRESENTING.test(file) && STATE_READ.test(line)) {
      problems.push({ ...at, rule: "presenter-state" })
    }
  })
  if (PRESENTING.test(file)) {
    for (const found of text.matchAll(STORE_IMPORT)) {
      const line = text.slice(0, found.index).split("\n").length
      problems.push({ file, line, rule: "presenter-state", text: sourceLines[line - 1].trim() })
    }
  }
  if (CALLING.test(file)) {
    for (const { command, line } of registryCalls(withoutStrings(text), registry)) {
      if (!callAllowed(file, command, callers)) {
        problems.push({ file, line, rule: "intent-call", text: sourceLines[line - 1].trim() })
      }
    }
  }
  if (RUNNING.test(file)) {
    const code = withoutStrings(text)
    for (const index of lateProjectReads(code)) {
      const line = code.slice(0, index).split("\n").length
      problems.push({ file, line, rule: "late-project-read", text: sourceLines[line - 1].trim() })
    }
  }
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

/**
 * The findings of the repository at `root` that the allowlist does not hold, the entries that match nothing, and the
 * entries of `callers` (`APP_COMMAND_CALLERS`) that name no `app(…)` command of its registry or whose module doesn't call it
 */
export function lintArchitecture(root, allowlist = ALLOWLIST, callers = APP_COMMAND_CALLERS) {
  const used = new Set()
  const problems = []
  const registry = readRegistry(root)
  // the modules that call each command of the app, by its name in the registry
  const called = new Map()
  // `/` on every platform, as the allowlist names a file
  for (const file of globSync(SOURCES, { cwd: root })
    .map(path => path.replaceAll("\\", "/"))
    .toSorted()) {
    const source = readFileSync(join(root, file), "utf8")
    if (CALLING.test(file)) {
      for (const { command } of registryCalls(withoutStrings(withoutComments(source)), registry)) {
        called.set(command.name, (called.get(command.name) ?? new Set()).add(file))
      }
    }
    for (const problem of findProblems(file, source, { registry, callers })) {
      const allowed = allowlist.findIndex(entry => entry.file === problem.file && entry.rule === problem.rule)
      if (allowed === -1) {
        problems.push(`${problem.file}:${problem.line}: ${problem.rule}: ${problem.text}`)
      } else {
        used.add(allowed)
      }
    }
  }
  for (const [name, module] of Object.entries(callers)) {
    const command = [...registry.values()].find(it => it.name === name)
    if (command?.class !== "app") {
      problems.push(`APP_COMMAND_CALLERS: ${name} is no app(…) command of ${REGISTRY}: remove it`)
    } else if (called.get(name)?.has(module) !== true) {
      problems.push(`APP_COMMAND_CALLERS: ${name} is not called in ${module}: name the module that calls it, or remove it`)
    }
  }
  allowlist.forEach((entry, index) => {
    if (!EXCEPTED_RULES.has(entry.rule)) {
      problems.push(`ALLOWLIST: ${entry.file} (${entry.rule}): the rule takes no exception, only a timer of the UI does (poll-timer)`)
    } else if (typeof entry.reason !== "string" || entry.reason.trim() === "") {
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
      `${problems.length} architecture problem(s): the page acts on the code of an error, never on its words, polls on no timer of its own, decides no rule of the domain, admits no operation, reads nothing before its subscription, calls the engine only through its intents and a command of the app from one module, presents what the caller selected and reads the selected project before it waits (scripts/lint-arch.mjs)`,
    )
    process.exit(1)
  }
}
