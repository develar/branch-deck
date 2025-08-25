<template>
  <td class="px-6 py-4 max-w-xs">
    <div class="flex items-center gap-2 min-w-0">
      <UButton
        v-if="canExpand"
        :icon="expanded ? 'i-lucide-folder-open' : 'i-lucide-folder-closed'"
        variant="ghost"
        size="xs"
        class="shrink-0"
        @click.stop="$emit('toggleExpanded')"
      />
      <LinkedText
        :text="displayName"
        text-class="text-sm font-medium shrink-0"
      />
      <span
        v-if="summary"
        class="text-xs text-muted truncate"
      >
        {{ summary }}
      </span>
    </div>
  </td>
</template>

<script lang="ts" setup>
const props = withDefaults(defineProps<{
  name: string
  summary?: string
  expanded: boolean
  canExpand?: boolean
  simplified?: boolean // For archived branches - show simple name
}>(), {
  canExpand: true,
  simplified: false,
  summary: undefined,
})

defineEmits<{
  toggleExpanded: []
}>()

const displayName = computed(() =>
  props.simplified ? getSimpleBranchName(props.name) : props.name,
)
</script>