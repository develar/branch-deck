import { describe, expect, it } from "vitest"
import type { DroppedCommit } from "@/lib/bindings"
import { buildRemoteMovedTexts, formatAuthors } from "@/lib/remote-status"
import { testRemoteStatus } from "@/stores/test-branch"

function dropped(...authors: string[]): DroppedCommit[] {
  return authors.map((author, index) => ({ id: `c${index}`, subject: `Fix ${index}`, author, authorTime: 0 }))
}

describe("formatAuthors", () => {
  it("names the distinct authors in the order of the commits", () => {
    expect(formatAuthors([])).toBe("")
    expect(formatAuthors(dropped("Ann", "Ann"))).toBe("Ann")
    expect(formatAuthors(dropped("Ann", "Bob", "Ann"))).toBe("Ann and Bob")
    expect(formatAuthors(dropped("Ann", "Bob", "Cy"))).toBe("Ann, Bob and Cy")
    expect(formatAuthors(dropped("Ann", "Bob", "Cy", "Dee"))).toBe("Ann, Bob and 2 others")
  })
})

describe("buildRemoteMovedTexts", () => {
  it("says how many commits the remote has that the branch lacks, and by whom; the action is a force push", () => {
    const remote = testRemoteStatus({ commitsBehind: 12, droppedCommits: dropped("Ann", "Bob"), fastForward: false, remoteTip: "c0" })
    expect(buildRemoteMovedTexts("login", remote)).toEqual({
      title: "login changed on the remote",
      description: "12 commits you don't have (by Ann and Bob). Nothing was pushed.",
      action: "Force Push",
    })
    expect(buildRemoteMovedTexts("login", testRemoteStatus({ commitsBehind: 1, droppedCommits: dropped("Ann") })).description)
      .toBe("1 commit you don't have (by Ann). Nothing was pushed.")
  })

  it("offers the push the button would show when the remote lacks nothing of the branch", () => {
    expect(buildRemoteMovedTexts("login", testRemoteStatus({ commitsAhead: 2, myCommitsAhead: 2 }))).toEqual({
      title: "login changed on the remote",
      description: "The remote has no commits the branch lacks. Nothing was pushed.",
      action: "Push",
    })
    expect(buildRemoteMovedTexts("login", testRemoteStatus())).toEqual({
      title: "login changed on the remote",
      description: "The remote has the branch's commits now. Nothing was pushed.",
      action: null,
    })
  })

  it("offers to create a branch deleted on the remote", () => {
    expect(buildRemoteMovedTexts("login", testRemoteStatus({ exists: false, commitsAhead: 1 }))).toEqual({
      title: "login was deleted on the remote",
      description: "Nothing was pushed.",
      action: "Create Remote",
    })
  })
})
