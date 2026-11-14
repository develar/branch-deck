// The patches of a session as the page of the IDE gets them: each `ViewPatch` of `brd mcp` is one change of the mirror
// of its repository, and the page builds the patch back from the ops, which brings its view where the session's is
import { describe, expect, it } from "vite-plus/test"
import type { RepositoryView, TableChanges, ViewPatch } from "@/lib/bindings"
import { applyViewPatch } from "@/model/patch"
import { applyOps, patchOf, versionOf, viewOf } from "@/platform/air/view-mirror"
import type { MirrorState, ViewOp } from "@/platform/air/view-mirror"
import { emptyView, grouping } from "@/test-support/fake-sessions"

/** The parts of the view that a change of the mirror sets and removes by key, as `brd mcp` sends them */
const KEYED = new Set([
  "branches",
  "stacks",
  "pushes",
  "openPullRequests",
  "lands",
  "landedStates",
  "nextSteps",
  "actions",
  "attention",
  "githubLayers",
  "reviewGroups",
  "approvalLanding",
  "operations",
])

/** The ops of the change from `before` to `after`, as the mirror of `brd mcp` has it: a keyed part by key, the others whole */
function opsOf(before: RepositoryView, after: RepositoryView): ViewOp[] {
  const ops: ViewOp[] = []
  const was = before as unknown as Record<string, unknown>
  const is = after as unknown as Record<string, unknown>
  for (const part of new Set([...Object.keys(was), ...Object.keys(is)])) {
    if (JSON.stringify(was[part]) === JSON.stringify(is[part])) {
      continue
    }
    if (KEYED.has(part)) {
      const old = (was[part] ?? {}) as Record<string, unknown>
      const next = (is[part] ?? {}) as Record<string, unknown>
      for (const [key, value] of Object.entries(next)) {
        if (JSON.stringify(old[key]) !== JSON.stringify(value)) {
          ops.push({ op: "set", part, key, value })
        }
      }
      for (const key of Object.keys(old).filter(key => !(key in next))) {
        ops.push({ op: "remove", part, key })
      }
    } else {
      ops.push({ op: "part", part, value: is[part] })
    }
  }
  return ops
}

/** The view after `changes` as the page of the desktop app gets it, and as the page of the IDE builds it from the mirror */
function bothWays(view: RepositoryView, changes: TableChanges): { desktop: RepositoryView; ide: RepositoryView; patch: ViewPatch } {
  const next = { epoch: view.version.epoch, seq: view.version.seq + 1 }
  const desktop = applyViewPatch(view, { base: view.version, next, changes })
  const ops = opsOf(view, desktop)
  const before = view as unknown as MirrorState
  const after = applyOps(before, ops)
  const patch = patchOf(before, after, ops, view.version, versionOf(after, view.version))
  return { desktop, ide: applyViewPatch(view, patch), patch }
}

/** A view with the branches `names` of a grouping */
function viewWith(...names: string[]): RepositoryView {
  const view = emptyView()
  return applyViewPatch(view, { base: view.version, next: { epoch: 1, seq: 1 }, changes: grouping(...names) })
}

describe("The mirror of a view", () => {
  it("A renamed branch: one change sets branches/b, removes branches/a and replaces order", () => {
    const view = viewWith("a")
    const { desktop, ide, patch } = bothWays(view, { ...grouping("b"), removedBranches: ["a"] })
    expect(ide).toEqual(desktop)
    expect(Object.keys(patch.changes.branches ?? {})).toEqual(["b"])
    expect(patch.changes.removedBranches).toEqual(["a"])
    expect(patch.changes.order).toEqual(["b"])
    expect(patch.next).toEqual({ epoch: 1, seq: 2 })
  })

  it("The news: the part news of the kept list, of which the page tells those it has not told", () => {
    const view = viewWith("a")
    const news = [{ id: 1, kind: { kind: "landed", branches: ["a"] } }] as unknown as TableChanges["news"]
    const { desktop, ide, patch } = bothWays(view, { news })
    expect(ide).toEqual(desktop)
    expect(patch.changes.news).toEqual(news)
  })

  it("A change of the parts that hold keyed entries of their own: the sections, the Patronus view and the facts", () => {
    const view = viewWith("a", "b")
    const changes: TableChanges = {
      archived: { names: ["old"], newlyArchived: ["old"], integration: { old: { status: "integrated" } as never } },
      unassigned: [],
      patronus: { state: { kind: "fresh" } as never, branches: { a: { robots: [] } as never }, removedBranches: [] },
      facts: { readiness: { a: { value: null, state: { kind: "loading" }, seq: 1, observedAt: 1 } as never }, removedReadiness: [] },
    }
    const { desktop, ide } = bothWays(view, changes)
    expect(ide).toEqual(desktop)

    // and their removal
    const { desktop: removed, ide: removedInIde } = bothWays(desktop, {
      archived: { removedIntegration: ["old"] },
      patronus: { removedBranches: ["a"] },
      facts: { removedReadiness: ["a"] },
    })
    expect(removedInIde).toEqual(removed)
  })

  it("The version of a state is the part version of brd mcp, else the page numbers it by the serverSeq of air-ide", () => {
    const view = viewWith("a")
    expect(versionOf(view as unknown as MirrorState, { epoch: 9, seq: 9 })).toEqual(view.version)
    const { version: _version, ...withoutVersion } = view
    expect(versionOf(withoutVersion as unknown as MirrorState, { epoch: 9, seq: 3 })).toEqual({ epoch: 9, seq: 3 })
    expect(viewOf(withoutVersion as unknown as MirrorState, { epoch: 9, seq: 3 }).version).toEqual({ epoch: 9, seq: 3 })
  })

  it("Applies the ops by the table rule and leaves the state before them as it was", () => {
    const before: MirrorState = { order: ["a"], branches: { a: 1 } }
    const after = applyOps(before, [
      { op: "set", part: "branches", key: "b", value: 2 },
      { op: "remove", part: "branches", key: "a" },
      { op: "part", part: "order", value: ["b"] },
    ])
    expect(after).toEqual({ order: ["b"], branches: { b: 2 } })
    expect(before).toEqual({ order: ["a"], branches: { a: 1 } })
  })
})
