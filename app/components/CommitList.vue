<template>
  <div class="divide-y divide-default">
    <div
      v-for="commit in commits.values()"
      :key="commit.original_hash"
      class="flex items-center justify-between px-6 py-3 hover:bg-muted transition-colors"
    >
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2">
          <!-- Status indicator -->
          <div class="flex-shrink-0">
            <UTooltip
              v-if="getCommitStatusIcon(commit.status)"
              :text="getCommitStatusText(commit.status, commit.error)"
            >
              <UIcon
                :name="getCommitStatusIcon(commit.status)"
                :class="getCommitStatusClass(commit.status)"
                class="h-4 w-4"
              />
            </UTooltip>
            <UTooltip
              v-else-if="commit.status === 'Syncing'"
              text="Syncing"
            >
              <UProgress
                size="xs"
                indeterminate
              />
            </UTooltip>
          </div>

          <!-- Commit message -->
          <p class="text-sm font-medium text-highlighted truncate">
            {{ commit.message }}
          </p>
        </div>

        <div class="mt-1 flex items-center gap-4 text-xs text-muted">
          <!-- Original hash -->
          <span class="font-mono">{{ commit.original_hash.substring(0, 7) }}</span>

          <!-- New hash if synced -->
          <span v-if="commit.hash" class="font-mono">
            → {{ commit.hash.substring(0, 7) }}
          </span>

          <!-- Timestamp -->
          <span>{{ formatTimestamp(commit.time) }}</span>

          <!-- Status text -->
          <span :class="getCommitStatusClass(commit.status)">
            {{ getCommitStatusText(commit.status, commit.error) }}
          </span>
        </div>

        <!-- Error details if any -->
        <div v-if="commit.error" class="mt-2">
          <MergeConflictViewer v-if="commit.error.MergeConflict" :conflict="commit.error.MergeConflict" :branch-name="branchName" />
          <UAlert
            v-else-if="commit.error.Generic"
            color="error"
            variant="soft"
            size="xs"
          >
            <template #description>
              <p class="text-xs">
                {{ commit.error.Generic }}
              </p>
            </template>
          </UAlert>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import type { CommitDetail, CommitSyncStatus, BranchError } from "~/utils/bindings"
import { formatTimestamp } from "~/utils/time"

defineProps<{
  commits: Map<string, CommitDetail>
  branchName?: string
}>()

function getCommitStatusIcon(status: CommitSyncStatus): string | null {
  switch (status) {
    case "Pending":
      return "i-lucide-loader-circle"
    case "Error":
      return "i-lucide-x-circle"
    case "Blocked":
      return "i-lucide-minus-circle"
    case "Created":
      return "i-lucide-check-circle"
    case "Unchanged":
      return "i-lucide-check"
    default:
      return null
  }
}

function getCommitStatusClass(status: CommitSyncStatus): string {
  switch (status) {
    case "Pending":
      return "text-dimmed"
    case "Syncing":
      return "text-info"
    case "Error":
      return "text-error"
    case "Blocked":
      return "text-warning"
    case "Created":
      return "text-success"
    case "Unchanged":
      return "text-muted"
    default:
      return ""
  }
}

function getCommitStatusText(status: CommitSyncStatus, error?: BranchError | null): string {
  if (status === "Error" && error && "MergeConflict" in error) {
    return "Merge Conflict"
  }
  return status
}
</script>
