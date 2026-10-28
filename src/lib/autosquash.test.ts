// @spec spec/commit/fixups.spec.md
import { describe, expect, it } from "vite-plus/test"
import { describeFixupTarget, formatSquashFixupsLabel, getFixupTarget, getListFixups } from "@/lib/autosquash"
import type { AutosquashPlan, PlannedGroup } from "@/lib/bindings"
import { testSubject } from "@/test-support/subject-parts"

const login: PlannedGroup = {
  target: "t1",
  targetSubject: "(auth) Add login",
  targetParts: testSubject("Add login", { prefix: "auth" }),
  branch: "auth",
  folded: [
    { commitId: "f1", subject: "fixup! (auth) Add login", subjectParts: testSubject("Add login", { autosquash: "fixup! ", prefix: "auth" }), kind: "Fixup" },
    { commitId: "f2", subject: "squash! Add login", subjectParts: testSubject("Add login", { autosquash: "squash! " }), kind: "Squash" },
  ],
}
const build: PlannedGroup = {
  target: "t2",
  targetSubject: "Add build script",
  targetParts: testSubject("Add build script"),
  branch: null,
  folded: [{ commitId: "f3", subject: "fixup! Add build script", subjectParts: testSubject("Add build script", { autosquash: "fixup! " }), kind: "Fixup" }],
}
const plan: AutosquashPlan = {
  head: "head",
  groups: [login, build],
  orphans: [{ commitId: "o1", subject: "fixup! (auth) Gone", branch: "auth" }],
}

describe("getListFixups", () => {
  it("takes the groups by the branch of their target and the orphans by their own branch", () => {
    expect(getListFixups(plan, "auth")).toEqual({ groups: [login], orphans: [plan.orphans[0]], count: 2 })
    expect(getListFixups(plan, null)).toEqual({ groups: [build], orphans: [], count: 1 })
    expect(getListFixups(plan, "docs").count).toBe(0)
    expect(getListFixups(null, "auth").count).toBe(0)
  })
})

describe("getFixupTarget", () => {
  it("names the target of a fixup, or none for an orphan", () => {
    expect(describeFixupTarget(getFixupTarget(plan, "f2"))).toBe("Squashes into “Add login”")
    expect(describeFixupTarget(getFixupTarget(plan, "o1"))).toBe("No target in this range")
    expect(describeFixupTarget(getFixupTarget(plan, "t1"))).toBeNull()
    expect(describeFixupTarget(getFixupTarget(null, "f1"))).toBeNull()
  })
})

describe("formatSquashFixupsLabel", () => {
  it("names the squash of the fixups alike on the button and in the branch menu, which open the confirmation", () => {
    expect(formatSquashFixupsLabel(1)).toBe("Squash 1 Fixup…")
    expect(formatSquashFixupsLabel(3)).toBe("Squash 3 Fixups…")
  })
})
