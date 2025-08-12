<template>
  <div class="ml-4 border-l-2 border-primary/50 pl-2 pr-6">
    <!-- Loading state -->
    <div
      v-if="branch.isLoadingCommits"
      class="flex items-center gap-2 text-muted text-sm"
    >
      <UIcon name="i-lucide-loader-2" class="size-4 animate-spin" />
      Loading commits…
    </div>

    <!-- Error states -->
    <div v-else-if="branch.loadError" class="text-error">
      Failed to load commits: {{ branch.loadError }}
    </div>

    <!-- Commit list -->
    <CommitList
      v-else-if="branch.commits && branch.commits.length > 0"
      :commits="branch.commits"
      variant="compact"
      :show-file-count="false"
    />

    <!-- No commits found -->
    <div v-else class="text-muted text-sm">
      No commits found
    </div>
  </div>
</template>

<script lang="ts" setup>

const props = defineProps<{
  branch: ReactiveArchivedBranch
}>()

// Load commits when component mounts and commits aren't loaded yet
onMounted(async () => {
  if (!props.branch.hasLoadedCommits && !props.branch.isLoadingCommits) {
    const { archivedBranches } = useBranchSync()
    await archivedBranches.loadCommitsForBranch(props.branch.name)
  }
})
</script>