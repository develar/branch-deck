import { watchDebounced } from "@vueuse/core"
import { commands } from "~/utils/bindings"
import { appStoreKey } from "~/utils/app-store"

export interface PathValidation {
  valid: boolean
  error?: string
}

export function useRepositoryPath() {
  const appStore = inject(appStoreKey)
  if (!appStore) {
    throw new Error("AppStore not provided")
  }

  // State
  const recentPaths = ref<string[]>([])
  const repositoryPath = ref("")
  const pathValidation = ref<PathValidation>({ valid: true })
  const isValidatingPath = ref(false)

  // Load recent paths on mount
  loadRecentPaths()

  // Watch for repository path changes and validate
  watchDebounced(
    repositoryPath,
    async (newPath: string) => {
      if (!newPath?.trim()) {
        pathValidation.value = { valid: true }
        return
      }

      isValidatingPath.value = true
      try {
        const result = await commands.validateRepositoryPath(newPath)
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
        console.error("Path validation error:", error)
        pathValidation.value = { valid: false, error: "Failed to validate path" }
      }
      finally {
        isValidatingPath.value = false
      }
    },
    { debounce: 300 },
  )

  // Methods
  const onRepositoryPathChange = async (value: string | null) => {
    const sanitizedValue = value?.trim()
    if (sanitizedValue) {
      await addToRecentPaths(sanitizedValue)
    }
  }

  const addToRecentPaths = async (path: string) => {
    try {
      const value = recentPaths.value.slice()
      if (!recentPaths.value.includes(path)) {
        value.push(path)
        value.sort()
        recentPaths.value = value
      }
      await appStore.setRecentPaths(value)
      await appStore.setSelectedProject(repositoryPath.value)
      recentPaths.value = value
    }
    catch (error) {
      console.error("Failed to save recent path:", error)
    }
  }

  const browseRepository = async () => {
    try {
      const result = await commands.browseRepository()
      if (result.status === "ok") {
        const { path, valid, error } = result.data

        if (path) {
          // User selected a path
          repositoryPath.value = path

          if (valid) {
            // Path is valid - add to recent and clear errors
            await addToRecentPaths(path)
            pathValidation.value = { valid: true }
          }
          else {
            // Path is invalid - show validation error
            pathValidation.value = { valid: false, error: error || "Invalid repository path" }
          }
        }
        // If no path, user cancelled - do nothing
      }
      else {
        // Internal error (Result::Err from Rust)
        const { reportError } = await import("~/utils/errorHandling")
        reportError("Failed to open folder browser", result.error, useToast())
      }
    }
    catch (error) {
      // Unexpected error
      const { reportError } = await import("~/utils/errorHandling")
      reportError("Failed to browse repository", error, useToast())
    }
  }

  async function loadRecentPaths() {
    const toast = useToast()
    try {
      recentPaths.value = await appStore!.getRecentPaths()
      if (!repositoryPath.value) {
        repositoryPath.value = await appStore!.getSelectedProject()
      }
    }
    catch (error) {
      console.error("Failed to load recent paths", error)
      toast.add({
        color: "error",
        title: "Failed to load recent paths",
        description: error instanceof Error ? error.message : String(error),
      })
      recentPaths.value = []
    }
  }

  return {
    // State
    repositoryPath,
    recentPaths,
    pathValidation: readonly(pathValidation),
    isValidatingPath: readonly(isValidatingPath),

    // Methods
    onRepositoryPathChange,
    addToRecentPaths,
    browseRepository,
  }
}