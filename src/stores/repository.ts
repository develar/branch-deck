import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { commands } from "@/lib/bindings"
import type { GitSetup, ResolvedBranchPrefix } from "@/lib/bindings"
import { appPrefixOf } from "@/lib/branch-prefix"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { VcsRequestFactory } from "@/lib/git/vcs-request"
import { repositoryName } from "@/lib/repository-config"
import { onWriterChange, snapshotValueOf } from "@/stores/settings/persistent-store"
import { getRecentProjects, repositorySettingsStore, setRecentProjects, updateProject, useRecentProjects } from "@/stores/settings/repositorySettings"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"
import { useRepositoryStateField } from "@/stores/settings/repositoryState"
import type { RepositoryStateEntry } from "@/stores/settings/repositoryState"

/** No prefix: a repository git cannot read (the error is shown through `pathValidation`), or one nothing sets a prefix for */
export const NO_BRANCH_PREFIX: ResolvedBranchPrefix = { prefix: "", source: "none" }

export interface PathValidation {
  valid: boolean
  path?: string

  error?: string | null
  errorDetails?: string | null
}

export interface RepositoryState {
  pathValidation: PathValidation
  /**
   * The prefix the branches of the selected repository are named with, as Rust resolves it (`resolve_branch_prefix`),
   * with its source; null until the first answer for the repository, while its cached prefix names the branches
   */
  branchPrefix: ResolvedBranchPrefix | null
  /** Git, the git identity and the suggested branch prefix (`check_git_setup`); `null` until the first check returns */
  gitSetup: GitSetup | null
}

/**
 * Transient (not persisted) state of the selected repository.
 * The selected project itself is `recentProjects[0]` in the persisted repository settings store.
 */
export const repositoryStore = createStore<RepositoryState>(() => ({
  pathValidation: { valid: true },
  branchPrefix: null,
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

/** One field of the selected repository's metadata; re-renders only when it changes */
export function useSelectedProjectField<K extends keyof ProjectMetadata>(field: K): ProjectMetadata[K] | undefined {
  return useRecentProjects(projects => projects[0]?.[field])
}

// ---- effective branch prefix ----

/** The prefix of the selected repository: the last answer of Rust, else the one cached for the repository until it answers */
function effectiveBranchPrefix(branchPrefix: ResolvedBranchPrefix | null, cachedBranchPrefix: ResolvedBranchPrefix | undefined): ResolvedBranchPrefix {
  return branchPrefix ?? cachedBranchPrefix ?? NO_BRANCH_PREFIX
}

/** Whether the first answer of Rust for the selected repository is still out */
export function useIsLoadingBranchPrefix(): boolean {
  return useRepository(state => state.branchPrefix === null)
}

/**
 * The prefix the branches of the selected repository are named with, and its source: what Rust resolved (the
 * repository's own git prefix, else the app's, else the global git prefix), shown as it is. Every command of the sync
 * family resolves it again in Rust; the page decides nothing by it
 */
export function useResolvedBranchPrefix(): ResolvedBranchPrefix {
  const branchPrefix = useRepository(state => state.branchPrefix)
  const cachedBranchPrefix = useRecentProjects(projects => projects[0]?.cachedBranchPrefix)
  return effectiveBranchPrefix(branchPrefix, cachedBranchPrefix)
}

/** The prefix the branches of the selected repository are named with ({@link useResolvedBranchPrefix}) */
export function useEffectiveBranchPrefix(): string {
  return useResolvedBranchPrefix().prefix
}

/** Non-hook variant of {@link useEffectiveBranchPrefix} for event handlers and stores. */
export function getEffectiveBranchPrefix(): string {
  return effectiveBranchPrefix(repositoryStore.getState().branchPrefix, getSelectedProject()?.cachedBranchPrefix).prefix
}

/** Full git branch name of a virtual branch, e.g. `john/virtual/feature-auth`. */
export function getFullBranchName(branchName: string): string {
  return `${getEffectiveBranchPrefix()}/virtual/${branchName}`
}

/** Full git branch name of the stack branch of a stack that forks, all its commits in one branch: `john/stack/auth` */
export function getFullStackBranchName(root: string): string {
  return `${getEffectiveBranchPrefix()}/stack/${root}`
}

export const vcsRequestFactory = new VcsRequestFactory(() => getSelectedProject()?.path ?? "")

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

/** Checks the git setup of `project` and resolves its branch prefix; the sync does not wait: it resolves the prefix itself */
export function triggerBranchPrefixLoad(project: ProjectMetadata | null): void {
  void loadGitSetup(project)
  void loadBranchPrefix(project)
}

/**
 * Resolves the prefix of the selected repository again, keeping the one shown until Rust answers: on the window's
 * focus (a `git config` meanwhile), after a change of the app's prefix, and after a sync refused for it; resolves once
 * the page has the answer
 */
export function refreshBranchPrefix(): Promise<void> {
  return loadBranchPrefix(getSelectedProject())
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
  const failed = (error: string, errorDetails: string) => {
    if (!isStale()) {
      repositoryStore.setState({ pathValidation: { valid: false, path: currentPath, error, errorDetails }, branchPrefix: NO_BRANCH_PREFIX })
    }
  }

  try {
    const result = await commands.resolveBranchPrefix({ repositoryPath: currentPath || "" })
    if (result.status === "error") {
      // repository not accessible; the error message already contains context
      failed(result.error.message, result.error.message)
      return
    }

    if (project != null) {
      updateProject(project.path, { cachedBranchPrefix: result.data.prefix === "" ? undefined : result.data })
    }
    if (!isStale()) {
      // repository is accessible - clear any previous error; the same values keep their records (a focus asks again)
      repositoryStore.setState(({ branchPrefix, pathValidation }) => ({
        branchPrefix: branchPrefix?.prefix === result.data.prefix && branchPrefix.source === result.data.source ? branchPrefix : result.data,
        pathValidation: pathValidation.valid && pathValidation.error == null ? pathValidation : { valid: true },
      }))
    }
  } catch (error) {
    failed(`Repository not accessible: ${currentPath}`, getErrorDetails(error))
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

  // the app's prefix as the writer has it: a change of it, from any window, names the branches anew once Rust has it
  let appPrefix = appPrefixOf(snapshotValueOf("appSettings"))
  const stopListening = onWriterChange("appSettings", value => {
    const prefix = appPrefixOf(value)
    if (prefix !== appPrefix) {
      appPrefix = prefix
      void refreshBranchPrefix()
    }
  })

  let timer: ReturnType<typeof setTimeout> | undefined
  // only the path matters: cachedBranchPrefix updates must not trigger a reload
  const unsubscribe = repositorySettingsStore.subscribe((state, prevState) => {
    if (state.items[0]?.path === prevState.items[0]?.path) {
      return
    }

    // set synchronously so the UI does not render the "loaded" state of the new selection while the timer is pending; the
    // prefix of the repository left must not name the branches of the new one, its cached prefix does until the load
    repositoryStore.setState({ branchPrefix: null })
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
    stopListening()
    disposeRepository = null
  }
}

// a hot-reloaded module must not leave the previous subscription behind
import.meta.hot?.dispose(() => {
  disposeRepository?.()
})
