<template>
  <div class="flex items-center gap-2">
    <UIcon
      v-if="isLoading"
      name="i-lucide-loader-2"
      class="size-3 animate-spin text-muted"
    />
    <template v-else>
      <UIcon
        :name="statusIcon"
        :class="`size-4 ${statusColor}`"
      />
      <UTooltip v-if="branch.type === 'not-integrated'" :text="tooltipContent">
        <span class="text-sm text-muted">
          {{ statusText }}
        </span>
      </UTooltip>
      <div v-else class="flex items-center gap-2">
        <span class="text-sm text-muted">{{ statusText }}</span>
        <!-- Only show confidence badge if it's not "High" (the default case) -->
        <UBadge
          v-if="showConfidenceBadge"
          :color="getConfidenceColor(branch.confidence!)"
          variant="soft"
          size="sm"
        >
          {{ branch.confidence }} confidence
        </UBadge>
      </div>
    </template>
  </div>
</template>

<script lang="ts" setup>

interface Props {
  branch: ReactiveArchivedBranch
}

const props = defineProps<Props>()

const isLoading = computed(() => props.branch.type === "placeholder")

const statusIcon = computed(() => {
  const branch = props.branch
  if (branch.type === "integrated") {
    return "i-lucide-check-circle"
  }
  else if (branch.type === "not-integrated") {
    const hasPartialIntegration = branch.integratedCount > 0
    return hasPartialIntegration ? "i-lucide-pie-chart" : "i-lucide-alert-triangle"
  }
  return "i-lucide-loader-2"
})

const statusColor = computed(() => {
  const branch = props.branch
  if (branch.type === "integrated") {
    return "text-success"
  }
  else if (branch.type === "not-integrated") {
    const hasPartialIntegration = branch.integratedCount > 0
    return hasPartialIntegration ? "text-info" : "text-warning"
  }
  return "text-muted"
})

const statusText = computed(() => {
  if (isLoading.value) {
    return "Detecting…"
  }

  const branch = props.branch
  if (branch.type === "integrated") {
    return `Merged · ${(branch.commitCount)}`
  }
  else if (branch.type === "not-integrated") {
    const hasPartialIntegration = branch.integratedCount > 0
    if (hasPartialIntegration) {
      return `Partially merged · ${branch.integratedCount}/${branch.commitCount}`
    }
    else {
      return `Not merged · ${(branch.commitCount)}`
    }
  }
  else {
    return ""
  }
})

const tooltipContent = computed(() => {
  const branch = props.branch
  if (branch.type === "not-integrated") {
    const hasPartialIntegration = branch.integratedCount > 0
    return hasPartialIntegration
      ? `Partially merged: ${branch.integratedCount} of ${branch.commitCount} commits merged`
      : "Original commits are missing from HEAD"
  }
  return ""
})

const showConfidenceBadge = computed(() => {
  const branch = props.branch
  return branch.type === "integrated"
    && branch.confidence
    && branch.confidence !== "High"
})

function getConfidenceColor(confidence: string) {
  switch (confidence) {
    case "Exact":
      return "success" as const
    case "High":
      return "info" as const
    case "Low":
      return "neutral" as const
    default:
      return "neutral" as const
  }
}
</script>