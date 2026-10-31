import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { APP_COMMAND_CALLERS, STARTUP_MODULES, findProblems, lintArchitecture, readRegistry } from "./lint-arch.mjs"

/** The rules each line of `source` breaks, by line */
function rulesOf(source) {
  return findProblems("src/stores/example.ts", source).map(problem => `${problem.line} ${problem.rule}`)
}

test("a branch on the words of an error is refused, compared or searched", () => {
  const source = [
    'if (result.error.message === "canceled") {}',
    'if (waited.error.message !== "canceled") {}',
    'if ("canceled" == error.message) {}',
    'if (error.message.includes("locked")) {}',
    'if (error.message?.startsWith("Cannot")) {}',
    'const moved = error.message.endsWith("moved") || error.message.match(/moved/) || error.message.indexOf("x") > 0',
    "if (/held/.test(result.error.message)) {}",
  ].join("\n")
  assert.deepEqual(rulesOf(source), [
    "1 message-control",
    "2 message-control",
    "3 message-control",
    "4 message-control",
    "5 message-control",
    "6 message-control",
    "7 message-control",
  ])
})

test("a branch on the words through a helper, a name, a switch, an index or a string of the error is refused", () => {
  const source = [
    'if (getErrorDetails(error) === "canceled") {}',
    'if (describeError(result.error).includes("locked")) {}',
    "const { message } = result.error",
    'if (message === "canceled") {}',
    "switch (result.error.message) {}",
    'if (result.error["message"] === "x") {}',
    'if (String(error).includes("x")) {}',
    "const text = getErrorDetails(failure)",
    'if (text.startsWith("Cannot")) {}',
    "const { message: words } = error",
    "switch (words) {}",
    'if (result.error?.["message"] !== "x") {}',
  ].join("\n")
  assert.deepEqual(rulesOf(source), [
    "1 message-control",
    "2 message-control",
    "4 message-control",
    "5 message-control",
    "6 message-control",
    "7 message-control",
    "9 message-control",
    "11 message-control",
    "12 message-control",
  ])
})

test("the words bound to a name and shown, a string of another value and a field typed string are no branch", () => {
  const source = [
    "const { message } = result.error",
    "notifyError(title, message)",
    "const text = getErrorDetails(error)",
    "toast.add({ description: text })",
    'if (String(count) === "1") {}',
    "type Failure = { message: string }",
    'if (typeof message === "string") {}',
    "if (message !== undefined) {}",
  ].join("\n")
  assert.deepEqual(rulesOf(source), [])
})

test("a check of the type, of null, an assignment and a read for the user are no branch", () => {
  const source = [
    'if (typeof error.message === "string") {}',
    "parts.push(test.message === null ? head : fence(test.message))",
    "if (failure.message !== undefined) {}",
    'error.message = "x"',
    "notifyError(title, result.error.message)",
    'if (result.error.code === "canceled") {}',
    '// a comment may say message === "canceled"',
    '/* and error.kind === "x" */',
    'const url = "https://example.com" // error.message.includes("x")',
  ].join("\n")
  assert.deepEqual(rulesOf(source), [])
})

test("the kind of an error is refused, the kind of a tagged union is not", () => {
  const source = [
    'if (error.kind === "headMoved") {}',
    'if (result.error.kind === "x") {}',
    "type K = CommandErrorKind",
    'if (node.kind === "file") {}',
    "switch (failure.kind) {}",
  ].join("\n")
  assert.deepEqual(rulesOf(source), ["1 error-kind", "2 error-kind", "3 error-kind"])
})

test("a routed command error names its code", () => {
  const source = [
    "export async function routeCommandError(page: Page, command: string, error: ApiError): Promise<void> {}",
    'await routeCommandError(page, "sync_branches", { code: "internal", message: "offline" })',
    'await routeCommandError(page, "unapply_branch", {',
    '  message: "Uncommitted files",',
    "})",
    'await routeCommandError(page, "sync_branches", failure)',
  ].join("\n")
  assert.deepEqual(rulesOf(source), ["3 route-error-code", "6 route-error-code"])
})

test("an e2e file takes the expect of its fixtures, whose poll asks every 50 ms", () => {
  const source = [
    'import { expect } from "@playwright/test"',
    'import { test, expect as check } from "@playwright/test"',
    'import type { Page } from "@playwright/test"',
    'import { expect } from "../fixtures/base-test"',
  ].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  assert.deepEqual(rules("tests/e2e/helpers/example-helpers.ts"), ["1 e2e-expect", "2 e2e-expect"])
  assert.deepEqual(rules("tests/e2e/fixtures/base-test.ts"), [])
  assert.deepEqual(rules("src/lib/example.ts"), [])
})

test("a timer of a store or a hook is a poll, of its test or another module not", () => {
  const source = ["const timer = setTimeout(ask, 300_000)", "const every = window.setInterval(read, 60_000)", "// setTimeout(ask) in a comment", "clearTimeout(timer)"].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  assert.deepEqual(rules("src/stores/example.ts"), ["1 poll-timer", "2 poll-timer"])
  assert.deepEqual(rules("src/hooks/useExample.tsx"), ["1 poll-timer", "2 poll-timer"])
  assert.deepEqual(rules("src/stores/example.test.ts"), [])
  assert.deepEqual(rules("src/lib/example.ts"), [])
})

test("an allowlist entry needs its reason and a finding to hold, and only a timer of the UI takes one", t => {
  const root = mkdtempSync(join(tmpdir(), "lint-arch-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, "src/stores"), { recursive: true })
  writeFileSync(join(root, "src/stores/a.ts"), "const timer = setTimeout(hide, 2000)\n")
  writeFileSync(join(root, "src/b.ts"), 'if (error.message === "x") {}\n')
  const problems = allowlist => lintArchitecture(root, allowlist, {})

  const word = 'src/b.ts:1: message-control: if (error.message === "x") {}'
  const timer = "src/stores/a.ts:1: poll-timer: const timer = setTimeout(hide, 2000)"
  assert.deepEqual(problems([]), [word, timer])
  assert.deepEqual(problems([{ file: "src/stores/a.ts", rule: "poll-timer", reason: "the Copied state for 2 s: no request" }]), [word])
  assert.deepEqual(problems([{ file: "src/stores/a.ts", rule: "poll-timer", reason: " " }]), [word, "ALLOWLIST: src/stores/a.ts (poll-timer) gives no reason"])
  assert.deepEqual(problems([{ file: "src/stores/b.ts", rule: "poll-timer", reason: "gone" }]), [word, timer, "ALLOWLIST: src/stores/b.ts (poll-timer) matches nothing: remove it"])
  // the words of an error, a rule of the domain: no exception
  assert.deepEqual(problems([{ file: "src/b.ts", rule: "message-control", reason: "the words of a third-party library" }]), [
    timer,
    "ALLOWLIST: src/b.ts (message-control): the rule takes no exception, only a timer of the UI does (poll-timer)",
  ])
})

test("a rule of the domain in the page is refused: a deleted rule module, an export of src/lib or src/hooks named as a rule", () => {
  const source = [
    "export function canPushNow(branch) {}",
    "export const needsPush = branch => true",
    "export function allowsPushMode(mode) {}",
    "export function getSafePushBlocker(branch) {}",
    "export async function startDecision() {}",
    "export function isStartBlocked() {}",
    "export function statusOutranksRobot(branch, landedIn) {}",
    // words of a reason Rust gave, a type, a name of no rule, a function of the module only
    "export function formatParkedBlocker(state) {}",
    "export type LandingDecision = string",
    "export function canvasOf() {}",
    "function rewriteBlocker() {}",
  ].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  const refused = ["1 rule-module", "2 rule-module", "3 rule-module", "4 rule-module", "5 rule-module", "6 rule-module", "7 rule-module"]
  assert.deepEqual(rules("src/lib/example.ts"), refused)
  assert.deepEqual(rules("src/hooks/useExample.ts"), refused)
  assert.deepEqual(rules("src/lib/example.test.ts"), [])
  assert.deepEqual(rules("src/components/Example.tsx"), [])
  assert.deepEqual(findProblems("src/lib/branch-next-step.ts", "export const x = 1\n"), [
    { file: "src/lib/branch-next-step.ts", line: 1, rule: "rule-module", text: "a rule module whose rule Rust decides (domain-policy)" },
  ])
})

test("an admission of the page is refused: the deleted registry, an export of src/stores or src/lib that admits, queues or joins", () => {
  const source = [
    "export function runOperation(spec, run) {}",
    "export function getOperationBlocker(running, key) {}",
    "export const operationItem = (running, item) => item",
    "export async function whenRewritesDone(path) {}",
    "export function whileSyncing(item, syncing) {}",
    "export function admitRewrite(intent) {}",
    "export function queueRewriteOperation() {}",
    // the claims of the surfaces, the words of an operation, a function of the module only
    "export function useIndicatorClaim(surface) {}",
    "export function operationBusyOf(operation) {}",
    "function runOperation() {}",
  ].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  const refused = ["1 operations-store", "2 operations-store", "3 operations-store", "4 operations-store", "5 operations-store", "6 operations-store", "7 operations-store"]
  assert.deepEqual(rules("src/stores/example.ts"), refused)
  // a blocker of a module of the page is a rule besides
  assert.deepEqual(rules("src/lib/example.ts"), refused.toSpliced(1, 0, "2 rule-module"))
  assert.deepEqual(rules("src/stores/example.test.ts"), [])
  assert.deepEqual(rules("src/components/Example.tsx"), [])
  assert.deepEqual(findProblems("src/stores/operations.ts", "export const x = 1\n"), [
    { file: "src/stores/operations.ts", line: 1, rule: "operations-store", text: "the registry of the page: the session admits every operation (RepositoryView.operations)" },
  ])
})

test("a read of the start of the main window is refused: a command of its start but Browse, the prefix outside Settings, an open sync", () => {
  const source = [
    "const setup = await commands.checkGitSetup({ repositoryPath })",
    "const prefix = await commands.resolveBranchPrefix({ repositoryPath })",
    'void syncBranches("open")',
    // the user's Browse, the sync the user asks, a command named in a comment or a type
    "const result = await commands.browseRepository()",
    'void syncBranches("manual")',
    "// commands.getSnapshot() at start",
    "type Commands = typeof commands",
  ].join("\n")
  // the registry decides who calls a command (`intent-call`, its own test): none here
  const rules = file => findProblems(file, source, { registry: new Map() }).map(problem => `${problem.line} ${problem.rule}`)
  for (const file of STARTUP_MODULES) {
    assert.deepEqual(rules(file), ["1 startup-read", "2 startup-read", "3 startup-read"])
  }
  // elsewhere a command is no read of the start, the prefix of the Settings window neither; a test asks what it tests
  assert.deepEqual(rules("src/stores/example.ts"), ["2 startup-read", "3 startup-read"])
  assert.deepEqual(rules("src/components/settings/BranchPrefixSetting.tsx"), ["3 startup-read"])
  assert.deepEqual(rules("src/stores/repository.test.ts"), [])
  assert.deepEqual(rules("tests/e2e/example.spec.ts"), [])
})

/** A registry of four commands, by the names the bindings give them (`readRegistry`) */
const REGISTRY = new Map([
  ["execute", { name: "execute", class: "engine" }],
  ["cancelOperation", { name: "cancel_operation", class: "engine" }],
  ["signIn", { name: "sign_in", class: "app" }],
  ["getSnapshot", { name: "get_snapshot", class: "read" }],
])
const CALLERS = { sign_in: "src/lib/sign-in-run.ts" }

test("the engine is called only from the intents, a command of the app only from its module", () => {
  const source = [
    "const executed = await commands.execute(params, events)",
    "void commands.cancelOperation({ op }).catch(() => {})",
    "const result = await commands",
    "  .signIn(request, events)",
    // a read, a name the registry lacks, a command in a comment or a string
    "const view = await commands.getSnapshot({ repositoryPath })",
    "const count = commands.length",
    "// commands.execute(params) in a comment",
    'const words = "commands.execute"',
  ].join("\n")
  const rules = file => findProblems(file, source, { registry: REGISTRY, callers: CALLERS }).map(problem => `${problem.line} ${problem.rule}`)
  assert.deepEqual(rules("src/stores/example.ts"), ["1 intent-call", "2 intent-call", "4 intent-call"])
  assert.deepEqual(rules("src/model/intents.ts"), ["4 intent-call"])
  assert.deepEqual(rules("src/lib/sign-in-run.ts"), ["1 intent-call", "2 intent-call"])
  // a test mocks what it calls; an e2e test calls no binding
  assert.deepEqual(rules("src/stores/example.test.ts"), [])
  assert.deepEqual(rules("tests/e2e/example.spec.ts"), [])
})

test("an entry of the callers of the app commands names a command of the app that its module calls", t => {
  const root = mkdtempSync(join(tmpdir(), "lint-arch-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, "crates/app-api"), { recursive: true })
  mkdirSync(join(root, "src/lib"), { recursive: true })
  const table = [
    { name: "sign_in", class: "app" },
    { name: "sign_out", class: "app" },
    { name: "get_snapshot", class: "read" },
  ]
  writeFileSync(join(root, "crates/app-api/commands.json"), JSON.stringify(table))
  writeFileSync(join(root, "src/lib/sign-in-run.ts"), "await commands.signIn(request, events)\n")
  writeFileSync(join(root, "src/lib/accounts.ts"), "await commands.signOut(account)\n")

  assert.deepEqual(readRegistry(root).get("signOut"), { name: "sign_out", class: "app" })
  assert.deepEqual(lintArchitecture(root, [], { sign_in: "src/lib/sign-in-run.ts", sign_out: "src/lib/accounts.ts" }), [])
  // a second caller, a command no module is named for
  assert.deepEqual(lintArchitecture(root, [], { sign_in: "src/lib/accounts.ts" }), [
    "src/lib/accounts.ts:1: intent-call: await commands.signOut(account)",
    "src/lib/sign-in-run.ts:1: intent-call: await commands.signIn(request, events)",
    "APP_COMMAND_CALLERS: sign_in is not called in src/lib/accounts.ts: name the module that calls it, or remove it",
  ])
  // a stale entry: a read, a command the registry lost
  assert.deepEqual(
    lintArchitecture(root, [], { sign_in: "src/lib/sign-in-run.ts", sign_out: "src/lib/accounts.ts", get_snapshot: "src/lib/accounts.ts", sync: "src/lib/accounts.ts" }),
    [
      "APP_COMMAND_CALLERS: get_snapshot is no app(…) command of crates/app-api/commands.json: remove it",
      "APP_COMMAND_CALLERS: sync is no app(…) command of crates/app-api/commands.json: remove it",
    ],
  )
})

test("every command of the app of this repository has its one module, every entry its command", () => {
  const registry = readRegistry(join(import.meta.dirname, ".."))
  const app = [...registry.values()].filter(command => command.class === "app").map(command => command.name)
  assert.ok(app.includes("sign_in"))
  const byName = (a, b) => a.localeCompare(b)
  assert.deepEqual(Object.keys(APP_COMMAND_CALLERS).toSorted(byName), app.toSorted(byName))
})

test("a presenter reads no store: the caller passes what it selected of the view", () => {
  const source = [
    "const operation = branchSyncStore.getState().operations[op]",
    "const { baselineBranch } = branchSyncStore",
    "  .getState()",
    // a part the caller passed
    "const operation = words.operations[op]",
    // the getters of a store module read the store: of the view, of the facts, of the window
    'import { getRowPart } from "@/model/store"',
    "import {",
    "  getOpenPullRequest,",
    '} from "@/stores/branch-sync"',
    "import { getOperationWords } from '@/stores/indicators'",
    'const actions = getRowPart(name, "actions")',
    // its types read nothing, nor a module beside the stores
    'import type { Branch, BranchSyncState } from "@/stores/branch-sync"',
    'import type { RowParts } from "@/model/store"',
    'import { useShowReviewReadiness } from "@/model/demand"',
    'import { findAction } from "@/presenters/actions"',
  ].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  assert.deepEqual(rules("src/presenters/example.ts"), ["1 presenter-state", "3 presenter-state", "5 presenter-state", "6 presenter-state", "9 presenter-state"])
  assert.deepEqual(rules("src/presenters/example.test.ts"), [])
  assert.deepEqual(rules("src/hooks/useExample.ts"), [])
})

test("an async function reads the selected project before its first await, the operand of the await included", () => {
  const source = [
    "export async function load(name: string): Promise<{ readonly commits: number }> {",
    "  const repositoryPath = getSelectedRepositoryPath()",
    "  const result = await commands.getCommits(repositoryPath, getFullBranchName(name))",
    "  const prefix = getEffectiveBranchPrefix()",
    "  return { commits: vcsRequestFactory.count(result), path: repositoryStore.getState().selected }",
    "}",
    "const run = async () => {",
    "  if (later) {",
    "    await later()",
    "    return",
    "  }",
    "  open(getSelectedProject()?.path)",
    "}",
    "const read = async (key: string) => show(await ask(key), getDiffRepositoryPath())",
    "function now() {",
    "  return getFullStackBranchName(getSelectedProject()?.name ?? '')",
    "}",
  ].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  assert.deepEqual(rules("src/stores/example.ts"), [
    "3 late-project-read",
    "4 late-project-read",
    "5 late-project-read",
    "5 late-project-read",
    "12 late-project-read",
    "14 late-project-read",
  ])
  assert.deepEqual(rules("src/stores/example.test.ts"), [])
  assert.deepEqual(rules("tests/e2e/example.spec.ts"), [])
})

test("the first await of an async function is its own: of a nested function or in a string it is none", () => {
  const source = [
    "async function start(): Promise<void> {",
    "  const later = async () => {",
    "    await wait()",
    "  }",
    '  notify("await the answer")',
    "  const path = getSelectedRepositoryPath()",
    "  await later()",
    "}",
    "const store = {",
    "  async refresh() {",
    "    const path = getSelectedRepositoryPath()",
    "    await read(path)",
    "    return getSelectedProject()",
    "  },",
    "}",
    "export const ask = async function (key: string) {",
    "  await key",
    "  return getFullBranchName(key)",
    "}",
  ].join("\n")
  assert.deepEqual(
    findProblems("src/stores/example.ts", source).map(problem => `${problem.line} ${problem.rule}`),
    ["13 late-project-read", "18 late-project-read"],
  )
})
