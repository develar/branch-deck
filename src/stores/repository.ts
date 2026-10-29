import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { commands } from "@/lib/bindings"
import type { ApiError } from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { repositoryName } from "@/lib/repository-config"
import { getRecentProjects, setRecentProjects, useRecentProjects } from "@/stores/settings/repositorySettings"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"
import { useRepositoryStateField } from "@/stores/settings/repositoryState"
import type { RepositoryStateEntry } from "@/stores/settings/repositoryState"

export interface PathValidation {
  valid: boolean
  path?: string

  error?: string | null
  errorDetails?: string | null
  /**
   * The subscription of the window found it: the repository git cannot read (it went away, it is no repository any
   * more); the next answer of a subscription replaces it
   */
  ofSession?: boolean
}

export interface RepositoryState {
  /**
   * Whether the selected repository can be read: what browsing found of the path picked (`browse_repository`), else
   * what the subscription of its session failed with (`ofSession`)
   */
  pathValidation: PathValidation
  /**
   * The prefix the branches of the selected repository are named with, as the view of its session names it
   * (`TableHeader.prefix`: what Rust resolved for the subscription of the window, `src/model/store.ts`); null until the
   * subscription answers, "" when it failed
   */
  branchPrefix: string | null
  /**
   * Why the subscription of the window to the session of the selected repository failed, by its code: no branch prefix
   * (`no_branch_prefix`, the welcome card, with the prefix git suggests), no usable git (`git_not_found`,
   * `GitSetupAlert`), a repository git cannot read (its error alert); null while it holds or has not answered
   */
  sessionFailure: ApiError | null
}

/**
 * Transient (not persisted) state of the selected repository.
 * The selected project itself is `recentProjects[0]` in the persisted repository settings store.
 */
export const repositoryStore = createStore<RepositoryState>(() => ({
  pathValidation: { valid: true },
  branchPrefix: null,
  sessionFailure: null,
}))

export function useRepository<T>(selector: (state: RepositoryState) => T): T {
  return useStore(repositoryStore, selector)
}

// ---- selected project ----

export function getSelectedProject(): ProjectMetadata | null {
  return getRecentProjects()[0] ?? null
}

/**
 * Why an action that started in `repositoryPath` (a push, a Safe Push start, a pull request, an undo) stops before a
 * step that reads the branches: the main window shows another repository now, whose branch of the same name is another
 * branch; null while it shows that one. The steps that name the repository (a command with its path) go on.
 */
export function getRepositorySwitchReason(repositoryPath: string): string | null {
  return isSelectedRepository(repositoryPath) ? null : `${repositoryName(repositoryPath)} is no longer the open repository`
}

/** Whether the main window shows `repositoryPath` (the path of a `VcsRequest`) */
export function isSelectedRepository(repositoryPath: string): boolean {
  return getSelectedRepositoryPath() === repositoryPath
}

/** The path of the selected repository as a `VcsRequest` names it; empty while none is selected */
export function getSelectedRepositoryPath(): string {
  return (getSelectedProject()?.path ?? "").trim()
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

/** One field of what the syncs recorded of the selected repository (`repositoryState`); re-renders only when it changes */
export function useSelectedRepositoryState<K extends keyof RepositoryStateEntry>(field: K): RepositoryStateEntry[K] | undefined {
  return useRepositoryStateField(useSelectedProjectPath(), field)
}

// ---- the branch prefix and the session ----

/**
 * What the subscription of the window to the session of the selected repository answered: the prefix of its view, or
 * why it failed (the model store tells it, `src/model/store.ts`); both null while no subscription answered
 */
export function setSessionAnswer(branchPrefix: string | null, sessionFailure: ApiError | null): void {
  const state = repositoryStore.getState()
  if (state.branchPrefix === branchPrefix && state.sessionFailure === sessionFailure) {
    return
  }
  // a repository git cannot read is a problem of the path, as browsing finds one: the selector, the header and Sync say
  // so; no prefix (the welcome card) and no git (the git alert) are not
  const ofPath = sessionFailure !== null && sessionFailure.code !== "no_branch_prefix" && sessionFailure.code !== "git_not_found"
  let pathValidation = state.pathValidation
  if (ofPath) {
    pathValidation = { valid: false, error: sessionFailure.message, errorDetails: sessionFailure.message, ofSession: true }
  } else if (pathValidation.ofSession === true) {
    pathValidation = { valid: true }
  }
  repositoryStore.setState({ branchPrefix, sessionFailure, pathValidation })
}

/** Why the subscription of the selected repository failed ({@link RepositoryState.sessionFailure}); null otherwise */
export function useSessionFailure(): ApiError | null {
  return useRepository(state => state.sessionFailure)
}

/** Whether the prefix of the selected repository is still unknown: its subscription has not answered */
export function useIsSubscribing(): boolean {
  return useRepository(state => state.branchPrefix === null)
}

/**
 * The prefix the branches of the selected repository are named with: the one of the view of its session (the
 * repository's own git prefix, else the app's, else the global git prefix, as Rust resolved it); "" without one. The
 * page decides nothing by it: every command of the repository resolves it in Rust again
 */
export function useEffectiveBranchPrefix(): string {
  return useRepository(state => state.branchPrefix ?? "")
}

/** Non-hook variant of {@link useEffectiveBranchPrefix} for event handlers and stores. */
export function getEffectiveBranchPrefix(): string {
  return repositoryStore.getState().branchPrefix ?? ""
}

/** Full git branch name of a virtual branch, e.g. `john/virtual/feature-auth`. */
export function getFullBranchName(branchName: string): string {
  return `${getEffectiveBranchPrefix()}/virtual/${branchName}`
}

/** Full git branch name of the stack branch of a stack that forks, all its commits in one branch: `john/stack/auth` */
export function getFullStackBranchName(root: string): string {
  return `${getEffectiveBranchPrefix()}/stack/${root}`
}

// ---- actions ----

function setPathValidation(pathValidation: PathValidation): void {
  repositoryStore.setState({ pathValidation })
}

/**
 * Makes the project the selected one (moves or adds it to the front of the recent projects).
 * The path is not validated here: browse validates explicitly, and an invalid saved path is the failure of the
 * subscription of the window to its session.
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
        const project: ProjectMetadata = getRecentProjects().find(it => it.path === path) ?? { path }
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
