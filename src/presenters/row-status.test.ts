// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { PatronusRobot, RowFact, RowStatus } from "@/lib/bindings"
import type { RowContext } from "@/presenters/row-status"
import { presentPreviewStatus, presentRowCell, presentRowMark } from "@/presenters/row-status"
import type { Branch } from "@/stores/branch-sync"
import { CREATE, PUSH, forcePush, testBranch, testRemoteStatus } from "@/stores/test-branch"

const RUNNING: RowFact = { kind: "robot", running: true, failed: false }
const robot = { id: "r1", pushMode: "rebaseAutosquash" } as PatronusRobot

function rowStatus(main: RowFact | null, also: RowFact[] = []): RowStatus {
  return { main, also }
}

function context(branch: Branch, extra: Partial<RowContext> = {}): RowContext {
  return { branch, landedIn: null, robot: undefined, parked: null, ...extra }
}

describe("the Status cell of a branch row", () => {
  it("says landed, in the merged hue, for a branch the baseline has all of: its push state no longer matters", () => {
    const branch = testBranch("feature-auth", null, { stack: null, remoteStatus: testRemoteStatus({ exists: false, commitsAhead: 1, myCommitsAhead: 1, action: CREATE }) })
    expect(presentRowCell(rowStatus(null), context(branch))).toMatchObject({ kind: "text", texts: { text: "not pushed", color: "primary" } })
    expect(presentRowCell(rowStatus({ kind: "landed" }), context(branch, { landedIn: "origin/master" }))).toEqual({
      kind: "text",
      texts: { text: "landed", tooltip: "Already in origin/master. Update to archive the branch.", color: "info" },
    })
  })

  it("is quiet for a branch up to date with its remote, which needs no action: the row shows nothing, the preview the text", () => {
    const pushed = testBranch("docs", null, { remoteStatus: testRemoteStatus() })
    const cell = presentRowCell(rowStatus(null), context(pushed))
    expect(cell).toMatchObject({ kind: "text", texts: { text: "up to date", quiet: true } })
    const preview = presentPreviewStatus(rowStatus(null), context(pushed))
    expect([preview.text, preview.quiet]).toEqual(["up to date", undefined])
    // a state that asks for something is never quiet
    const quiet = (status: RowStatus, branch: Branch) => {
      const cell = presentRowCell(status, context(branch))
      return cell.kind === "text" ? cell.texts.quiet : null
    }
    expect(quiet(rowStatus(null), { ...pushed, remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH }) })).toBeUndefined()
    expect(quiet(rowStatus({ kind: "blocked", by: "auth" }), pushed)).toBeUndefined()
  })

  it("is in the push colour for an amended or restacked branch: its push replaces older versions", () => {
    const restacked = testBranch("login", "auth", { remoteStatus: testRemoteStatus({ commitsRestacked: 1, fastForward: false, action: forcePush({ replaces: true }) }) })
    expect(presentRowCell(rowStatus(null), context(restacked))).toMatchObject({ texts: { text: "restacked", color: "primary" } })
    // someone else's commit beside them is still a drop
    const drops = {
      ...restacked,
      remoteStatus: testRemoteStatus({ commitsRestacked: 1, commitsBehind: 1, fastForward: false, action: forcePush({ drops: true, replaces: true }) }),
    }
    expect(presentRowCell(rowStatus(null), context(drops))).toMatchObject({ texts: { color: "warning" } })
  })

  it("shows the robot Rust puts first, a block and a conflict beside it as a mark that names them, and a sync over all of it", () => {
    const login = testBranch("login", "auth", { status: "Blocked", statusText: "Blocked: parent auth has a merge conflict" })
    const status = rowStatus(RUNNING, [{ kind: "blocked", by: "auth" }])
    expect(presentRowCell(status, context(login, { robot }))).toEqual({ kind: "robot", running: true })
    expect(presentRowMark(status, context(login, { robot }))).toEqual({ color: "warning", lines: ["Blocked: parent auth has a merge conflict"] })
    const conflict = testBranch("auth", null, { status: "MergeConflict", statusText: "conflict in config.txt", hasError: true })
    const both = rowStatus(RUNNING, [
      { kind: "syncFailed", status: "MergeConflict" },
      { kind: "blocked", by: "core" },
    ])
    expect(presentRowMark(both, context(conflict, { robot }))).toMatchObject({ color: "error" })
    // the preview shows its robot apart: the first fact that is not the robot
    expect(presentPreviewStatus(status, context(login, { robot }))).toMatchObject({ text: "blocked by auth", color: "warning" })
    // a sync passes over the cell; no mark while it runs
    const syncing = { ...login, status: "Syncing" as const, processedCount: 1, commitCount: 2 }
    expect(presentRowCell(status, context(syncing, { robot }))).toMatchObject({ kind: "text", texts: { text: "syncing 1/2…" } })
    expect(presentRowMark(status, context(syncing, { robot }))).toBeNull()
  })

  it("says a failed sync as the badge of its error, a stack problem by its message", () => {
    const failed = testBranch("auth", null, { status: "MergeConflict", statusText: "conflict in config.txt", hasError: true })
    expect(presentRowCell(rowStatus({ kind: "syncFailed", status: "MergeConflict" }), context(failed))).toMatchObject({
      texts: { text: "conflict in config.txt", color: "error", badge: true },
    })
    expect(presentRowCell(rowStatus({ kind: "syncFailed", status: "AnalyzingConflict" }), context({ ...failed, statusText: "" }))).toMatchObject({
      texts: { tooltip: "Analyzing conflict…", color: "warning" },
    })
  })
})
