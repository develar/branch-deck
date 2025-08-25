<template>
  <div class="flex items-center gap-2">
    <UIcon
      v-if="isLoading"
      name="i-lucide-loader-2"
      class="size-3 animate-spin text-muted"
    />
    <span
      v-else-if="formattedDate"
      class="text-sm text-muted"
    >
      {{ formattedDate }}
    </span>
  </div>
</template>

<script lang="ts" setup>
import { formatTimestamp } from "#layers/shared-ui/utils/time"

interface Props {
  branch: ReactiveArchivedBranch
}

const props = defineProps<Props>()

const isLoading = computed(() => props.branch.type === "placeholder")

const formattedDate = computed(() => {
  if (props.branch.integratedAt > 0) {
    return formatTimestamp(props.branch.integratedAt)
  }
  return null
})
</script>