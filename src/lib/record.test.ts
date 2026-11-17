// @spec openspec/specs/branch/branch-row/spec.md
import { describe, expect, it } from "vite-plus/test"
import { sameJson } from "@/lib/record"

describe("sameJson", () => {
  it("answers whether two JSON values say the same, in new objects too", () => {
    const status = { kind: "push", mark: { kind: "blocked", by: "auth" }, facts: ["conflict", "parked"] }
    // the part a patch replaced: the same words in new objects
    expect(sameJson(status, JSON.parse(JSON.stringify(status)))).toBe(true)
    expect(sameJson(null, null)).toBe(true)
    expect(sameJson("origin/master", "origin/master")).toBe(true)
    expect(sameJson(Number.NaN, Number.NaN)).toBe(true)
    expect(sameJson(undefined, undefined)).toBe(true)
  })

  it("tells a value of another field, element, length, key or kind", () => {
    const status = { kind: "push", mark: { kind: "blocked", by: "auth" }, facts: ["conflict"] }
    expect(sameJson(status, { ...status, mark: { kind: "blocked", by: "login" } })).toBe(false)
    expect(sameJson(status, { ...status, facts: ["parked"] })).toBe(false)
    expect(sameJson(status, { ...status, facts: ["conflict", "parked"] })).toBe(false)
    expect(sameJson({ a: 1 }, { b: 1 })).toBe(false)
    expect(sameJson({ a: undefined }, { b: undefined })).toBe(false)
    expect(sameJson({ a: 1 }, { a: 1, b: 2 })).toBe(false)
    expect(sameJson([1], { 0: 1 })).toBe(false)
    expect(sameJson(null, {})).toBe(false)
    expect(sameJson(0, "0")).toBe(false)
    expect(sameJson(null, undefined)).toBe(false)
  })
})
