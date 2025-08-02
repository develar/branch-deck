import { watchDebounced } from "@vueuse/core"
import { commands } from "~/utils/bindings"
import type { Result } from "~/utils/bindings"
import { appStore, type ProjectMetadata } from "~/utils/app-store"
import { VcsRequestFactory } from "~/composables/git/vcsRequest"
import { useAppSettingsStore } from "~/stores/appSettings"
import { getErrorDetails } from "#layers/shared-ui/utils/errorHandling"

// Debounce delay for persistence operations (ms)
const PERSISTENCE_DEBOUNCE_MS = 500

export interface PathValidation {
  valid: boolean
  path?: string

  error?: string | null
  errorDetails?: string | null
}

// Injection key
const RepositoryKey = Symbol("repository")

// Repository state interface
export interface RepositoryState {
  // State
  selectedProject: Ref<ProjectMetadata | null>
  recentProjects: Readonly<Ref<ProjectMetadata[]>>
  pathValidation: Readonly<Ref<PathValidation>>
  isValidatingPath: Readonly<Ref<boolean>>
  gitProvidedBranchPrefix: Readonly<Ref<Result<string, string>>>
  isLoadingBranchPrefix: Readonly<Ref<boolean>>
  loadingPromise: Readonly<Ref<Promise<void> | null>>

  // Computed
  effectiveBranchPrefix: ComputedRef<string>
  vcsRequestFactory: VcsRequestFactory
  issueNavigationConfig: Readonly<ComputedRef<IssueNavigationConfig | undefined>>

  // Actions
  browseRepository: () => Promise<void>
  getFullBranchName: (branchName: string) => string
}

// Create repository state
export function createRepositoryState(): RepositoryState {
  // State
  const recentProjects = ref<ProjectMetadata[]>([])
  const pathValidation = ref<PathValidation>({ valid: true })
  const isValidatingPath = ref(false)
  const gitProvidedBranchPrefix = ref<Result<string, string>>({ status: "error", error: "Not loaded" })
  const isLoadingBranchPrefix = ref(true)
  const loadingPromise = shallowRef<Promise<void> | null>(null)

  // App settings store for global settings
  const appSettingsStore = useAppSettingsStore()

  // Computed selected project (first in the list)
  const selectedProject = computed<ProjectMetadata | null>({
    get: () => recentProjects.value[0] || null,
    set: (project: ProjectMetadata | null) => {
      if (project) {
        // we validate explicitly on browse (and even if path is not valid anymore, it is ok - we will report on sync)
        // noinspection JSIgnoredPromiseFromCall
        selectProjectWithValidation(isReactive(project) ? project : reactive(project), { valid: true, error: undefined })
      }
      else {
        console.error("Selected project is null")
      }
    },
  })

  const effectiveBranchPrefix = computed(() =>
    appSettingsStore.globalUserBranchPrefix
    || selectedProject.value?.cachedBranchPrefix
    || (gitProvidedBranchPrefix.value.status === "ok" ? gitProvidedBranchPrefix.value.data : "")
    || "",
  )
  const vcsRequestFactory = new VcsRequestFactory(selectedProject, effectiveBranchPrefix)

  // Computed property for issue navigation config
  const issueNavigationConfig = computed(() =>
    selectedProject.value?.issueNavigationConfig,
  )

  // async function validatePath(path: string) {
  //   if (!path) {
  //     pathValidation.value = { valid: true }
  //     return
  //   }
  //
  //   isValidatingPath.value = true
  //   try {
  //     const result = await commands.validateRepositoryPath({ path })
  //     if (result.status === "ok") {
  //       // Empty string means valid, non-empty means error
  //       pathValidation.value = {
  //         valid: result.data === "",
  //         error: result.data || undefined,
  //       }
  //     }
  //     else {
  //       pathValidation.value = { valid: false, error: result.error }
  //     }
  //   }
  //   catch (error) {
  //     handleInternalErrorWithPathValidation(error, "Path validation", pathValidation)
  //   }
  //   finally {
  //     isValidatingPath.value = false
  //   }
  // }

  function selectProjectWithValidation(
    project: ProjectMetadata,
    preValidated: PathValidation,
  ) {
    pathValidation.value = preValidated
    if (!preValidated.valid) {
      // don't add invalid paths to recent projects
      return
    }

    const path = project.path
    // path is valid - update recent projects list
    const existingIndex = recentProjects.value.findIndex(p => p.path === path)
    if (existingIndex > 0) {
      // move existing project to front
      const projects = [...recentProjects.value]
      projects.splice(existingIndex, 1)
      projects.unshift(recentProjects.value[existingIndex]!)
      recentProjects.value = projects
    }
    else if (existingIndex === -1) {
      // add new project at front
      recentProjects.value = [project, ...recentProjects.value]
    }
    // If existingIndex === 0, it's already at the front
  }

  async function browseRepository() {
    try {
      const result = await commands.browseRepository()
      if (result.status === "ok") {
        const { path, valid, error } = result.data
        if (path) {
          if (error) {
            selectProjectWithValidation(reactive({ path }), { valid, path, error: `Invalid path: ${path}`, errorDetails: getErrorDetails(error) })
          }
          else {
            selectProjectWithValidation(reactive({ path }), { valid })
          }
        }
      }
      else {
        // Internal error (Result::Err from Rust)
        notifyInternalError(result.error, "Browse repository")
        pathValidation.value = { valid: false, error: `Internal error: ${result.error}` }
      }
    }
    catch (error) {
      // Unexpected error
      handleInternalErrorWithPathValidation(error, "Browse repository", pathValidation)
    }
  }

  function triggerBranchPrefixLoad(project: ProjectMetadata | null): void {
    const promise = loadBranchPrefix(project)
    loadingPromise.value = promise
    promise.finally(() => {
      if (loadingPromise.value === promise) {
        loadingPromise.value = null
      }
    })
  }

  async function loadBranchPrefix(project: ProjectMetadata | null) {
    const currentPath = project?.path

    isLoadingBranchPrefix.value = true
    try {
      // Pass empty string to get global config when no repository is selected
      const result = await commands.getBranchPrefixFromGitConfig({ repositoryPath: currentPath || "" })

      if (project != null) {
        project.cachedBranchPrefix = result.status === "ok" ? result.data : undefined
      }

      // check if project changed while we were loading
      if (selectedProject.value?.path === currentPath) {
        gitProvidedBranchPrefix.value = result
      }
    }
    catch (error) {
      // Check if path changed while we were loading
      if (selectedProject.value?.path !== currentPath) {
        return
      }

      notifyInternalError(error, "Load branch prefix")
      gitProvidedBranchPrefix.value = { status: "error", error: "Failed to load branch prefix" }
    }
    finally {
      // Only set loading to false if we're still on the same path
      if (selectedProject.value?.path === currentPath) {
        isLoadingBranchPrefix.value = false
      }
    }
  }

  async function initialize() {
    try {
      const projects = await appStore.get<ProjectMetadata[]>("recentProjects") ?? []
      recentProjects.value = projects.map(p => reactive(p))

      triggerBranchPrefixLoad(selectedProject.value)

      // watch for selected project changes to load branch prefix
      watchDebounced(selectedProject, (value) => {
        triggerBranchPrefixLoad(value)
      }, { flush: "post", debounce: 50 })

      setupRepositoryPersistence(recentProjects)
    }
    catch (error) {
      notifyError("Failed to load recent paths", error, useToast())
      recentProjects.value = []
    }
  }

  // Initialize on creation
  // noinspection JSIgnoredPromiseFromCall
  initialize()

  // Utility function to get full branch name with prefix
  const getFullBranchName = (branchName: string) => {
    return `${effectiveBranchPrefix.value}/virtual/${branchName}`
  }

  return {
    // State
    selectedProject,
    recentProjects: readonly(recentProjects) as Readonly<Ref<ProjectMetadata[]>>,
    pathValidation: readonly(pathValidation),
    isValidatingPath: readonly(isValidatingPath),
    gitProvidedBranchPrefix: readonly(gitProvidedBranchPrefix),
    isLoadingBranchPrefix: readonly(isLoadingBranchPrefix),
    loadingPromise: readonly(loadingPromise),

    // Computed
    effectiveBranchPrefix,
    vcsRequestFactory,
    issueNavigationConfig: issueNavigationConfig,

    // Actions
    browseRepository,
    getFullBranchName,
  }
}

// Provide repository state
export function provideRepository() {
  const state = createRepositoryState()
  provide(RepositoryKey, state)
  return state
}

// Use repository state
export function useRepository(): RepositoryState {
  const state = inject<RepositoryState>(RepositoryKey)
  if (!state) {
    throw new Error("Repository state not provided. Make sure to call provideRepository() at the app level.")
  }
  return state
}

function setupRepositoryPersistence(
  recentProjects: Ref<ProjectMetadata[]>,
) {
  // Watch recent projects
  watchDebounced(
    recentProjects,
    async (newProjects) => {
      try {
        await appStore.set("recentProjects", newProjects.length ? newProjects : null)
      }
      catch (error) {
        logError("Failed to save recent projects", error)
      }
    },
    {
      debounce: PERSISTENCE_DEBOUNCE_MS,
      deep: true,
      flush: "post",
    },
  )
}

// Helper for internal errors that need to be shown via pathValidation
function handleInternalErrorWithPathValidation(
  error: unknown,
  context: string,
  pathValidation: Ref<PathValidation>,
): void {
  notifyInternalError(error, context)

  const title = `Internal error: ${context} failed`
  const errorDetails = getErrorDetails(error)
  // noinspection JSIgnoredPromiseFromCall
  logError(title, error, errorDetails)

  pathValidation.value = {
    valid: false,
    error: title,
    errorDetails,
  }
}
