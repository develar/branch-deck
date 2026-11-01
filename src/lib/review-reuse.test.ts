// @spec spec/review/review-window.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { planReviewOpen, pushReviewHistory, REVIEW_HISTORY_LIMIT, restoredReview, reviewSubjectName, reviewWindowTitle } from "@/lib/review-reuse"
import type { ShownReview } from "@/lib/review-reuse"
import type { BranchReviewWindowData, CommitsReviewWindowData } from "@/types/sub-window"

const shown: ShownReview = { kind: "branch", repositoryPath: "/repo", chain: ["auth", "login"], wholeStack: false, startedAt: 1 }

function openBranch(branchName: string, chain: string[], focus: BranchReviewFocus, from: ShownReview | null = shown) {
  return planReviewOpen(from, { kind: "branch", repositoryPath: "/repo", branchName, chain, wholeStack: false, focus }, 9)
}

const commits = [
  { id: "a1a1a1a1a1", subject: "first", message: "first", author: "A", authorTime: 0, committerTime: 0 },
  { id: "b2b2b2b2b2", subject: "second", message: "second", author: "A", authorTime: 0, committerTime: 0 },
]
const list: ShownReview = { kind: "commits", repositoryPath: "/repo", listLabel: "Unassigned", ids: ["a1a1a1a1a1", "b2b2b2b2b2"], startedAt: 1 }

const branchData: BranchReviewWindowData = {
  kind: "branch",
  repositoryPath: "/repo",
  baselineBranch: "origin/master",
  chain: ["auth", "login"],
  groups: [],
  focus: { range: "own", filter: { kind: "all" }, commitId: null, path: null },
  openedAt: 1,
  startedAt: 1,
  move: false,
  state: { repositoryPath: "/repo", selected: true, items: {}, menus: {}, createPullRequest: null, wholeStack: null, commitMenus: {}, groups: null },
}
const commitsData: CommitsReviewWindowData = {
  kind: "commits",
  repositoryPath: "/repo",
  commits,
  listLabel: "Unassigned",
  index: 0,
  path: null,
  openedAt: 2,
  startedAt: 2,
  move: false,
}

describe("planReviewOpen", () => {
  it("keeps its review for a plain open of the branch it shows, and starts over for another branch or a place", () => {
    expect(openBranch("login", ["auth", "login"], {})).toEqual({ shown, move: false })
    expect(openBranch("auth", ["auth"], {})).toEqual({ shown: { ...shown, chain: ["auth"], startedAt: 9 }, move: false })
    // the "Review Changes" of the Safe Push popover: the whole stack, the files of a group, a file
    expect(openBranch("login", ["auth", "login"], { range: "all", filter: { kind: "group", name: "Air" }, path: "a.kt" }).shown.startedAt).toBe(9)
    expect(openBranch("login", ["auth", "login"], {}, null).shown.startedAt).toBe(9)
    expect(openBranch("login", ["auth", "login"], {}, { ...shown, wholeStack: true }).shown.startedAt).toBe(9)
  })

  it("moves to a commit of a branch of the chain on screen, and starts over for a commit of another branch", () => {
    // a commit of the parent: the chain on screen stays
    expect(openBranch("auth", ["auth"], { commitId: "a1" })).toEqual({ shown, move: true })
    expect(openBranch("login", ["auth", "login"], { commitId: "l1", path: "a.kt" })).toEqual({ shown, move: true })
    expect(openBranch("ui", ["auth", "login", "ui"], { commitId: "u1" })).toEqual({ shown: { ...shown, chain: ["auth", "login", "ui"], startedAt: 9 }, move: false })
    expect(planReviewOpen(shown, { kind: "branch", repositoryPath: "/other", branchName: "auth", chain: ["auth"], wholeStack: false, focus: { commitId: "a1" } }, 9).move).toBe(
      false,
    )
  })

  it("keeps a kept branch and a whole stack for a plain open of them, moves to a commit of the kept branch", () => {
    const kept = { kind: "unapplied", name: "jane/unapplied/docs" } as const
    const keptShown: ShownReview = { kind: "branch", repositoryPath: "/repo", chain: ["docs"], wholeStack: false, kept, startedAt: 1 }
    const openKept = (focus: BranchReviewFocus) =>
      planReviewOpen(keptShown, { kind: "branch", repositoryPath: "/repo", branchName: "docs", chain: ["docs"], wholeStack: false, kept, focus }, 9)
    expect(openKept({})).toEqual({ shown: keptShown, move: false })
    expect(openKept({ commitId: "d1" })).toEqual({ shown: keptShown, move: true })
    // the branch it was is another subject
    expect(openBranch("docs", ["docs"], {}, keptShown).shown.startedAt).toBe(9)
    const stack: ShownReview = { kind: "branch", repositoryPath: "/repo", chain: ["auth"], wholeStack: true, startedAt: 1 }
    const openStack = (focus: BranchReviewFocus) =>
      planReviewOpen(stack, { kind: "branch", repositoryPath: "/repo", branchName: "auth", chain: ["auth"], wholeStack: true, focus }, 9)
    expect(openStack({})).toEqual({ shown: stack, move: false })
    // with the groups of a code-owner check
    expect(openStack({ groups: [] }).shown.startedAt).toBe(9)
  })

  it("moves to a commit of the list on screen, and starts over for another list, a file or a branch", () => {
    const openList = (listLabel: string | null, ids: string[], path: string | null = null, from: ShownReview = list) =>
      planReviewOpen(from, { kind: "commits", repositoryPath: "/repo", listLabel, ids, path }, 9)
    expect(openList("Unassigned", ["a1a1a1a1a1", "b2b2b2b2b2"])).toEqual({ shown: list, move: true })
    expect(openList("Unassigned", ["a1a1a1a1a1", "b2b2b2b2b2"], "src/a.kt").shown.startedAt).toBe(9)
    expect(openList("Missing", ["a1a1a1a1a1", "b2b2b2b2b2"]).shown).toEqual({ ...list, listLabel: "Missing", startedAt: 9 })
    expect(openList("Unassigned", ["a1a1a1a1a1"]).shown.startedAt).toBe(9)
    expect(openList("Unassigned", ["a1a1a1a1a1", "b2b2b2b2b2"], null, shown).shown.startedAt).toBe(9)
    expect(openBranch("login", ["auth", "login"], { commitId: "l1" }, list).shown.startedAt).toBe(9)
  })
})

describe("the history of the review window", () => {
  it("keeps the reviews it left, the latest last, at most the limit", () => {
    let history = pushReviewHistory([], branchData)
    expect(history).toEqual([branchData])
    for (let index = 0; index < REVIEW_HISTORY_LIMIT + 5; index++) {
      history = pushReviewHistory(history, { ...commitsData, startedAt: 100 + index })
    }
    expect(history).toHaveLength(REVIEW_HISTORY_LIMIT)
    expect(history.at(-1)?.startedAt).toBe(100 + REVIEW_HISTORY_LIMIT + 4)
  })

  it("starts a review of the history over at the place it was left", () => {
    const place = { range: { from: "auth", to: "login" }, filter: { kind: "all" }, commitId: "l1", path: "src/a.kt" } as const
    expect(restoredReview(branchData, { kind: "branch", focus: place }, 50)).toEqual({ ...branchData, focus: place, openedAt: 50, startedAt: 50, move: false })
    expect(restoredReview(branchData, undefined, 50)).toEqual({ ...branchData, openedAt: 50, startedAt: 50 })
    expect(restoredReview(commitsData, { kind: "commits", index: 1, path: "src/b.kt" }, 50)).toEqual({ ...commitsData, index: 1, path: "src/b.kt", openedAt: 50, startedAt: 50 })
  })

  it("names the review it goes back to as its title does", () => {
    expect(reviewSubjectName(branchData)).toBe("login")
    expect(reviewWindowTitle({ ...branchData, focus: { ...branchData.focus, range: { from: "auth", to: "auth" }, commitId: "a1b2c3d4e5f6" } })).toBe("Review auth · a1b2c3d4")
    expect(reviewSubjectName({ ...branchData, chain: ["air"], wholeStack: true })).toBe("the whole air stack")
    expect(reviewSubjectName(commitsData)).toBe("a1a1a1a1 first")
    expect(reviewWindowTitle({ ...commitsData, index: 1 })).toBe("Review b2b2b2b2 second")
  })
})
