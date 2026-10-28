import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { test } from "node:test"
import { findProblems, lintArchitecture } from "./lint-arch.mjs"

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

test("a timer of a store or a hook is a poll, of its test or another module not", () => {
  const source = ["const timer = setTimeout(ask, 300_000)", "const every = window.setInterval(read, 60_000)", "// setTimeout(ask) in a comment", "clearTimeout(timer)"].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  assert.deepEqual(rules("src/stores/example.ts"), ["1 poll-timer", "2 poll-timer"])
  assert.deepEqual(rules("src/hooks/useExample.tsx"), ["1 poll-timer", "2 poll-timer"])
  assert.deepEqual(rules("src/stores/example.test.ts"), [])
  assert.deepEqual(rules("src/lib/example.ts"), [])
})

test("an allowlist entry needs its reason and a finding to hold", t => {
  const root = mkdtempSync(join(tmpdir(), "lint-arch-"))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(join(root, "src"))
  writeFileSync(join(root, "src/a.ts"), 'if (error.message === "x") {}\n')
  writeFileSync(join(root, "src/b.ts"), "export const b = 1\n")

  assert.deepEqual(lintArchitecture(root, []), ['src/a.ts:1: message-control: if (error.message === "x") {}'])
  assert.deepEqual(lintArchitecture(root, [{ file: "src/a.ts", rule: "message-control", reason: "the words of a third-party library, which has no code" }]), [])
  assert.deepEqual(lintArchitecture(root, [{ file: "src/a.ts", rule: "message-control", reason: " " }]), ["ALLOWLIST: src/a.ts (message-control) gives no reason"])
  assert.deepEqual(lintArchitecture(root, [{ file: "src/b.ts", rule: "message-control", reason: "gone" }]), [
    'src/a.ts:1: message-control: if (error.message === "x") {}',
    "ALLOWLIST: src/b.ts (message-control) matches nothing: remove it",
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
    // words of a reason Rust gave, a type, a name of no rule, a function of the module only
    "export function formatParkedBlocker(state) {}",
    "export type LandingDecision = string",
    "export function canvasOf() {}",
    "function rewriteBlocker() {}",
  ].join("\n")
  const rules = file => findProblems(file, source).map(problem => `${problem.line} ${problem.rule}`)
  const refused = ["1 rule-module", "2 rule-module", "3 rule-module", "4 rule-module", "5 rule-module", "6 rule-module"]
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
