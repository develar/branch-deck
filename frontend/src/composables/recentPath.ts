import { Ref, ref } from "vue"

const recentsPathFile = "recent-paths.json"

import { Get as getConfig, Set as setConfig } from "../../wailsjs/go/wailsconfigstore/ConfigStore"
import { useToast } from "@nuxt/ui/composables/useToast"

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
    if (value) {
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
      await setConfig(recentsPathFile, JSON.stringify({ paths: value, selected: repositoryPath.value }))
      recentPaths.value = value
    } catch (error) {
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
    const data = JSON.parse(await getConfig(recentsPathFile, "null"))
    recentPaths.value = data.paths || []
    if (!repositoryPath.value && data.selected) {
      repositoryPath.value = data.selected
    }
  } catch (error) {
    console.error("Failed to load recent paths", error)
    toast.add({
      color: "error",
      title: "Failed to load recent paths",
      description: error.message,
    })
    recentPaths.value = []
  }
}
