import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { sessionRepositoryPath, whenSessionReady } from "@/model/store"
import { getSelectedRepositoryPath, repositoryStore } from "@/stores/repository"
import { appSettingsStore } from "@/stores/settings/appSettings"
import { settingsCaughtUp } from "@/stores/settings/persistent-store"
import { getRecentProjects, repositorySettingsStore } from "@/stores/settings/repositorySettings"

/** Presentation progress only: whether a prefix is usable remains the current repository session's answer. */
const onboardingStore = createStore<{ ready: boolean; focusPrefixPath: string | null }>(() => ({ ready: false, focusPrefixPath: null }))

export function useOnboardingReady(): boolean {
  return useStore(onboardingStore, state => state.ready)
}

export function useOnboardingPrefixFocus(path: string | undefined): boolean {
  return useStore(onboardingStore, state => path !== undefined && state.focusPrefixPath === path)
}

export function consumeOnboardingPrefixFocus(): void {
  onboardingStore.setState({ focusPrefixPath: null })
}

/** Lives above the keyed welcome rows, so selection, errors and session remounts retain the walkthrough. */
export function trackOnboarding(): () => void {
  let stopped = false
  let ready = false
  let lastPath = getSelectedRepositoryPath()
  let awaiting: Promise<void> | null = null
  let completedAnswer: Promise<void> | null = null

  const check = () => {
    if (!ready || stopped) {
      return
    }
    const path = getSelectedRepositoryPath()
    const active = appSettingsStore.getState().onboardingStatus === "inProgress"
    if (path !== lastPath) {
      lastPath = path
      onboardingStore.setState({ focusPrefixPath: active ? (path ?? null) : null })
    }
    const repository = repositoryStore.getState()
    if (!active || path === undefined || sessionRepositoryPath() !== path || !repository.pathValidation.valid || !repository.branchPrefix || repository.sessionFailure !== null) {
      return
    }
    // A cached frame may already contain a prefix. Wait for the subscription, and reject an answer left behind by a switch.
    const answer = whenSessionReady()
    if (awaiting === answer || completedAnswer === answer) {
      return
    }
    awaiting = answer
    void answer.then(() => {
      if (stopped || awaiting !== answer) {
        return
      }
      awaiting = null
      const current = repositoryStore.getState()
      if (
        getSelectedRepositoryPath() === path &&
        sessionRepositoryPath() === path &&
        whenSessionReady() === answer &&
        current.pathValidation.valid &&
        current.branchPrefix &&
        current.sessionFailure === null &&
        appSettingsStore.getState().onboardingStatus === "inProgress"
      ) {
        // A failed settings write rolls the value back. Do not retry it in a loop for the same subscription.
        completedAnswer = answer
        appSettingsStore.setState({ onboardingStatus: "complete" })
        consumeOnboardingPrefixFocus()
      }
    })
  }

  const stops = [repositorySettingsStore.subscribe(check), repositoryStore.subscribe(check), appSettingsStore.subscribe(check)]
  void settingsCaughtUp.then(() => {
    if (stopped) {
      return
    }
    // Catch-up is restoration, not a selection made in this window: it must not request focus.
    lastPath = getSelectedRepositoryPath()
    ready = true
    if (appSettingsStore.getState().onboardingStatus === undefined) {
      appSettingsStore.setState({ onboardingStatus: getRecentProjects().length === 0 ? "inProgress" : "complete" })
    }
    onboardingStore.setState({ ready: true })
    check()
  })
  return () => {
    stopped = true
    for (const stop of stops) {
      stop()
    }
  }
}

export function useOnboardingTracking(): void {
  useEffect(() => trackOnboarding(), [])
}
