<template>
  <div :class="computedContainerClass">
    <div
      v-for="(commit, index) in normalizedCommits"
      :key="('originalHash' in commit ? commit.originalHash : undefined) || commit.hash"
      :class="computedItemClass"
    >
      <div class="flex-1 min-w-0">
        <!-- Commit message with optional badge -->
        <div class="flex items-center gap-2">
          <p :class="computedMessageClass">
            {{ commit.message }}
          </p>

          <!-- Status badge for exceptional states (only in status variant) -->
          <template v-if="variant === 'status' && 'status' in commit && isExceptionalStatus(commit.status)">
            <UBadge v-if="'status' in commit && commit.status === 'Pending'" size="xs" variant="subtle">
              <UIcon name="i-lucide-loader-circle" class="animate-spin mr-1 size-3" />
              Pending
            </UBadge>
            <UBadge
v-else-if="'status' in commit && commit.status === 'Error'"
color="error"
size="xs"
variant="subtle">
              Error
            </UBadge>
            <UBadge
v-else-if="'status' in commit && commit.status === 'Blocked'"
color="warning"
size="xs"
variant="subtle">
              Blocked
            </UBadge>
          </template>
        </div>

        <!-- Metadata line -->
        <div class="mt-1 flex items-center gap-2 text-xs text-muted">
          <!-- Hash(es) -->
          <span class="font-mono">{{ formatShortHash(('originalHash' in commit ? commit.originalHash : undefined) || commit.hash!) }}</span>

          <template v-if="variant === 'status' && commit.hash">
            <span>→</span>
            <span class="font-mono">{{ formatShortHash(commit.hash) }}</span>
          </template>

          <!-- Author (if enabled) -->
          <template v-if="showAuthor && 'author' in commit && commit.author">
            <span>•</span>
            <span>{{ commit.author }}</span>
          </template>

          <!-- Timestamp -->
          <template v-if="commit.authorTime">
            <span>•</span>
            <TimestampWithPopover
              v-if="commit.committerTime"
              :author-time="commit.authorTime"
              :committer-time="commit.committerTime"
            />
            <span v-else>{{ formatTimestamp(commit.authorTime) }}</span>
          </template>

          <!-- File count (if enabled) -->
          <template v-if="showFileCount && getFileCount(commit)">
            <span>•</span>
            <span>{{ getFileCount(commit) }} {{ getFileCount(commit) === 1 ? 'file' : 'files' }}</span>
          </template>

          <!-- Status text (only show for non-common statuses) -->
          <template v-if="variant === 'status' && 'status' in commit && commit.status && shouldShowStatusText(commit.status)">
            <span>•</span>
            <span :class="getCommitStatusClass(commit.status)">
              {{ getCommitStatusText(commit.status, 'error' in commit ? commit.error : undefined) }}
            </span>
          </template>
        </div>

        <!-- Error details (status variant only) -->
        <div v-if="variant === 'status' && 'error' in commit && commit.error" class="mt-2">
          <MergeConflictViewer
            v-if="'error' in commit && commit.error && 'MergeConflict' in commit.error"
            :conflict="commit.error.MergeConflict"
            :branch-name="branchName"
          />
          <UAlert
            v-else-if="'error' in commit && commit.error && 'Generic' in commit.error"
            color="error"
            variant="soft"
            size="xs"
          >
            <template #description>
              <p class="text-xs">{{ commit.error.Generic }}</p>
            </template>
          </UAlert>
        </div>
      </div>

      <!-- Slot for additional content after commit -->
      <slot name="after-commit" :commit="commit as any" :index="index" />
    </div>
  </div>
</template>

<script lang="ts" setup>
import type { CommitDetail, CommitSyncStatus, BranchError, FileDiff, MissingCommit } from "~/utils/bindings"
import { formatShortHash } from "~/utils/hash"
import { formatTimestamp } from "~/utils/time"

// Generic commit interface for flexibility
interface GenericCommit {
  hash?: string
  originalHash?: string
  message: string
  author?: string
  authorTime?: number
  committerTime?: number
  fileCount?: number
  fileDiffs?: FileDiff[]
  status?: CommitSyncStatus
  error?: BranchError | null
}

interface Props {
  // Commits can be array, Map, or single type
  commits: CommitDetail[] | Map<string, CommitDetail> | GenericCommit[] | MissingCommit[]

  // Display variants
  variant?: "compact" | "detailed" | "status"
  showDividers?: boolean
  showHover?: boolean

  // Status-specific
  branchName?: string

  // Layout customization
  containerClass?: string
  itemClass?: string
  messageClass?: string

  // Feature flags
  showFileCount?: boolean
  showAuthor?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  variant: "compact",
  showDividers: true,
  showHover: true,
  showFileCount: false,
  showAuthor: false,
  branchName: undefined,
  containerClass: undefined,
  itemClass: undefined,
  messageClass: undefined,
})

// Normalize commits to array format
const normalizedCommits = computed(() => {
  if (props.commits instanceof Map) {
    return Array.from(props.commits.values())
  }
  return props.commits
})

// Helper to get file count for a commit
function getFileCount(commit: CommitDetail | GenericCommit | MissingCommit): number | undefined {
  if ("fileDiffs" in commit && commit.fileDiffs?.length) {
    return commit.fileDiffs.length
  }
  if ("fileCount" in commit) {
    return commit.fileCount
  }
  return undefined
}

// Container class computation
const computedContainerClass = computed(() => {
  if (props.containerClass) return props.containerClass

  const classes = []
  if (props.showDividers) {
    classes.push("divide-y divide-default")
  }
  else {
    classes.push("space-y-2")
  }
  return classes.join(" ")
})

// Item class computation
const computedItemClass = computed(() => {
  if (props.itemClass) return props.itemClass

  const classes = ["px-6 py-3"]
  if (props.showHover) {
    classes.push("hover:bg-muted transition-colors")
  }
  if (!props.showDividers) {
    classes.push("rounded-md")
  }
  return classes.join(" ")
})

// Message class computation
const computedMessageClass = computed(() => {
  if (props.messageClass) return props.messageClass

  const baseClass = "text-sm font-medium text-highlighted"
  if (props.variant === "compact") {
    return `${baseClass} break-words`
  }
  return `${baseClass} truncate`
})

// Helper functions for status variant
function isExceptionalStatus(status?: CommitSyncStatus): boolean {
  return status === "Pending" || status === "Error" || status === "Blocked"
}

function shouldShowStatusText(status: CommitSyncStatus): boolean {
  // Only show status text for non-common statuses
  return status !== "Unchanged"
}

function getCommitStatusClass(status: CommitSyncStatus): string {
  switch (status) {
    case "Pending":
      return "text-dimmed"
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