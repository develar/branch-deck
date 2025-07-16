import { defineStore } from "pinia"
import { commands } from "~/utils/bindings"
import type { Result } from "~/utils/bindings"
import { appStore } from "~/utils/app-store"
import { VcsRequestFactory } from "~/composables/git/vcsRequest"
// notifyInternalError is auto-imported from shared-ui layer

export interface PathValidation {
  valid: boolean
  error?: string
}

export const useRepositoryStore = defineStore("repository", () => {
  // State
  const repositoryPath = ref("")
  const recentPaths = ref<string[]>([])
  const pathValidation = ref<PathValidation>({ valid: true })
  const isValidatingPath = ref(false)
  const branchPrefix = ref("")
  const gitProvidedBranchPrefix = ref<Result<string, string>>({ status: "ok", data: "" })

  // Computed
  const vcsRequestFactory = computed(() =>
    new VcsRequestFactory(repositoryPath, branchPrefix),
  )

  // Actions
  async function setRepositoryPath(path: string | null) {
    const sanitizedValue = path?.trim() || ""
    repositoryPath.value = sanitizedValue

    if (sanitizedValue) {
      await validatePath(sanitizedValue)
      await addToRecentPaths(sanitizedValue)
      await appStore.setSelectedProject(sanitizedValue)
    }
    else {
      pathValidation.value = { valid: true }
    }
  }

  async function validatePath(path: string) {
    if (!path) {
      pathValidation.value = { valid: true }
      return
    }

    isValidatingPath.value = true
    try {
      const result = await commands.validateRepositoryPath(path)
      if (result.status === "ok") {
        // Empty string means valid, non-empty means error
        pathValidation.value = {
          valid: result.data === "",
          error: result.data || undefined,
        }
      }
      else {
        pathValidation.value = { valid: false, error: result.error }
      }
    }
    catch (error) {
      notifyInternalError(error, "Path validation")
      pathValidation.value = { valid: false, error: "Failed to validate path" }
    }
    finally {
      isValidatingPath.value = false
    }
  }

  async function addToRecentPaths(path: string) {
    try {
      const paths = recentPaths.value.slice()
      if (!paths.includes(path)) {
        paths.push(path)
        paths.sort()
        recentPaths.value = paths
        await appStore.setRecentPaths(paths)
      }
    }
    catch (error) {
      notifyInternalError(error, "Save recent path")
    }
  }

  async function browseRepository() {
    try {
      const result = await commands.browseRepository()
      if (result.status === "ok") {
        const { path, valid, error } = result.data

        if (path) {
          // User selected a path
          await setRepositoryPath(path)

          if (!valid) {
            // Path is invalid - show validation error
            pathValidation.value = { valid: false, error: error || "Invalid repository path" }
          }
        }
        // If no path, user cancelled - do nothing
      }
      else {
        // Internal error (Result::Err from Rust)
        notifyError("Failed to open folder browser", result.error, useToast())
      }
    }
    catch (error) {
      // Unexpected error
      notifyError("Failed to browse repository", error, useToast())
    }
  }

  async function loadBranchPrefix() {
    if (!repositoryPath.value) {
      gitProvidedBranchPrefix.value = { status: "ok", data: "" }
      return
    }

    try {
      gitProvidedBranchPrefix.value = await commands.getBranchPrefixFromGitConfig(repositoryPath.value)

      // If we got a prefix from git config, use it
      if (gitProvidedBranchPrefix.value.status === "ok" && gitProvidedBranchPrefix.value.data) {
        branchPrefix.value = gitProvidedBranchPrefix.value.data
      }
    }
    catch (error) {
      notifyInternalError(error, "Load branch prefix")
      gitProvidedBranchPrefix.value = { status: "error", error: "Failed to load branch prefix" }
    }
  }

  async function initialize() {
    const toast = useToast()
    try {
      recentPaths.value = await appStore.getRecentPaths()
      const selectedProject = await appStore.getSelectedProject()
      if (selectedProject) {
        await setRepositoryPath(selectedProject)
      }
    }
    catch (error) {
      notifyInternalError(error, "Load recent paths")
      toast.add({
        color: "error",
        title: "Failed to load recent paths",
        description: error instanceof Error ? error.message : String(error),
      })
      recentPaths.value = []
    }
  }

  // Watch for repository path changes to load branch prefix
  watch(repositoryPath, () => {
    loadBranchPrefix()
  })

  // Initialize on store creation
  onMounted(() => {
    initialize()
  })

  return {
    // State
    repositoryPath: readonly(repositoryPath),
    recentPaths,
    pathValidation: readonly(pathValidation),
    isValidatingPath: readonly(isValidatingPath),
    branchPrefix,
    gitProvidedBranchPrefix: readonly(gitProvidedBranchPrefix),

    // Computed
    vcsRequestFactory,

    // Actions
    setRepositoryPath,
    browseRepository,
  }
})