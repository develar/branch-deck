// @spec spec/repository/landed-commits.spec.md
import { describe, expect, it } from "vitest"
import type { AppliedUpdate, LandedBranch, LandedCommit, UpdateCheck } from "@/lib/bindings"
import {
  appliedUpdateToast,
  describeLanding,
  formatStayLocal,
  fullyLandedNames,
  isFullyLanded,
  landedBranchDescription,
  landedChipLabel,
  landedCountLabel,
  mergeLandedReport,
  sharedCommitter,
  shortLanding,
} from "@/lib/landed"

function commit(id: string, upstream: string | null = `up-${id}`, committer = "Patronus"): LandedCommit {
  return { id, subject: `Subject ${id}`, upstream: upstream === null ? null : { id: upstream, author: "A", committer, time: 100 } }
}

function branch(name: string | null, landed: number, remaining = 0, squashed = false): LandedBranch {
  const commits = Array.from({ length: landed }, (_, i) => commit(`${name ?? "u"}${i}`, squashed ? null : undefined))
  return { name, commits, remaining, landedAt: 1000 }
}

const check: UpdateCheck = { branch: "master", upstream: "origin/master", head: "h", upstreamHead: "u", base: "b", state: { status: "clean", kind: "rebase" }, fetchedAt: null }
const rebase: AppliedUpdate = { kind: "rebase", newHead: "n", replayed: 116, droppedAlreadyUpstream: 5 }

describe("describeLanding", () => {
  it("names the user's branches, a part as a count of the branch", () => {
    expect(describeLanding([])).toBeNull()
    expect(describeLanding([branch("wsm", 5)])).toBe("brings in your wsm")
    expect(describeLanding([branch("wsm", 5), branch("IJPL-257337", 3, 9)])).toBe("brings in your wsm and 3 commits of IJPL-257337")
    expect(describeLanding([branch(null, 2)])).toBe("brings in your 2 commits without a branch")
    expect(describeLanding(["a", "b", "c", "d", "e"].map(name => branch(name, 1)))).toBe("brings in your a, b, c and 2 more")
  })

  it("says it in a few words for the footer", () => {
    expect(shortLanding([])).toBeNull()
    expect(shortLanding([branch("wsm", 5)])).toBe("your wsm landed")
    expect(shortLanding([branch("wsm", 40, 12)])).toBe("40 of your commits landed")
    expect(shortLanding([branch(null, 1)])).toBe("1 of your commits landed")
    expect(shortLanding([branch("a", 1), branch("b", 1)])).toBe("your 2 branches landed")
  })
})

describe("landedCountLabel", () => {
  it("gives each number its verb", () => {
    expect(formatStayLocal(1)).toBe("1 stays local")
    expect(formatStayLocal(12)).toBe("12 stay local")
    expect(landedCountLabel(branch("wsm", 1))).toBe("1 landed")
    expect(landedCountLabel(branch("wsm", 40, 12))).toBe("40 landed · 12 stay local")
    expect(landedCountLabel(branch("wsm", 1, 1))).toBe("1 landed · 1 stays local")
    expect(landedCountLabel(branch("wsm", 3, 0, true))).toBe("3 landed · squashed")
  })

  it("says it in fewer words on the chip, \"Landed\" when the baseline has all of the branch", () => {
    expect(landedChipLabel(branch("wsm", 40, 12), true)).toBe("40 landed · 12 local")
    expect(landedChipLabel(branch("wsm", 40, 12), false)).toBe("40 landed · 12 local")
    expect(landedChipLabel(branch("wsm", 5), true)).toBe("Landed")
    expect(landedChipLabel(branch("wsm", 3, 0, true), false)).toBe("3 landed")
  })

  it("has nothing to push for a branch the fetched baseline has all of, before the update", () => {
    expect(isFullyLanded(branch("wsm", 5), true)).toBe(true)
    expect(isFullyLanded(branch("wsm", 40, 12), true)).toBe(false)
    // after the update a branch still in the table has new commits
    expect(isFullyLanded(branch("wsm", 5), false)).toBe(false)
    expect([...fullyLandedNames([branch("wsm", 5), branch("IJPL-257337", 3, 9), branch(null, 2)])]).toEqual(["wsm"])
    expect(landedBranchDescription("origin/master")).toBe("Already in origin/master. Update to archive the branch.")
  })

  it("names the committer once when every branch has the same one", () => {
    expect(sharedCommitter([branch("a", 1), branch("b", 2)])).toBe("Patronus")
    const other: LandedBranch = { name: "c", commits: [commit("c0", "up-c0", "Space Team")], remaining: 0, landedAt: 1000 }
    expect(sharedCommitter([branch("a", 1), other])).toBeUndefined()
    // a squashed branch has no committer to share
    expect(sharedCommitter([branch("a", 1), branch("s", 1, 0, true)])).toBeUndefined()
    expect(sharedCommitter([])).toBeUndefined()
  })
})

describe("appliedUpdateToast", () => {
  it("leads with what landed, then how the branch was updated", () => {
    expect(appliedUpdateToast(check, rebase, [])).toEqual({ title: "Updated master: rebased 116 commits onto origin/master" })
    expect(appliedUpdateToast(check, rebase, [branch("wsm", 5)])).toEqual({ title: "wsm landed in origin/master", description: "Updated master: rebased 116 commits" })
    expect(appliedUpdateToast(check, rebase, [branch("wsm", 5), branch("IJPL-257337", 1), branch("IJAI-3828", 2)])).toEqual({
      title: "3 branches landed in origin/master",
      description: "wsm, IJPL-257337 and IJAI-3828 · rebased 116 commits",
    })
    expect(appliedUpdateToast(check, rebase, [branch("IJPL-257337", 40, 12)])).toEqual({
      title: "40 commits of IJPL-257337 landed · 12 stay local",
      description: "Updated master: rebased 116 commits",
    })
    expect(appliedUpdateToast(check, rebase, [branch("IJPL-257337", 1, 1)]).title).toBe("1 commit of IJPL-257337 landed · 1 stays local")
    expect(appliedUpdateToast(check, rebase, [branch("wsm", 2, 0, true)]).title).toBe("wsm landed in origin/master, squashed")
    expect(appliedUpdateToast(check, rebase, [branch(null, 2)]).title).toBe("2 of your commits landed in origin/master")
    expect(appliedUpdateToast(check, rebase, [branch(null, 40, 12)]).title).toBe("40 of your commits landed · 12 stay local")
  })

  it("keeps the fast-forward wording", () => {
    const fastForward: AppliedUpdate = { kind: "fastForward", newHead: "n", replayed: 0, droppedAlreadyUpstream: 0 }
    expect(appliedUpdateToast(check, fastForward, []).title).toBe("Updated master: fast-forwarded to origin/master")
  })
})

describe("mergeLandedReport", () => {
  it("adds new commits to a known branch and takes its new count of local commits", () => {
    const known = branch("auth", 1, 2)
    const found: LandedBranch = { name: "auth", commits: [commit("auth0"), commit("new")], remaining: 1, landedAt: 2000 }
    const merged = mergeLandedReport([known], [found, branch("ui", 1)])
    expect(merged.map(it => [it.name, it.commits.map(c => c.id), it.remaining, it.landedAt])).toEqual([
      ["auth", ["auth0", "new"], 1, 2000],
      ["ui", ["ui0"], 0, 1000],
    ])
    expect(known.commits).toHaveLength(1)
  })
})
