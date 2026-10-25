import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { LandedBranch, PatronusRobot } from "@/lib/bindings"
import { resetEventToasts, showEventToast, toastKeys } from "@/lib/event-toast"
import { showMergeToast } from "@/lib/landing-toast"
import { landedStore } from "@/stores/landed"
import { repositoryStore } from "@/stores/repository"
import { updateProjectStore } from "@/stores/update-project"
import { describeToast, openToasts, resetToasts } from "@/test-support/toast-mock"

const { calls } = vi.hoisted(() => ({ calls: [] as string[] }))

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@/lib/patronus", () => ({ openRobot: (robot: PatronusRobot) => calls.push(`open ${robot.url}`) }))
vi.mock("@/stores/update-project", async () => {
  const { createStore } = await import("zustand/vanilla")
  return {
    // what runs, by repository: the update the toast, the header or a key started
    updateProjectStore: createStore<{ running: Record<string, { run: { kind: "update" | "fetch" }; endedAt: number | null }> }>(() => ({ running: {} })),
    selectRunningUpdate: (state: { running: Record<string, { endedAt: number | null }> }, repositoryPath: string) => {
      const running = state.running[repositoryPath]
      return running?.endedAt === null ? running : undefined
    },
    runUpdateProjectOf: async (repositoryPath: string) => {
      calls.push(`update ${repositoryPath}`)
    },
    runFetchBaseline: async (repositoryPath: string) => {
      calls.push(`fetch ${repositoryPath}`)
    },
  }
})
vi.mock("@/stores/landed", async () => {
  const { createStore } = await import("zustand/vanilla")
  return { landedStore: createStore(() => ({ repositoryPath: null as string | null, landed: [] as LandedBranch[], upstream: null as string | null })) }
})
vi.mock("@/stores/repository", async () => {
  const { createStore } = await import("zustand/vanilla")
  const repositoryStore = createStore(() => ({ selected: "/repo" }))
  return { repositoryStore, getSelectedProject: () => ({ path: repositoryStore.getState().selected }) }
})
vi.mock("@/stores/branch-sync", () => ({ branchSyncStore: { getState: () => ({ baselineStatus: null }) } }))
vi.mock("@/stores/settings/repositoryState", () => ({ getRepositoryState: () => undefined }))

// merged at 1000 s
const ROBOT = { id: "r1", branchName: "auth", stackRoot: null, status: "success", finishTime: 1000, url: "https://patronus/r1" } as unknown as PatronusRobot

function landed(name: string, landedAt: number, squashed = false): LandedBranch {
  return {
    name,
    remaining: 0,
    landedAt,
    commits: [{ id: `${name}-1`, subject: `Change ${name}`, upstream: squashed ? null : { id: "u", committer: "Patronus", time: landedAt } }],
  } as unknown as LandedBranch
}

function run(kind: "update" | "fetch" | null, endedAt: number | null = null): void {
  updateProjectStore.setState({ running: kind === null ? {} : { "/repo": { run: { kind }, endedAt } } } as never)
}

function toasts() {
  return openToasts().map(describeToast)
}

beforeEach(() => {
  resetEventToasts()
  resetToasts()
  calls.length = 0
  run(null)
  landedStore.setState({ repositoryPath: null, landed: [], upstream: null } as never)
  repositoryStore.setState({ selected: "/repo" } as never)
})

describe("the toast of a merge", () => {
  it("is one toast from the merge to the landing: Update Project, Updating… while the update runs, then what the update landed in place", () => {
    showMergeToast("/repo", ROBOT, "auth merged into master", "Safe Push", [])
    expect(toasts()).toEqual([["auth merged into master", "Safe Push", "Update Project", "Open in Patronus"]])
    const merged = openToasts()[0]!
    merged.actionProps!.onClick!()
    expect(calls).toEqual(["update /repo"])

    // the update runs (started here, from the header or by ⌘T): the toast stays and offers no second one
    run("update")
    expect(openToasts()).toEqual([merged])
    expect(toasts()).toEqual([["auth merged into master", "Safe Push", "[Updating…]", "Open in Patronus"]])
    expect(merged.timeout).toBe(0)

    // the outcome of the update takes its place, as `applyUpdate` shows it
    showEventToast({
      key: toastKeys.update("/repo"),
      supersedes: [toastKeys.merged("/repo")],
      render: () => ({ type: "success", title: "auth landed in origin/master, squashed", description: "Updated master: rebased 104 commits" }),
    })
    run("update", 2000)
    run(null)
    expect(openToasts()).toEqual([merged])
    expect(toasts()).toEqual([["auth landed in origin/master, squashed", "Updated master: rebased 104 commits", undefined, undefined]])
  })

  it("says in place what a sync found landed after an update elsewhere (bd update, the IDE)", () => {
    showMergeToast("/repo", ROBOT, "Whole air stack merged into master", "Safe Push", ["air", "bazel"])
    // a landing of a branch of the same name days before is not of this merge
    landedStore.setState({ repositoryPath: "/repo", landed: [landed("air", 1), landed("bazel", 1)], upstream: "origin/master" } as never)
    expect(toasts()).toEqual([["Whole air stack merged into master", "Safe Push", "Update Project", "Open in Patronus"]])
    landedStore.setState({ repositoryPath: "/repo", landed: [landed("air", 1100), landed("bazel", 1100, true), landed("ui", 1100)], upstream: "origin/master" } as never)
    expect(toasts()).toEqual([["2 branches landed in origin/master", "air and bazel · found by a sync", undefined, undefined]])
  })

  it("closes after an update that ended without an outcome of its own: the dialog of the conflict says it", () => {
    showMergeToast("/repo", ROBOT, "auth merged into master", "Safe Push", [])
    run("update")
    run("update", 2000)
    expect(openToasts()).toEqual([])
  })

  it("offers Fetch once the main window shows another repository, and Update Project again on the way back", () => {
    showMergeToast("/repo", ROBOT, "auth merged into master", "Safe Push", [])
    repositoryStore.setState({ selected: "/other" } as never)
    expect(toasts()).toEqual([["auth merged into master", "Safe Push", "Fetch", "Open in Patronus"]])
    openToasts()[0]!.actionProps!.onClick!()
    expect(calls).toEqual(["fetch /repo"])
    repositoryStore.setState({ selected: "/repo" } as never)
    expect(toasts()).toEqual([["auth merged into master", "Safe Push", "Update Project", "Open in Patronus"]])
    openToasts()[0]!.data!.secondaryAction!.onClick()
    expect(calls).toEqual(["fetch /repo", "open https://patronus/r1"])
  })

  it("shows the landing at once when an update elsewhere landed it before the toast showed: the toast waited for the focus", () => {
    landedStore.setState({ repositoryPath: "/repo", landed: [landed("auth", 1100)], upstream: "origin/master" } as never)
    showMergeToast("/repo", ROBOT, "auth merged into master", "Safe Push", [])
    expect(toasts()).toEqual([["auth landed in origin/master", "Found by a sync", undefined, undefined]])
  })
})
