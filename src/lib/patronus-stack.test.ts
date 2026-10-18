import { describe, expect, it } from "vite-plus/test"
import type { StackCommit, StackIndependence } from "@/lib/bindings"
import type { StackLandingInput } from "@/lib/patronus-stack"
import { describeStackLanding } from "@/lib/patronus-stack"

function landing(patch: Partial<StackLandingInput>) {
  return describeStackLanding({
    branchName: "oauth",
    mode: "rebaseAutosquash",
    target: "master",
    ancestors: [],
    commitCount: 1,
    children: [],
    independence: null,
    ...patch,
  })
}

const CHILD: StackCommit = { id: "c1", subject: "Cache the parsed icon", branch: "perf-part-2" }
const PARSED: StackCommit = { id: "p1", subject: "Parse the icon off the EDT", branch: "IJPL-257337" }

describe("describeStackLanding", () => {
  it("lands a stacked branch with what it is built on, in one robot, counted", () => {
    expect(landing({ ancestors: ["auth", "login"], commitCount: 12 })).toEqual({
      lands: "Lands oauth with auth and login, which it is built on: 3 branches · 12 commits into master.",
      children: null,
      alone: null,
    })
    expect(landing({ ancestors: ["login"], commitCount: 2 }).lands).toBe("Lands oauth with login, which it is built on: 2 branches · 2 commits into master.")
    // a lone branch lands alone: nothing to say
    expect(landing({})).toEqual({ lands: null, children: null, alone: null })
  })

  it("names three ancestors and counts the rest, the size of what lands first", () => {
    const ancestors = ["vm-lane-daemon", "air-fake-agent", "air-lane-document", "IJAI-3885"]
    expect(landing({ branchName: "IJAI-4074", ancestors, commitCount: 42 }).lands).toBe(
      "Lands IJAI-4074 with vm-lane-daemon, air-fake-agent, air-lane-document and 1 more, which it is built on: 5 branches · 42 commits into master.",
    )
  })

  it("says that a dry run checks the commits of the ancestors too, and nothing of the children", () => {
    expect(landing({ mode: "dryRun", ancestors: ["auth", "login"], commitCount: 3, children: ["device"] })).toEqual({
      lands: "Checks oauth with auth and login, which it is built on: 3 branches · 3 commits.",
      children: null,
      alone: null,
    })
    expect(landing({ mode: "dryRun", children: ["device"] })).toEqual({ lands: null, children: null, alone: null })
  })

  it("says that the branches built on it stay, and what the next sync after the merge does to them", () => {
    expect(landing({ branchName: "auth", children: ["login"] }).children).toBe("login is built on auth: it stays, and after the merge the next sync builds it on master.")
    // a branch in the middle: what lands, and what stays
    expect(landing({ branchName: "login", ancestors: ["auth"], commitCount: 2, children: ["oauth", "device"] })).toMatchObject({
      lands: "Lands login with auth, which it is built on: 2 branches · 2 commits into master.",
      children: "oauth and device are built on login: they stay, and after the merge the next sync builds them on master.",
    })
  })

  describe("with the trial build of the branch alone, the second way", () => {
    const stacked = { branchName: "perf-part-2", ancestors: ["IJPL-257337"], commitCount: 3 }

    it("offers the unstack when the branch applies on the baseline without its parent, after the landing", () => {
      const result = landing({ ...stacked, independence: { kind: "builds", parent: "IJPL-257337", uses: [], symbols: false } })
      expect(result.lands).toBe("Lands perf-part-2 with IJPL-257337, which it is built on: 2 branches · 3 commits into master.")
      expect(result.alone).toEqual({
        kind: "unstack",
        parent: "IJPL-257337",
        text: "To land perf-part-2 alone: it applies on the baseline without IJPL-257337. Compile dependencies are not checked.",
        warning: false,
      })
      // with the symbols on, no name of the parent is used either
      expect(landing({ ...stacked, independence: { kind: "builds", parent: "IJPL-257337", uses: [], symbols: true } }).alone).toMatchObject({
        text: "To land perf-part-2 alone: it applies on the baseline without IJPL-257337 and uses no name it adds. Not checked by a compiler.",
      })
    })

    it("warns that a branch that applies alone uses a name its parent adds, the unstack still offered", () => {
      const uses = [{ commit: CHILD, name: "parsedIcon", declaredBy: PARSED }]
      expect(landing({ ...stacked, independence: { kind: "builds", parent: "IJPL-257337", uses, symbols: true } }).alone).toEqual({
        kind: "unstack",
        parent: "IJPL-257337",
        text: '"Cache the parsed icon" uses parsedIcon, which "Parse the icon off the EDT" of IJPL-257337 adds. perf-part-2 applies on the baseline without IJPL-257337, but needs it to compile. Found by names, not by a compiler.',
        warning: true,
      })
      const more = [...uses, { commit: CHILD, name: "iconCache", declaredBy: PARSED }]
      expect(landing({ ...stacked, independence: { kind: "builds", parent: "IJPL-257337", uses: more, symbols: true } }).alone?.text).toMatch(
        /^"Cache the parsed icon" uses parsedIcon and 1 more name, which/,
      )
    })

    it("offers to move the commits it needs when no other branch needs them", () => {
      const independence: StackIndependence = { kind: "needs", parent: "IJPL-257337", childCommit: CHILD, files: ["AppUIUtil.kt"], parentCommits: [PARSED], movable: true }
      expect(landing({ ...stacked, independence }).alone).toEqual({
        kind: "move",
        commits: [PARSED],
        label: "Move 1 Commit Here",
        text: 'To land perf-part-2 alone, move what it needs here: "Cache the parsed icon" changes lines that "Parse the icon off the EDT" of IJPL-257337 adds.',
      })
      const two = { ...independence, parentCommits: [PARSED, { ...PARSED, id: "p0", subject: "Load icons" }] }
      expect(landing({ ...stacked, independence: two }).alone).toMatchObject({ kind: "move", label: "Move 2 Commits Here" })
      expect(landing({ ...stacked, independence: two }).alone?.text).toMatch(/move what it needs here: "Cache the parsed icon" changes lines that 2 commits of IJPL-257337 add\.$/)
    })

    it("offers no way alone when the needed commits can't move, nor while the check runs or after it failed", () => {
      const independence: StackIndependence = { kind: "needs", parent: "IJPL-257337", childCommit: CHILD, files: ["AppUIUtil.kt"], parentCommits: [PARSED], movable: false }
      expect(landing({ ...stacked, independence }).alone).toBeNull()
      // a conflict without a needed commit
      expect(landing({ ...stacked, independence: { ...independence, parentCommits: [], movable: true } }).alone).toBeNull()
      expect(landing({ ...stacked, independence: null }).alone).toBeNull()
      // the landing stays the lead
      expect(landing({ ...stacked, independence }).lands).toBe("Lands perf-part-2 with IJPL-257337, which it is built on: 2 branches · 3 commits into master.")
    })
  })
})
