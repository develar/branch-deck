import { watchDebounced } from "@vueuse/core"
import { commands } from "~/utils/bindings"
import type { Result } from "~/utils/bindings"
import { VcsRequestFactory } from "~/composables/git/vcsRequest"
import { useAppSettingsStore } from "~/stores/appSettings"
import { repositorySettings, type ProjectMetadata } from "~/stores/repositorySettings"
import { getErrorDetails } from "#layers/shared-ui/utils/errorHandling"

export interface PathValidation {
  valid: boolean
  path?: string

  error?: string | null
  errorDetails?: string | null
}

// Injection key
const RepositoryKey = Symbol("repository")

// Create repository state
export function createRepositoryState() {
  // Repository settings is now synchronous

  // State
  const pathValidation = ref<PathValidation>({ valid: true })
  const isValidatingPath = ref(false)
  const gitProvidedBranchPrefix = ref<Result<string, string>>({ status: "error", error: "Not loaded" })
  const isLoadingBranchPrefix = ref(true)
  const loadingPromise = shallowRef<Promise<void> | null>(null)

  // App settings store for global settings
  const appSettingsStore = useAppSettingsStore()

  // Computed selected project (first in the list)
  const selectedProject = computed<ProjectMetadata | null>({
    get: () => repositorySettings[0] || null,
    set: (project: ProjectMetadata | null) => {
      if (project) {
        // we validate explicitly on browse (and even if path is not valid anymore, it is ok - we will report on sync)
        // noinspection JSIgnoredPromiseFromCall
        selectProjectWithValidation(project, { valid: true, error: undefined })
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
    const existingIndex = repositorySettings.findIndex(p => p.path === path)
    if (existingIndex > 0) {
      // move existing project to front
      const projects = [...repositorySettings]
      projects.splice(existingIndex, 1)
      projects.unshift(repositorySettings[existingIndex]!)
      repositorySettings.splice(0, repositorySettings.length, ...projects)
    }
    else if (existingIndex === -1) {
      // add new project at front
      repositorySettings.unshift(project)
    }
    // If existingIndex === 0, it's already at the front
  }

  async function browseRepository() {
    try {
      const result = await commands.browseRepository()
      if (result.status === "ok") {
        const { path, valid, error } = result.data
        if (path) {
          // Check if this project already exists in our list
          const existingProject = repositorySettings.find(p => p.path === path)
          const project: ProjectMetadata = reactive(existingProject || { path, cachedBranchPrefix: undefined })

          if (error) {
            selectProjectWithValidation(project, { valid, path, error: `Invalid path: ${path}`, errorDetails: getErrorDetails(error) })
          }
          else {
            selectProjectWithValidation(project, { valid })
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
      // Try to get branch prefix for the specific repository path
      const result = await commands.getBranchPrefixFromGitConfig({ repositoryPath: currentPath || "" })

      // Check if the Tauri command returned an error
      if (result.status === "error") {
        // Check if path changed while we were loading
        if (selectedProject.value?.path !== currentPath) {
          return
        }

        // Repository not accessible - update pathValidation for UI feedback
        // Use the error message from the result directly (it already contains context)
        pathValidation.value = {
          valid: false,
          path: currentPath,
          error: result.error,
          errorDetails: result.error,
        }

        // Set empty prefix - error is already in pathValidation for UI display
        gitProvidedBranchPrefix.value = { status: "ok", data: "" }
        return
      }

      if (project != null) {
        project.cachedBranchPrefix = result.status === "ok" && result.data ? result.data : undefined
      }

      // check if project changed while we were loading
      if (selectedProject.value?.path === currentPath) {
        gitProvidedBranchPrefix.value = result
        // Repository is accessible - clear any previous errors
        pathValidation.value = { valid: true }
      }
    }
    catch (error) {
      // Check if path changed while we were loading
      if (selectedProject.value?.path !== currentPath) {
        return
      }

      // JavaScript exception (unexpected error) - update pathValidation for UI feedback
      pathValidation.value = {
        valid: false,
        path: currentPath,
        error: `Repository not accessible: ${currentPath}`,
        errorDetails: getErrorDetails(error),
      }

      // Set empty prefix - error is already in pathValidation for UI display
      gitProvidedBranchPrefix.value = { status: "ok", data: "" }
    }
    finally {
      // Only set loading to false if we're still on the same path
      if (selectedProject.value?.path === currentPath) {
        isLoadingBranchPrefix.value = false
      }
    }
  }

  // Initialize branch prefix loading for selected project
  triggerBranchPrefixLoad(selectedProject.value)

  // watch for selected project path changes to load branch prefix
  // Only watch the path to avoid triggering when cachedBranchPrefix changes
  watchDebounced(() => selectedProject.value?.path, (path, oldPath) => {
    // Only trigger if path actually changed
    if (path === oldPath) {
      return
    }

    // Always call backend - it handles invalid paths gracefully with global fallback
    triggerBranchPrefixLoad(selectedProject.value)
  }, { flush: "post", debounce: 50 })

  // Utility function to get full branch name with prefix
  const getFullBranchName = (branchName: string) => {
    return `${effectiveBranchPrefix.value}/virtual/${branchName}`
  }

  return {
    // State
    selectedProject,
    recentProjects: readonly(toRef(() => repositorySettings)),
    pathValidation: readonly(pathValidation),
    isValidatingPath: readonly(isValidatingPath),
    gitProvidedBranchPrefix: readonly(gitProvidedBranchPrefix),
    isLoadingBranchPrefix: readonly(isLoadingBranchPrefix),
    loadingPromise: readonly(loadingPromise),

    // Computed
    effectiveBranchPrefix,
    vcsRequestFactory,
    issueNavigationConfig,

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
export function useRepository() {
  const state = inject<ReturnType<typeof createRepositoryState>>(RepositoryKey)
  if (!state) {
    throw new Error("Repository state not provided. Make sure to call provideRepository() at the app level.")
  }
  return state
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
