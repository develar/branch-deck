import { describe, expect, it } from "vitest"
import type { StackCommit, StackIndependence } from "@/lib/bindings"
import type { StackStart } from "@/lib/patronus-stack"
import { describeStackStart } from "@/lib/patronus-stack"

function start(patch: Partial<StackStart>) {
  return describeStackStart({ branchName: "oauth", mode: "rebaseAutosquash", target: "master", ancestors: [], ancestorCommitCount: 0, children: [], pullRequest: null, childrenHavePullRequests: false, independence: null, ...patch })
}

const CHILD: StackCommit = { id: "c1", subject: "Cache the parsed icon", branch: "perf-part-2" }
const PARSED: StackCommit = { id: "p1", subject: "Parse the icon off the EDT", branch: "IJPL-257337" }
const ROOT_FIRST = "perf-part-2 is built on IJPL-257337, which isn't in master yet. Safe Push IJPL-257337 first, then perf-part-2: after a merge, the next sync moves the branches built on it onto master."

describe("describeStackStart", () => {
  it("leads a Safe Push of a stacked branch root first, and says that its own start lands the ancestors", () => {
    expect(start({ ancestors: ["login"] })).toEqual({
      lead: { text: "oauth is built on login, which isn't in master yet. Safe Push login first, then oauth: after a merge, the next sync moves the branches built on it onto master.", root: "login", action: { kind: "safePushRoot" } },
      note: "Or start oauth now: the Safe Push lands login with it.",
    })
    expect(start({ ancestors: ["auth", "login"], pullRequest: 8 })).toEqual({
      lead: { text: "oauth is built on auth and login, which aren't in master yet. Safe Push auth first, then the next: after a merge, the next sync moves the branches built on it onto master.", root: "auth", action: { kind: "safePushRoot" } },
      note: "Or start oauth now: the Safe Push lands auth and login with it. The approvals of pull request #8 don't count: it compares oauth with login, and Patronus merges a pull request into its base.",
    })
  })

  it("counts the branches and the commits a Safe Push lands with the branch, from three ancestors on", () => {
    const ancestors = ["vm-lane-daemon", "air-fake-agent", "air-lane-document", "IJAI-3885"]
    expect(start({ branchName: "IJAI-4074", ancestors, ancestorCommitCount: 40 }).note)
      .toBe("Or start IJAI-4074 now: the Safe Push lands 4 branches · 40 commits with it: vm-lane-daemon, air-fake-agent, air-lane-document and 1 more.")
    expect(start({ branchName: "IJAI-4074", mode: "dryRun", ancestors, ancestorCommitCount: 40 }).note)
      .toBe("It checks 4 branches · 40 commits too: vm-lane-daemon, air-fake-agent, air-lane-document and 1 more, which IJAI-4074 is built on.")
  })

  it("says that a dry run checks the ancestors' commits too", () => {
    expect(start({ mode: "dryRun", ancestors: ["auth", "login"] })).toEqual({ lead: null, note: "It checks the commits of auth and login too: oauth is built on them." })
    expect(start({ mode: "dryRun", children: ["device"] })).toEqual({ lead: null, note: null })
  })

  it("says what the merge of a parent does to the branches built on it", () => {
    expect(start({ branchName: "auth", children: ["login"] }).note).toBe("login is built on auth: after the merge, the next sync moves them onto master.")
    expect(start({ branchName: "auth", children: ["login", "oauth"], childrenHavePullRequests: true }).note)
      .toBe("login and oauth are built on auth: after the merge, the next sync moves them onto master; their pull requests are retargeted to master when they are pushed.")
    expect(start({ branchName: "auth" })).toEqual({ lead: null, note: null })
  })

  describe("with the trial build of the branch alone", () => {
    const stacked = { branchName: "perf-part-2", ancestors: ["IJPL-257337"] }

    it("offers the unstack first when the branch applies on the baseline without its parent", () => {
      expect(start({ ...stacked, independence: { kind: "builds", parent: "IJPL-257337" } }).lead).toEqual({
        text: "perf-part-2 applies on the baseline without IJPL-257337. Compile dependencies are not checked.",
        root: "IJPL-257337",
        action: { kind: "unstack", parent: "IJPL-257337" },
      })
    })

    it("offers to move the commits it needs when no other branch needs them", () => {
      const independence: StackIndependence = { kind: "needs", parent: "IJPL-257337", childCommit: CHILD, files: ["AppUIUtil.kt"], parentCommits: [PARSED], movable: true }
      expect(start({ ...stacked, independence }).lead).toEqual({
        text: "\"Cache the parsed icon\" changes lines that \"Parse the icon off the EDT\" of IJPL-257337 adds. perf-part-2 may build without IJPL-257337 once it is here.",
        root: "IJPL-257337",
        action: { kind: "move", commits: [PARSED], label: "Move 1 Commit Here" },
      })
      const two = { ...independence, parentCommits: [PARSED, { ...PARSED, id: "p0", subject: "Load icons" }] }
      expect(start({ ...stacked, independence: two }).lead?.action).toMatchObject({ kind: "move", label: "Move 2 Commits Here" })
      expect(start({ ...stacked, independence: two }).lead?.text).toMatch(/^"Cache the parsed icon" changes lines that 2 commits of IJPL-257337 add\./)
    })

    it("leads root first and names the needed commit when a move would make another branch need this one", () => {
      const independence: StackIndependence = { kind: "needs", parent: "IJPL-257337", childCommit: CHILD, files: ["AppUIUtil.kt"], parentCommits: [PARSED], movable: false }
      expect(start({ ...stacked, independence }).lead).toEqual({
        text: `${ROOT_FIRST} "Cache the parsed icon" changes lines that "Parse the icon off the EDT" of IJPL-257337 adds.`,
        root: "IJPL-257337",
        action: { kind: "safePushRoot" },
      })
      // no commit of the parent explains the conflict: its files
      expect(start({ ...stacked, independence: { ...independence, parentCommits: [] } }).lead?.text)
        .toBe(`${ROOT_FIRST} Without IJPL-257337, "Cache the parsed icon" conflicts in AppUIUtil.kt.`)
    })

    it("leads root first while the check runs or after it failed", () => {
      expect(start({ ...stacked, independence: null }).lead).toEqual({ text: ROOT_FIRST, root: "IJPL-257337", action: { kind: "safePushRoot" } })
    })
  })
})
