<template>
  <div class="flex items-start gap-3">
    <UIcon name="i-lucide-git-merge" class="size-5 text-error mt-0.5 flex-shrink-0"/>
    <div class="flex-1 min-w-0">
      <p class="text-sm font-medium text-highlighted mb-1">
        {{ title || 'Cherry-pick conflict' }}
      </p>
      <p class="text-sm text-toned">
        {{ commitMessage }}
      </p>
      <div class="flex items-center gap-3 mt-1">
        <CommitInfo :hash="commitHash" :author-time="commitAuthorTime" :committer-time="commitCommitterTime" />
        <template v-if="branchName">
          <span class="text-xs text-muted">•</span>
          <span class="text-xs text-muted">Branch: {{ branchName }}</span>
        </template>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
defineProps<{
  title?: string
  commitMessage: string
  commitHash: string
  commitAuthorTime: number
  commitCommitterTime: number
  branchName?: string
}>()
</script>
