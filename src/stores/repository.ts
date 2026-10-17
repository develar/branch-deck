import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { commands } from "@/lib/bindings"
import type { CommandError, GitSetup } from "@/lib/bindings"
import { normalizeBranchPrefix } from "@/lib/branch-prefix"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { VcsRequestFactory } from "@/lib/git/vcs-request"
import { appSettingsStore, useAppSettings } from "@/stores/settings/appSettings"
import { getRecentProjects, repositorySettingsStore, setRecentProjects, updateProject, useRecentProjects } from "@/stores/settings/repositorySettings"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"

/** The prefix of `getBranchPrefixFromGitConfig` (`repositoryScoped` is not read yet: the app's prefix still wins) */
export type BranchPrefixResult = { status: "ok"; data: string } | { status: "error"; error: CommandError }

export interface PathValidation {
  valid: boolean
  path?: string

  error?: string | null
  errorDetails?: string | null
}

export interface RepositoryState {
  pathValidation: PathValidation
  /** Branch prefix from `git config branchdeck.branchPrefix` of the selected repository (or the global git config) */
  gitProvidedBranchPrefix: BranchPrefixResult
  isLoadingBranchPrefix: boolean
  /** Git, the git identity and the suggested branch prefix (`check_git_setup`); `null` until the first check returns */
  gitSetup: GitSetup | null
}

/**
 * Transient (not persisted) state of the selected repository.
 * The selected project itself is `recentProjects[0]` in the persisted repository settings store.
 */
export const repositoryStore = createStore<RepositoryState>(() => ({
  pathValidation: { valid: true },
  gitProvidedBranchPrefix: { status: "error", error: { message: "Not loaded" } },
  isLoadingBranchPrefix: true,
  gitSetup: null,
}))

export function useRepository<T>(selector: (state: RepositoryState) => T): T {
  return useStore(repositoryStore, selector)
}

// ---- selected project ----

export function getSelectedProject(): ProjectMetadata | null {
  return getRecentProjects()[0] ?? null
}

/**
 * The repository whose commits the diffs read: the one a sub-window was opened for (`repositoryPath` of its
 * `__INIT_DATA__`, read at call time, so a reused window follows its new data), otherwise the selected project. A sub-window
 * never follows the project switched in the main window.
 */
export function getDiffRepositoryPath(): string | undefined {
  const data = window.__INIT_DATA__
  if (typeof data === "object" && data !== null && "repositoryPath" in data && typeof data.repositoryPath === "string") {
    return data.repositoryPath
  }
  return getSelectedProject()?.path
}

/** The whole metadata: re-renders on every change of it (the last sync time, the issue links); prefer a field */
export function useSelectedProject(): ProjectMetadata | null {
  return useRecentProjects(projects => projects[0] ?? null)
}

/** The path of the selected repository; re-renders only when another one is selected, not when its metadata changes */
export function useSelectedProjectPath(): string | undefined {
  return useRecentProjects(projects => projects[0]?.path)
}

/** One field of the selected repository's metadata; re-renders only when it changes */
export function useSelectedProjectField<K extends keyof ProjectMetadata>(field: K): ProjectMetadata[K] | undefined {
  return useRecentProjects(projects => projects[0]?.[field])
}

// ---- effective branch prefix ----

function computeEffectiveBranchPrefix(globalUserBranchPrefix: string | undefined, cachedBranchPrefix: string | undefined, gitProvidedBranchPrefix: BranchPrefixResult): string {
  return (
    normalizeBranchPrefix(globalUserBranchPrefix) ||
    normalizeBranchPrefix(cachedBranchPrefix) ||
    (gitProvidedBranchPrefix.status === "ok" ? normalizeBranchPrefix(gitProvidedBranchPrefix.data) : "")
  )
}

/** User setting first, then the prefix cached for the selected project, then the one from git config. */
export function useEffectiveBranchPrefix(): string {
  const globalUserBranchPrefix = useAppSettings(state => state.globalUserBranchPrefix)
  const cachedBranchPrefix = useRecentProjects(projects => projects[0]?.cachedBranchPrefix)
  const gitProvidedBranchPrefix = useRepository(state => state.gitProvidedBranchPrefix)
  return computeEffectiveBranchPrefix(globalUserBranchPrefix, cachedBranchPrefix, gitProvidedBranchPrefix)
}

/** Non-hook variant of {@link useEffectiveBranchPrefix} for event handlers and stores. */
export function getEffectiveBranchPrefix(): string {
  return computeEffectiveBranchPrefix(
    appSettingsStore.getState().globalUserBranchPrefix,
    getSelectedProject()?.cachedBranchPrefix,
    repositoryStore.getState().gitProvidedBranchPrefix,
  )
}

/** Full git branch name of a virtual branch, e.g. `john/virtual/feature-auth`. */
export function getFullBranchName(branchName: string): string {
  return `${getEffectiveBranchPrefix()}/virtual/${branchName}`
}

export const vcsRequestFactory = new VcsRequestFactory(() => getSelectedProject()?.path ?? "", getEffectiveBranchPrefix)

// ---- actions ----

function setPathValidation(pathValidation: PathValidation): void {
  repositoryStore.setState({ pathValidation })
}

/**
 * Makes the project the selected one (moves or adds it to the front of the recent projects).
 * The path is not validated here: browse validates explicitly, and an invalid saved path is reported on prefix load or sync.
 */
export function selectProject(project: ProjectMetadata, preValidated: PathValidation = { valid: true, error: undefined }): void {
  setPathValidation(preValidated)
  if (!preValidated.valid) {
    // don't add invalid paths to recent projects
    return
  }

  const projects = getRecentProjects()
  const existingIndex = projects.findIndex(it => it.path === project.path)
  if (existingIndex > 0) {
    // move the existing project to the front
    const existing = projects[existingIndex]!
    setRecentProjects([existing, ...projects.toSpliced(existingIndex, 1)])
  } else if (existingIndex === -1) {
    // add a new project at the front
    setRecentProjects([project, ...projects])
  }
  // existingIndex === 0: already selected
}

const BROWSE_FAILURE = "Cannot browse for a repository"

export async function browseRepository(): Promise<void> {
  try {
    const result = await commands.browseRepository()
    if (result.status === "ok") {
      const { path, valid, error } = result.data
      if (path) {
        const project: ProjectMetadata = getRecentProjects().find(it => it.path === path) ?? { path, cachedBranchPrefix: undefined }
        if (error) {
          selectProject(project, { valid, path, error: `Invalid path: ${path}`, errorDetails: getErrorDetails(error) })
        } else {
          selectProject(project, { valid })
        }
      }
    } else {
      // internal error (Result::Err from Rust)
      notifyError(BROWSE_FAILURE, result.error)
      setPathValidation({ valid: false, error: BROWSE_FAILURE, errorDetails: result.error.message })
    }
  } catch (error) {
    notifyError(BROWSE_FAILURE, error)
    setPathValidation({ valid: false, error: BROWSE_FAILURE, errorDetails: getErrorDetails(error) })
  }
}

let loadingPromise: Promise<void> | null = null

/** Pending branch prefix load, if any. Sync awaits it so it does not run with a stale prefix. */
export function getLoadingPromise(): Promise<void> | null {
  return loadingPromise
}

export function triggerBranchPrefixLoad(project: ProjectMetadata | null): void {
  void loadGitSetup(project)
  const promise = loadBranchPrefix(project)
  loadingPromise = promise
  void promise.finally(() => {
    if (loadingPromise === promise) {
      loadingPromise = null
    }
  })
}

/** Not awaited by sync: a sync without git fails on its own, with the same message */
async function loadGitSetup(project: ProjectMetadata | null): Promise<void> {
  const currentPath = project?.path
  try {
    const result = await commands.checkGitSetup({ repositoryPath: currentPath || "" })
    if (getSelectedProject()?.path === currentPath && result.status === "ok") {
      repositoryStore.setState({ gitSetup: result.data })
    }
  } catch (error) {
    // the check is advisory: whatever it would have found, the branch prefix load and sync report
    console.warn("Cannot check the git setup", error)
  }
}

async function loadBranchPrefix(project: ProjectMetadata | null): Promise<void> {
  const currentPath = project?.path
  // the selection may change while the command runs; a stale result must not overwrite the state of the new selection
  const isStale = () => getSelectedProject()?.path !== currentPath

  repositoryStore.setState({ isLoadingBranchPrefix: true })
  try {
    const result = await commands.getBranchPrefixFromGitConfig({ repositoryPath: currentPath || "" })

    if (result.status === "error") {
      if (isStale()) {
        return
      }

      // repository not accessible; the error message already contains context
      repositoryStore.setState({
        pathValidation: {
          valid: false,
          path: currentPath,
          error: result.error.message,
          errorDetails: result.error.message,
        },
        // the error is shown via pathValidation
        gitProvidedBranchPrefix: { status: "ok", data: "" },
      })
      return
    }

    const prefix = result.data.prefix
    if (project != null) {
      updateProject(project.path, { cachedBranchPrefix: prefix || undefined })
    }

    if (!isStale()) {
      // repository is accessible - clear any previous error
      repositoryStore.setState({ gitProvidedBranchPrefix: { status: "ok", data: prefix }, pathValidation: { valid: true } })
    }
  } catch (error) {
    if (isStale()) {
      return
    }

    repositoryStore.setState({
      pathValidation: {
        valid: false,
        path: currentPath,
        error: `Repository not accessible: ${currentPath}`,
        errorDetails: getErrorDetails(error),
      },
      gitProvidedBranchPrefix: { status: "ok", data: "" },
    })
  } finally {
    if (!isStale()) {
      repositoryStore.setState({ isLoadingBranchPrefix: false })
    }
  }
}

// ---- initialization ----

let disposeRepository: (() => void) | null = null

/**
 * Loads the branch prefix of the selected project and reloads it (debounced) whenever the selected path changes,
 * including changes made by another window. Idempotent; called by the main window when the repository UI mounts.
 */
export function initRepository(): void {
  if (disposeRepository !== null) {
    return
  }

  triggerBranchPrefixLoad(getSelectedProject())

  let timer: ReturnType<typeof setTimeout> | undefined
  // only the path matters: cachedBranchPrefix updates must not trigger a reload
  const unsubscribe = repositorySettingsStore.subscribe((state, prevState) => {
    if (state.items[0]?.path === prevState.items[0]?.path) {
      return
    }

    // set synchronously so the UI does not render the "loaded" state of the new selection while the timer is pending
    repositoryStore.setState({ isLoadingBranchPrefix: true })
    clearTimeout(timer)
    timer = setTimeout(() => {
      timer = undefined
      // the backend handles invalid paths gracefully with the global fallback
      triggerBranchPrefixLoad(getSelectedProject())
    }, 50)
  })

  disposeRepository = () => {
    clearTimeout(timer)
    unsubscribe()
    disposeRepository = null
  }
}

// a hot-reloaded module must not leave the previous subscription behind
import.meta.hot?.dispose(() => {
  disposeRepository?.()
})
