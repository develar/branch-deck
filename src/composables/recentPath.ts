import { Ref, ref } from "vue"
import { useToast } from "@nuxt/ui/composables/useToast"
import { LazyStore } from "@tauri-apps/plugin-store"

const store = new LazyStore("settings.json")

export interface RecentPath {
  readonly paths: string

  readonly selected?: string
}

export function useRecentPath() {
  const recentPaths = ref<string[]>([])
  const repositoryPath = ref("")

  // noinspection JSIgnoredPromiseFromCall
  loadRecentPaths(recentPaths, repositoryPath)

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
      await store.set("recentsPaths", value)
      await store.set("selectedProject", repositoryPath.value)
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

async function loadRecentPaths(recentPaths: Ref<string[]>, repositoryPath: Ref<string>) {
  const toast = useToast()
  try {
    recentPaths.value = await store.get<string[]>("recentsPaths") ?? []
    if (!repositoryPath.value) {
      repositoryPath.value = await store.get<string>("selectedProject") ?? ""
    }
  }
  catch (error) {
    console.error("Failed to load recent paths", error)
    toast.add({
      color: "error",
      title: "Failed to load recent paths",
      description: error.message,
    })
    recentPaths.value = []
  }
}
