import { describe, expect, it } from "vite-plus/test"
import type { PatronusRobot } from "@/lib/bindings"
import { describeLandedCloses } from "@/presenters/news"

function robot(branchName: string | null, stackRoot: string | null = null): PatronusRobot {
  return {
    id: "r1",
    name: "IJPL-1 fix",
    status: "success",
    pushMode: "rebaseAutosquash",
    branchName,
    stackRoot,
    sourceRevision: "tip",
    targetBranch: "master",
    startTime: 1,
    finishTime: 2,
    cancellationReason: null,
    url: "https://patronus/robot/r1",
    pullRequest: null,
    profile: null,
  }
}

describe("describeLandedCloses", () => {
  it("says which pull requests the merge closes: of the branches, or how many the whole stack landed", () => {
    const pullRequest = (branch: string, number: number) => ({ branch, number })
    expect(describeLandedCloses(robot("oauth"), [pullRequest("auth", 5)])).toBe("closes the pull request of auth: it landed with oauth")
    expect(describeLandedCloses(robot("oauth"), [pullRequest("auth", 5), pullRequest("login", 6)])).toBe("closes the pull requests of auth and login: they landed with oauth")
    const six = ["a", "b", "c", "d", "e", "f"].map((branch, index) => pullRequest(branch, index))
    expect(describeLandedCloses(robot(null, "air"), six)).toBe("closes the pull requests of 6 branches: they landed with the whole stack")
    expect(describeLandedCloses(robot("oauth"), [])).toBeNull()
  })
})
