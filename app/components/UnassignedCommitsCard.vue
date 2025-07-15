<template>
  <UCard class="overflow-hidden">
    <template #header>
      <CardHeader
        title="Unassigned Commits"
        :count="commits.length"
        item-singular="commit"
        item-plural="commits"
        badge-color="neutral"
      />
    </template>

    <CommitList :commits="commits" variant="compact" :show-author="true" />

    <template #footer>
      <InfoCard
        title="Commits without prefix"
        icon="i-lucide-info"
      >
        <p>
          These commits don't have a prefix in parentheses (e.g., (feature) or (bugfix)) and won't be assigned to virtual branches.
        </p>
      </InfoCard>
    </template>
  </UCard>
</template>

<script lang="ts" setup>
import type { CommitDetail } from "~/utils/bindings"

defineProps<{
  commits: CommitDetail[]
}>()
</script>