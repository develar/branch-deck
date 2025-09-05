<template>
  <UFieldGroup size="xs">
    <UButton
      icon="i-lucide-align-left"
      :color="store.conflictDiffViewMode === 'unified' ? 'primary' : 'neutral'"
      variant="outline"
      @click="store.conflictDiffViewMode = 'unified'"
    >
      Unified
    </UButton>
    <UButton
      icon="i-lucide-columns-2"
      :color="store.conflictDiffViewMode === 'split' ? 'primary' : 'neutral'"
      variant="outline"
      @click="store.conflictDiffViewMode = 'split'"
    >
      Split
    </UButton>
  </UFieldGroup>
</template>

<script lang="ts" setup>
// Get conflict viewer settings from store
const store = useConflictViewerStore()
</script>