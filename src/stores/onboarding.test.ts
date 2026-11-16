// @spec openspec/specs/repository/onboarding/spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { consumeOnboardingPrefixFocus, trackOnboarding } from "@/stores/onboarding"
import type { RepositoryState } from "@/stores/repository"
import { repositoryStore } from "@/stores/repository"
import { appSettingsStore } from "@/stores/settings/appSettings"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"

const session = vi.hoisted(() => ({
  path: null as string | null,
  ready: Promise.resolve(),
  catchUp: Promise.withResolvers<undefined>(),
}))

vi.mock("@/model/store", () => ({ sessionRepositoryPath: () => session.path, whenSessionReady: () => session.ready }))
vi.mock("@/stores/settings/persistent-store", () => ({
  get settingsCaughtUp() {
    return session.catchUp.promise
  },
}))
vi.mock("@/stores/settings/appSettings", async () => {
  const { createStore } = await import("zustand/vanilla")
  return { appSettingsStore: createStore<{ onboardingStatus?: "inProgress" | "complete" }>(() => ({})) }
})
vi.mock("@/stores/settings/repositorySettings", async () => {
  const { createStore } = await import("zustand/vanilla")
  const store = createStore<{ items: { path: string }[] }>(() => ({ items: [] }))
  return { repositorySettingsStore: store, getRecentProjects: () => store.getState().items }
})
vi.mock("@/stores/repository", async () => {
  const { createStore } = await import("zustand/vanilla")
  const { repositorySettingsStore: projects } = await import("@/stores/settings/repositorySettings")
  return {
    repositoryStore: createStore<RepositoryState>(() => ({ pathValidation: { valid: true }, branchPrefix: null, sessionFailure: null })),
    getSelectedRepositoryPath: () => projects.getState().items[0]?.path,
  }
})

let stop: (() => void) | undefined
async function settle() {
  await Promise.resolve(undefined)
  await Promise.resolve(undefined)
}

beforeEach(() => {
  appSettingsStore.setState({ onboardingStatus: undefined })
  repositorySettingsStore.setState({ items: [] })
  repositoryStore.setState({ pathValidation: { valid: true }, branchPrefix: null, sessionFailure: null })
  session.path = null
  session.ready = Promise.resolve()
  session.catchUp = Promise.withResolvers<undefined>()
  consumeOnboardingPrefixFocus()
})
afterEach(() => stop?.())

async function start() {
  stop = trackOnboarding()
  session.catchUp.resolve(undefined)
  await settle()
}

function select(path: string) {
  session.path = path
  repositorySettingsStore.setState({ items: [{ path }] })
}

describe("first-time onboarding progress", () => {
  it("initializes after settings catch-up and treats restored recent projects as existing users", async () => {
    stop = trackOnboarding()
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBeUndefined()
    select("/restored")
    session.catchUp.resolve(undefined)
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("complete")
  })

  it("starts fresh onboarding and waits for the current subscription before completing", async () => {
    await start()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
    const answer = Promise.withResolvers<undefined>()
    session.ready = answer.promise
    select("/first")
    repositoryStore.setState({ branchPrefix: "jane" })
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
    answer.resolve(undefined)
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("complete")
  })

  it("resumes saved onboarding even when recent projects already exist", async () => {
    appSettingsStore.setState({ onboardingStatus: "inProgress" })
    select("/first")
    await start()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
  })

  it("ignores a late subscription from a repository left behind", async () => {
    await start()
    const old = Promise.withResolvers<undefined>()
    session.ready = old.promise
    select("/old")
    repositoryStore.setState({ branchPrefix: "old-prefix" })
    const current = Promise.withResolvers<undefined>()
    session.ready = current.promise
    repositoryStore.setState({ branchPrefix: null })
    select("/current")
    old.resolve(undefined)
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
    repositoryStore.setState({ branchPrefix: "current-prefix" })
    current.resolve(undefined)
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("complete")
  })

  it("keeps progress through a refused subscription and completes after retry", async () => {
    await start()
    select("/first")
    repositoryStore.setState({ branchPrefix: "", sessionFailure: { code: "no_branch_prefix", message: "Choose a prefix", details: null, retry: "no", operation_id: null } })
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
    repositoryStore.setState({ branchPrefix: "jane", sessionFailure: null })
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("complete")
  })

  it("does not replay completed onboarding for a later repository without a prefix", async () => {
    appSettingsStore.setState({ onboardingStatus: "complete" })
    await start()
    select("/later")
    repositoryStore.setState({ branchPrefix: "" })
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("complete")
  })

  it("does not loop completion writes when the settings writer rolls one back", async () => {
    await start()
    select("/first")
    repositoryStore.setState({ branchPrefix: "jane" })
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("complete")
    appSettingsStore.setState({ onboardingStatus: "inProgress" })
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
  })

  it("does not initialize or complete after tracking stops", async () => {
    stop = trackOnboarding()
    stop()
    session.catchUp.resolve(undefined)
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBeUndefined()
    await start()
    const answer = Promise.withResolvers<undefined>()
    session.ready = answer.promise
    select("/stopped")
    repositoryStore.setState({ branchPrefix: "jane" })
    stop?.()
    answer.resolve(undefined)
    await settle()
    expect(appSettingsStore.getState().onboardingStatus).toBe("inProgress")
  })
})
