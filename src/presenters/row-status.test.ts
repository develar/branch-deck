// @spec openspec/specs/branch/branch-row/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { PatronusRobot, RegenerateFile, RowFact, RowStatus } from "@/lib/bindings"
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
  return { branch, landedIn: null, robot: undefined, parked: null, wait: null, ...extra }
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

  it("says regenerate with each file, why it is generated and the commit its copy lacks", () => {
    const lockFile: RegenerateFile = {
      commit: { id: "b1", subject: "Add a dependency", branch: "feature" },
      path: "app/Cargo.lock",
      generatedBy: { kind: "name", name: "lockFile" },
      lacks: { id: "a1", subject: "Add another dependency", branch: "core" },
    }
    const regenerate: RowFact = { kind: "regenerate", files: [lockFile], fix: { kind: "restack", branch: "feature", onto: "core" } }
    const branch = testBranch("feature", null, { remoteStatus: testRemoteStatus({ commitsAhead: 1, myCommitsAhead: 1, action: PUSH }) })
    const tooltip =
      "app/Cargo.lock (Lock file) was merged by words in the copy of “Add a dependency”, not written by its generator; the copy lacks “Add another dependency” of core. " +
      "Stack feature on core to land it too. It clears once core lands, or a commit of the branch writes the file."
    expect(presentRowCell(rowStatus(regenerate), context(branch))).toEqual({ kind: "text", texts: { text: "regenerate Cargo.lock", tooltip, color: "warning" } })
    // beside a running robot, which tests that copy: the mark names it
    expect(presentRowMark(rowStatus(RUNNING, [regenerate]), context(branch, { robot }))).toEqual({ color: "warning", lines: [tooltip] })
    const header: RegenerateFile = { ...lockFile, path: "gen/limits.js", generatedBy: { kind: "header", line: "Generated by tools/limits.mjs" }, lacks: null }
    const two: RowFact = { kind: "regenerate", files: [lockFile, header], fix: null }
    const cell = presentRowCell(rowStatus(two), context(branch))
    expect(cell).toMatchObject({ kind: "text", texts: { text: "regenerate 2 files", color: "warning" } })
    expect(
      cell.kind === "text" &&
        cell.texts.tooltip.endsWith("gen/limits.js (Generated by tools/limits.mjs) was merged by words in the copy of “Add a dependency”, not written by its generator."),
    ).toBe(true)
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

  it("says the state the next step waits in, worded by the step, and the preview its words: no action of the slot says it", () => {
    const branch = testBranch("docs", null, { remoteStatus: testRemoteStatus() })
    const wait = {
      label: "Awaiting review",
      description: "Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078",
      icon: undefined,
      tone: "quiet" as const,
      open: () => {},
    }
    expect(presentRowCell(rowStatus({ kind: "wait" }), context(branch))).toEqual({ kind: "wait" })
    expect(presentPreviewStatus(rowStatus({ kind: "wait" }), context(branch, { wait }))).toMatchObject({ text: "Awaiting review", tooltip: wait.description, color: "neutral" })
    expect(presentPreviewStatus(rowStatus({ kind: "wait" }), context(branch, { wait: { ...wait, label: "Changes requested", tone: "destructive" } }))).toMatchObject({
      color: "error",
    })
    // a sync lays over it as over any fact
    expect(presentRowCell(rowStatus({ kind: "wait" }), context({ ...branch, status: "Syncing" }))).toMatchObject({ kind: "text" })
  })

  it("marks the review after landing beside what landed the branch, its hover whose move it is in each state", () => {
    const auth = testBranch("auth", null, { remoteStatus: testRemoteStatus() })
    const mark = (state: Extract<RowFact, { kind: "shown" }>["state"]) =>
      presentRowMark(rowStatus({ kind: "landed" }, [{ kind: "shown", number: 12, state, reviewers: ["alice"] }]), context(auth))
    expect([
      mark({ kind: "draft", landing: true }),
      mark({ kind: "draft", landing: false }),
      mark({ kind: "inReview", waiting: ["alice", "bob"] }),
      mark({ kind: "changesRequested", by: ["alice"] }),
      mark({ kind: "extending", robot: "r2" }),
      mark({ kind: "approved", by: ["alice"] }),
    ]).toEqual([
      { color: "warning", lines: ["auth is landing: once its Safe Push merges, #12 opens and its reviewers are asked"] },
      { color: "warning", lines: ["Review after landing marked: once auth lands, #12 opens and asks alice"] },
      { color: "warning", lines: ["In review after landing: waits for alice and bob in #12"] },
      { color: "error", lines: ["alice requested changes in #12: commit a fix as (auth) …, and its Safe Push adds it to the review"] },
      { color: "warning", lines: ["A fix of auth is landing: once its Safe Push merges, it goes into #12 and the reviewers are asked again"] },
      { color: "success", lines: ["Approved by alice: #12 closes and its branches are deleted; the change stays in master"] },
    ])
    // the preview names the first fact that is not a robot: the review where nothing else is said
    expect(presentPreviewStatus(rowStatus(null, [{ kind: "shown", number: 12, state: { kind: "changesRequested", by: ["alice"] } }]), context(auth)).text).toBe(
      "changes requested after landing",
    )
  })
})
