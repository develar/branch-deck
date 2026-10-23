import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { internalError } from "@/lib/api-error"
import { commands } from "@/lib/bindings"
import type { GitBranchPrefix, GitSetup } from "@/lib/bindings"
import { normalizeBranchPrefix } from "@/lib/branch-prefix"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { VcsRequestFactory } from "@/lib/git/vcs-request"
import { repositoryName } from "@/lib/repository-config"
import { appSettingsStore, useAppSettings } from "@/stores/settings/appSettings"
import { getRecentProjects, repositorySettingsStore, setRecentProjects, updateProject, useRecentProjects } from "@/stores/settings/repositorySettings"
import type { ProjectMetadata } from "@/stores/settings/repositorySettings"

/** The prefix of git for the selected repository (`getBranchPrefixFromGitConfig`), with whether it is the repository's own */
export type BranchPrefixResult = Awaited<ReturnType<typeof commands.getBranchPrefixFromGitConfig>>

/** Before the first load of the selected repository answers: the prefix cached for it applies */
const NOT_LOADED: BranchPrefixResult = { status: "error", error: internalError("Not loaded") }

/** A repository git cannot read has no git prefix; the error is shown through `pathValidation` */
const NO_GIT_PREFIX: BranchPrefixResult = { status: "ok", data: { prefix: "", repositoryScoped: false } }

export interface PathValidation {
  valid: boolean
  path?: string

  error?: string | null
  errorDetails?: string | null
}

export interface RepositoryState {
  pathValidation: PathValidation
  /** `branchdeck.branchPrefix` of the selected repository as git resolves it (or of the global config), and its scope */
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
  gitProvidedBranchPrefix: NOT_LOADED,
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

/** One field of the selected repository's metadata; re-renders only when it changes */
export function useSelectedProjectField<K extends keyof ProjectMetadata>(field: K): ProjectMetadata[K] | undefined {
  return useRecentProjects(projects => projects[0]?.[field])
}

// ---- effective branch prefix ----

/**
 * The git prefix of the repository: the one the load read, else the one cached for the repository until the load
 * answers
 */
function gitBranchPrefixOf(cachedBranchPrefix: GitBranchPrefix | undefined, gitProvidedBranchPrefix: BranchPrefixResult): GitBranchPrefix | undefined {
  return gitProvidedBranchPrefix.status === "ok" ? gitProvidedBranchPrefix.data : cachedBranchPrefix
}

/** The repository's own prefix (its local or worktree git config, or `git -c`), which wins over the app's; empty without one */
export function repositoryOwnBranchPrefix(gitPrefix: GitBranchPrefix | undefined): string {
  return gitPrefix?.repositoryScoped === true ? normalizeBranchPrefix(gitPrefix.prefix) : ""
}

/**
 * The branch prefix of a repository, the specific one first, as `resolve_branch_prefix` of the backend (`bd` too): the
 * repository's own git prefix, else the app's prefix (`globalUserBranchPrefix`), else the git prefix of the global or
 * system config. The git prefix is the one cached for the repository until its load answers.
 */
export function computeEffectiveBranchPrefix(
  globalUserBranchPrefix: string | undefined,
  cachedBranchPrefix: GitBranchPrefix | undefined,
  gitProvidedBranchPrefix: BranchPrefixResult,
): string {
  const gitPrefix = gitBranchPrefixOf(cachedBranchPrefix, gitProvidedBranchPrefix)
  return repositoryOwnBranchPrefix(gitPrefix) || normalizeBranchPrefix(globalUserBranchPrefix) || normalizeBranchPrefix(gitPrefix?.prefix)
}

/** The prefix the branches of the selected repository are named with ({@link computeEffectiveBranchPrefix}) */
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

/** Full git branch name of the stack branch of a stack that forks, all its commits in one branch: `john/stack/auth` */
export function getFullStackBranchName(root: string): string {
  return `${getEffectiveBranchPrefix()}/stack/${root}`
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
        gitProvidedBranchPrefix: NO_GIT_PREFIX,
      })
      return
    }

    if (project != null) {
      updateProject(project.path, { cachedBranchPrefix: result.data.prefix === "" ? undefined : result.data })
    }

    if (!isStale()) {
      // repository is accessible - clear any previous error
      repositoryStore.setState({ gitProvidedBranchPrefix: result, pathValidation: { valid: true } })
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
      gitProvidedBranchPrefix: NO_GIT_PREFIX,
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

    // set synchronously so the UI does not render the "loaded" state of the new selection while the timer is pending; the
    // prefix of the repository left must not name the branches of the new one, its cached prefix does until the load
    repositoryStore.setState({ isLoadingBranchPrefix: true, gitProvidedBranchPrefix: NOT_LOADED })
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
