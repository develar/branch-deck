import { describe, expect, it } from "vitest"
import type { DroppedCommit } from "@/lib/bindings"
import { buildRemoteMovedTexts, buildRemoteStatusTexts, formatAuthors, formatForcePushLine, formatForcePushPart } from "@/lib/remote-status"
import { testRemoteStatus } from "@/stores/test-branch"

function dropped(...authors: string[]): DroppedCommit[] {
  return authors.map((author, index) => ({ id: `c${index}`, subject: `Fix ${index}`, author, authorTime: 0 }))
}

describe("buildRemoteStatusTexts", () => {
  it("repeats the count in parentheses only when some commits to push are others'", () => {
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 3, myCommitsAhead: 3 }))).toEqual({ text: "3↑", tooltip: "3 commits to push." })
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1 })).tooltip).toBe("1 commit to push.")
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 3, myCommitsAhead: 2 }))).toEqual({ text: "2 (3)↑", tooltip: "2 of 3 commits to push are yours." })
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 3, myCommitsAhead: 0 }))).toEqual({ text: "0 (3)↑", tooltip: "3 commits to push, none of them yours." })
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 0 })).tooltip).toBe("1 commit to push, not yours.")
  })

  it("says what the remote has that the branch lacks, and by whom", () => {
    const diverged = testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsBehind: 2, fastForward: false, droppedCommits: dropped("Ann", "Bob") })
    expect(buildRemoteStatusTexts(diverged)).toEqual({ text: "1↑ · 2↓", tooltip: "1 commit to push.\nThe remote has 2 commits your branch doesn't (by Ann and Bob)." })
    expect(buildRemoteStatusTexts({ ...diverged, commitsAhead: 0, myCommitsAhead: 0 })).toEqual({ text: "2↓", tooltip: "The remote has 2 commits your branch doesn't (by Ann and Bob)." })
    expect(buildRemoteStatusTexts({ ...diverged, droppedCommits: [] }).tooltip).toBe("1 commit to push.\nThe remote has 2 commits your branch doesn't.")
  })

  it("says that the remote has older versions of commits amended here or of the parent's, not commits the branch lacks", () => {
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, fastForward: false }))).toEqual({
      text: "1↑ · 1 amended",
      tooltip: "1 commit to push.\nThe remote has an older version of 1 commit amended here.",
    })
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 2, myCommitsAhead: 2, commitsAmended: 2, fastForward: false })).tooltip)
      .toBe("2 commits to push.\nThe remote has older versions of 2 commits amended here.")
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsAhead: 4, myCommitsAhead: 4, commitsRestacked: 2, fastForward: false }), "air-lane-document")).toEqual({
      text: "4↑ · restacked",
      tooltip: "4 commits to push.\nRebuilt on the new air-lane-document: the remote has older versions of 2 of its commits.",
    })
    expect(buildRemoteStatusTexts(testRemoteStatus({ commitsRestacked: 1, fastForward: false }), "auth")).toEqual({
      text: "restacked",
      tooltip: "Rebuilt on the new auth: the remote has an older version of 1 of its commits.",
    })
    // someone else's commit is still named, after the older versions
    const both = testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, commitsAmended: 1, commitsBehind: 1, fastForward: false, droppedCommits: dropped("Ann") })
    expect(buildRemoteStatusTexts(both)).toEqual({
      text: "1↑ · 1 amended · 1↓",
      tooltip: "1 commit to push.\nThe remote has an older version of 1 commit amended here.\nThe remote has 1 commit your branch doesn't (by Ann).",
    })
  })

  it("says that everything is pushed in one sentence, and when it was pushed", () => {
    expect(buildRemoteStatusTexts(testRemoteStatus())).toEqual({ text: "up to date", tooltip: "Everything is pushed." })
    expect(buildRemoteStatusTexts(testRemoteStatus({ fastForward: false })).tooltip)
      .toBe("Everything is pushed.\nThe branch was only rebased onto the baseline: the remote has its commits.")
    expect(buildRemoteStatusTexts(testRemoteStatus({ lastPushTime: 1704139200 })).tooltip).toMatch(/^Everything is pushed\.\n\nPushed \S/)
  })

  it("says that the push creates a branch not on the remote", () => {
    expect(buildRemoteStatusTexts(testRemoteStatus({ exists: false, commitsAhead: 2, myCommitsAhead: 2 })))
      .toEqual({ text: "not pushed", tooltip: "2 commits to push.\nThe branch isn't on the remote yet: the push creates it." })
    expect(buildRemoteStatusTexts(testRemoteStatus({ exists: false })).tooltip).toBe("The branch isn't on the remote yet: the push creates it.")
  })
})

describe("formatForcePushLine", () => {
  it("names what a force push replaces when it drops nothing", () => {
    expect(formatForcePushLine(testRemoteStatus({ commitsAhead: 1, commitsAmended: 1, fastForward: false }))).toBe("Force push replaces 1 commit amended here.")
    expect(formatForcePushLine(testRemoteStatus({ commitsRestacked: 2, fastForward: false }), "air-lane-document"))
      .toBe("Force push replaces the older versions of air-lane-document's commits.")
    expect(formatForcePushLine(testRemoteStatus({ commitsAmended: 2, commitsRestacked: 1, fastForward: false }), "auth"))
      .toBe("Force push replaces 2 commits amended here and the older versions of auth's commits.")
    // a rebased branch, and one whose push drops remote commits
    expect(formatForcePushLine(testRemoteStatus({ commitsAhead: 1, fastForward: false }))).toBe("Force push replaces the remote branch.")
    expect(formatForcePushLine(testRemoteStatus({ commitsAmended: 1, commitsBehind: 1, fastForward: false }))).toBe("Force push replaces the remote branch.")
  })

  it("names it in the line of a stack push in the same words, or the remote commits the push drops", () => {
    expect(formatForcePushPart(testRemoteStatus({ commitsAhead: 1, commitsAmended: 1, fastForward: false }))).toBe("force push, replaces 1 commit amended here")
    expect(formatForcePushPart(testRemoteStatus({ commitsRestacked: 2, fastForward: false }), "auth")).toBe("force push, replaces the older versions of auth's commits")
    expect(formatForcePushPart(testRemoteStatus({ commitsAhead: 1, commitsAmended: 1, commitsBehind: 2, fastForward: false }))).toBe("force push, drops 2 remote commits")
    expect(formatForcePushPart(testRemoteStatus({ commitsAhead: 1, fastForward: false }))).toBe("force push")
  })

  it("starts the description of a force push with it, and doesn't repeat the older versions it names", () => {
    const restacked = testRemoteStatus({ commitsAhead: 4, myCommitsAhead: 4, commitsRestacked: 2, fastForward: false })
    expect(buildRemoteStatusTexts(restacked, "auth", { forcePush: true }).tooltip).toBe("Force push replaces the older versions of auth's commits.\n4 commits to push.")
    // beside a drop the push replaces the remote branch, and every line says what it has
    expect(buildRemoteStatusTexts({ ...restacked, commitsBehind: 1, droppedCommits: dropped("Ann") }, "auth", { forcePush: true }).tooltip).toBe(
      "Force push replaces the remote branch.\n4 commits to push.\nRebuilt on the new auth: the remote has older versions of 2 of its commits.\nThe remote has 1 commit your branch doesn't (by Ann).",
    )
  })
})

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
    expect(buildRemoteMovedTexts("login", testRemoteStatus({ commitsAhead: 1, commitsAmended: 1, fastForward: false }))).toEqual({
      title: "login changed on the remote",
      description: "The remote has older versions of the branch's commits. Nothing was pushed.",
      action: "Force Push",
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
