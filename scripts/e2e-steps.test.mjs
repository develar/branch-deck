import assert from "node:assert/strict"
import { test } from "node:test"
import { kindsOf, slowSteps, stepKind } from "./e2e-steps.mjs"

test("a kind of step keeps its matcher and drops what it acts on", () => {
  assert.equal(stepKind('expect: Expect "toHaveCount"'), 'expect: Expect "toHaveCount"')
  assert.equal(stepKind('expect: Expect "poll toBe"'), 'expect: Expect "poll toBe"')
  assert.equal(stepKind('pw:api: Press "Escape"'), "pw:api: Press")
  assert.equal(stepKind('pw:api: Wait for event "requestfinished"'), "pw:api: Wait for event")
  assert.equal(stepKind("pw:api: Click"), "pw:api: Click")
})

test("the kinds of the top-level steps sum their time, the costliest first", () => {
  const profiles = [
    {
      title: "a",
      ms: 3000,
      steps: [
        [0, 400, 0, "pw:api: Click", ""],
        [10, 300, 1, "pw:api: Evaluate", ""],
        [500, 900, 0, 'expect: Expect "toHaveCount"', ""],
      ],
    },
    { title: "b", ms: 1000, steps: [[0, 500, 0, "pw:api: Click", ""]] },
  ]
  assert.deepEqual(kindsOf(profiles), [
    { kind: "pw:api: Click", ms: 900, count: 2 },
    { kind: 'expect: Expect "toHaveCount"', ms: 900, count: 1 },
  ])
})

test("the slow steps of a test are the long ones and those after a gap", () => {
  const profile = {
    steps: [
      [0, 100, 0, "fast", "a.ts:1"],
      [100, 500, 0, "slow", "a.ts:2"],
      [150, 400, 1, "inner", "a.ts:3"],
      [800, 50, 0, "after a gap", "a.ts:4"],
    ],
  }
  assert.deepEqual(slowSteps(profile, 300), [
    { at: 100, ms: 500, gap: 0, title: "slow", where: "a.ts:2" },
    { at: 800, ms: 50, gap: 200, title: "after a gap", where: "a.ts:4" },
  ])
})
