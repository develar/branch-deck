<template>
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <h2 class="text-sm font-semibold text-highlighted">
        {{ title }}
      </h2>
      <UBadge
        v-if="count !== undefined"
        :color="badgeColor"
        variant="subtle"
        size="sm"
      >
        {{ count }} {{ count === 1 ? itemSingular : itemPlural }}
      </UBadge>
    </div>
    <div v-if="$slots.actions" class="flex items-center gap-3">
      <slot name="actions" />
    </div>
  </div>
</template>

<script lang="ts" setup>
interface Props {
  title: string
  count?: number
  itemSingular?: string
  itemPlural?: string
  badgeColor?: "primary" | "secondary" | "success" | "info" | "warning" | "error" | "neutral"
}

withDefaults(defineProps<Props>(), {
  badgeColor: "warning",
  itemSingular: "item",
  itemPlural: "items",
  count: undefined,
})
</script>