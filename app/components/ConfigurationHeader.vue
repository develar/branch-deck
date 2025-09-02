<template>
  <div class="bg-elevated backdrop-blur-sm rounded-lg border border-default" data-testid="configuration-header">
    <div class="h-14 px-4 flex items-center justify-between gap-4">
      <!-- Left group: Repository info -->
      <div class="flex items-center gap-3 min-w-0">
        <RepositorySelector
          :disabled="isSyncing"
        />

        <BranchPrefixField
          :disabled="isSyncing || !pathValidation.valid"
        />

        <div
          v-if="selectedProject && branchCount > 0 && pathValidation.valid"
          class="hidden sm:flex items-center gap-3 text-xs text-muted"
        >
          <span class="whitespace-nowrap">
            {{ branchCount }} {{ branchCount === 1 ? 'branch' : 'branches' }}
          </span>
        </div>
      </div>

      <!-- Right group: Actions -->
      <div class="flex items-center gap-3">
        <div v-if="lastSyncTime && !isSyncing && pathValidation.valid" class="text-xs text-muted">
          Last synced {{ formatRelativeTime(lastSyncTime) }}
        </div>
        <UTooltip v-if="syncError" :text="syncError">
          <UIcon name="i-lucide-alert-circle" class="size-4 text-error" />
        </UTooltip>
        <SyncButton />
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
const { pathValidation, selectedProject } = useRepository()
const { isSyncing, syncError, branches } = useBranchSync()

// Computed properties for cleaner template
const branchCount = computed(() => branches.value.length)
const lastSyncTime = computed(() => selectedProject.value?.lastSyncTime)
</script>
