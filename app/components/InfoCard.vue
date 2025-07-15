<template>
  <div class="bg-elevated rounded-lg p-4">
    <div class="flex items-center gap-2 mb-3">
      <UIcon :name="icon" class="size-4 flex-shrink-0" :class="iconClass" />
      <h3 class="text-sm font-medium text-highlighted break-normal">{{ title }}</h3>
    </div>
    <div class="break-normal text-sm text-toned">
      <slot />
    </div>
  </div>
</template>

<script lang="ts" setup>
withDefaults(defineProps<{
  title: string
  icon: string
  iconClass?: string
}>(), {
  iconClass: "text-info",
})
</script>
