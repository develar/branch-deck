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
