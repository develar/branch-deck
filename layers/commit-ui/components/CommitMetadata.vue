<template>
  <!-- Hash -->
  <span class="font-mono">{{ formatShortHash(commitHash) }}</span>

  <!-- Status variant: show arrow and new hash -->
  <template v-if="showStatusHash && statusHash">
    <span>→</span>
    <span class="font-mono">{{ formatShortHash(statusHash) }}</span>
  </template>

  <!-- Author -->
  <template v-if="showAuthor && author">
    <span>•</span>
    <span>{{ author }}</span>
  </template>

  <!-- Timestamp -->
  <template v-if="authorTime">
    <span>•</span>
    <TimeWithPopover
      v-if="committerTime"
      :author-time="authorTime"
      :committer-time="committerTime"
    />
    <span v-else>{{ formatTimestamp(authorTime) }}</span>
  </template>
</template>

<script lang="ts" setup>
import type { Commit, MissingCommit } from "~/utils/bindings"
import type { SyncedCommit } from "~/composables/branchSyncProvider"

// Union type for all supported commit types
type CommitUnion = Commit | SyncedCommit | MissingCommit

const props = withDefaults(defineProps<{
  commit: CommitUnion
  showAuthor?: boolean
  variant?: "compact" | "status"
}>(), {
  showAuthor: false,
  variant: "compact",
})

// Extract values from different commit types
const commitHash = computed(() => {
  if ("originalHash" in props.commit && props.commit.originalHash) {
    return props.commit.originalHash
  }
  if ("hash" in props.commit && props.commit.hash) {
    return props.commit.hash
  }
  return ""
})

const statusHash = computed(() => {
  return "hash" in props.commit ? props.commit.hash : null
})

const showStatusHash = computed(() => {
  return props.variant === "status" && statusHash.value && commitHash.value !== statusHash.value
})

const author = computed(() => props.commit.author || "")
const authorTime = computed(() => props.commit.authorTime || 0)
const committerTime = computed(() => props.commit.committerTime)
</script>