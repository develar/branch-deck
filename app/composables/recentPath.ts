import type { Ref } from "vue"
import { ref, inject } from "vue"
import { appStoreKey } from "~/utils/app-store"

export interface RecentPath {
  readonly paths: string

  readonly selected?: string
}

export function useRecentPath() {
  const appStore = inject(appStoreKey)
  if (!appStore) {
    throw new Error("AppStore not provided")
  }

  const recentPaths = ref<string[]>([])
  const repositoryPath = ref("")

  // noinspection JSIgnoredPromiseFromCall
  loadRecentPaths(appStore, recentPaths, repositoryPath)

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

  return {
    recentPaths,
    onRepositoryPathChange,
    addToRecentPaths,
    repositoryPath,
  }
}

async function loadRecentPaths(appStore: import("~/utils/app-store").IAppStore, recentPaths: Ref<string[]>, repositoryPath: Ref<string>) {
  const toast = useToast()
  try {
    recentPaths.value = await appStore.getRecentPaths()
    if (!repositoryPath.value) {
      repositoryPath.value = await appStore.getSelectedProject()
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
