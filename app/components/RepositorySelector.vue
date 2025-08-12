<template>
  <UTooltip :text="pathValidation.error || 'Select a Git repository to manage branches'">
    <UFieldGroup>
      <USelect
        v-model="selectedProjectModel"
        :disabled="disabled"
        :items="projectItems"
        :loading="isValidatingPath"
        :color="pathValidation.error ? 'error' : 'primary'"
        placeholder="Select repository..."
        class="w-3xs"
        size="sm"
      >
        <template #item-label="{ item }">
          <div class="truncate-none">
            <div class="whitespace-nowrap">
              {{ formatPathWithTruncation(item.project.path) }}
            </div>
            <div v-if="item.error" class="text-error text-xs truncate">
              {{ item.error }}
            </div>
            <div v-else-if="item.project.lastSyncTime" class="text-xs text-muted whitespace-nowrap">
              {{ item.project.lastBranchCount === 1 ? "1 branch" : `${item.project.lastBranchCount || 0} branches` }}
              • {{ formatRelativeTime(item.project.lastSyncTime) }}
            </div>
          </div>
        </template>
      </USelect>
      <UButton
        :disabled="disabled"
        icon="i-lucide-folder-search"
        variant="outline"
        :color="pathValidation.error ? 'error' : 'primary'"
        size="sm"
        data-testid="browse-repository-button"
        @click="handleBrowse"
      />
    </UFieldGroup>
  </UTooltip>
</template>

<script lang="ts" setup>
import type { ProjectMetadata } from "~/stores/repositorySettings"

defineProps<{
  disabled?: boolean
}>()

const { isValidatingPath, pathValidation, recentProjects, selectedProject, browseRepository } = useRepository()

interface ProjectItem {
  label: string
  value: string
  error?: string | null
  project: ProjectMetadata
}

// Transform ProjectMetadata to proper Select item format
const projectItems: ComputedRef<Array<ProjectItem>> = computed(() => {
  const result: Array<ProjectItem> = recentProjects.value.map(project => ({
    label: formatPathWithTruncation(project.path),
    value: project.path,
    error: undefined,
    project,
  }))

  // If there's a validation error with a path, show that path
  const invalid = pathValidation.value
  if (!invalid.valid && invalid.path) {
    const invalidItem: ProjectItem = { label: formatPathWithTruncation(invalid.path), value: invalid.path, project: { path: invalid.path, cachedBranchPrefix: undefined }, error: invalid.errorDetails }
    return [invalidItem].concat(result)
  }
  else {
    return result
  }
})

const selectedProjectModel = computed({
  get: () => {
    // If there's a validation error with a path, show that path
    const invalid = pathValidation.value
    if (!invalid.valid && invalid.path) {
      return invalid.path
    }

    // Otherwise show the selected project path
    if (!selectedProject.value) {
      return undefined
    }
    // Return just the path string for v-model
    return selectedProject.value.path
  },
  set: (value: string | null) => {
    if (value) {
      // Check if it's an existing project from our items
      const existingItem = projectItems.value.find(item => item.value === value)
      if (existingItem) {
        // Use the project from the existing item directly
        selectedProject.value = existingItem.project
      }
      else {
        // It's a new path typed by the user (creatable)
        selectedProject.value = { path: value, cachedBranchPrefix: undefined }
      }
    }
    else {
      selectedProject.value = null
    }
  },
})

const handleBrowse = async () => {
  await browseRepository()
}

// Store home directory
const homeDir = ref<string>("")

// Load home directory on mount
onMounted(async () => {
  const { homeDir: getHomeDir } = await import("@tauri-apps/api/path")
  homeDir.value = await getHomeDir()
})

// Format path to replace home directory with ~
const formatPath = (path: string): string => {
  if (homeDir.value && path.startsWith(homeDir.value)) {
    return path.replace(homeDir.value, "~")
  }
  return path
}

// Format path with smart truncation for dropdown display
const formatPathWithTruncation = (path: string, maxLength: number = 36): string => {
  const formatted = formatPath(path)

  if (formatted.length <= maxLength) {
    return formatted
  }

  // Split path into segments
  const segments = formatted.split("/").filter(s => s.length > 0)

  // Handle edge case of empty segments
  if (segments.length === 0) {
    return formatted
  }

  const projectName = segments[segments.length - 1]!

  // If project name alone is too long, truncate it
  if (projectName.length > maxLength - 3) {
    return "..." + projectName.slice(-(maxLength - 3))
  }

  // Build path from end, keeping as much as possible
  let result = projectName
  let i = segments.length - 2

  while (i >= 0) {
    const newSegment = segments[i]! + "/"
    // Reserve space for ".../" at the beginning
    if (result.length + newSegment.length + 4 > maxLength) {
      // Add what we can from the beginning
      const remainingSpace = maxLength - result.length - 4
      if (remainingSpace > 0 && i === 0 && segments[0]!.length <= remainingSpace) {
        // If it's the first segment and it fits, include it
        result = segments[0]! + "/.../" + result
      }
      else {
        result = ".../" + result
      }
      break
    }
    result = newSegment + result
    i--
  }

  return result
}
</script>
