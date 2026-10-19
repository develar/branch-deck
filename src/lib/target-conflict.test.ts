import { describe, expect, it } from "vite-plus/test"
import type { TargetCheck, TargetConflict } from "@/lib/bindings"
import { describeTargetCheck } from "@/lib/target-conflict"

const CONFLICT: TargetConflict = {
  targetTip: "faebba53ecd77000",
  commit: { id: "c0b1e5a5e90c2bf6", author: "Me", authorTime: 1, subject: "Add the composer kind" },
  files: ["AgentPromptPalettePopup.kt"],
  fileCount: 1,
  targetCommits: [{ id: "6fddca13ab01cd02", author: "Alice", authorTime: 2, subject: "IJAI-4233 Change the palette popup" }],
}

function check(patch: Partial<TargetCheck>): TargetCheck {
  return { baseline: "origin/master", fetchError: null, outcome: { kind: "clean" }, ...patch }
}

describe("describeTargetCheck", () => {
  it("refuses a conflict with the commit as the row shows it, its files, the target commits that change them, and Update Project", () => {
    const decision = describeTargetCheck(check({ outcome: { kind: "conflict", ...CONFLICT } }), "composer-kind", copy => (copy === CONFLICT.commit.id ? "e90c2bf606405" : copy))
    expect(decision).toEqual({
      kind: "refuse",
      reason:
        "Patronus can't rebase composer-kind onto origin/master: e90c2bf6 (Add the composer kind) conflicts in AgentPromptPalettePopup.kt, which origin/master changed after the base of composer-kind: 6fddca13 (Alice: IJAI-4233 Change the palette popup). Update Project resolves it locally; then start again.",
    })
  })

  it("starts a clean one without a word, and says when the check used the last fetched target or did not run", () => {
    expect(describeTargetCheck(check({}), "auth")).toEqual({ kind: "start", note: null })
    expect(describeTargetCheck(check({ fetchError: "Failed to fetch origin/master: timeout" }), "auth")).toEqual({
      kind: "start",
      note: "The fetch of origin/master failed, so the check used the last fetched origin/master: Failed to fetch origin/master: timeout",
    })
    expect(describeTargetCheck(check({ outcome: { kind: "notRun", error: "git replay failed" } }), "the whole auth stack")).toEqual({
      kind: "start",
      note: "Not checked whether Patronus can rebase the whole auth stack onto origin/master: git replay failed",
    })
  })

  it("names the files beyond the first ones by their count, and the target without commits that change them", () => {
    const decision = describeTargetCheck(
      check({ fetchError: "offline", outcome: { kind: "conflict", ...CONFLICT, files: ["a.kt", "b.kt"], fileCount: 5, targetCommits: [] } }),
      "auth",
    )
    expect(decision).toEqual({
      kind: "refuse",
      reason:
        "Patronus can't rebase auth onto origin/master: c0b1e5a5 (Add the composer kind) conflicts in a.kt, b.kt and 3 more files, which origin/master changed after the base of auth. Update Project resolves it locally; then start again. The fetch of origin/master failed, so the check used the last fetched origin/master: offline.",
    })
  })
})
