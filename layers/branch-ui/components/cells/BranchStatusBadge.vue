<template>
  <div class="w-40">
    <UPopover mode="hover" :open-delay="300" :ui="{ content: 'whitespace-pre-line break-words px-3 py-2 text-sm leading-snug' }">
      <UBadge
        :color="status.color"
        variant="soft"
        class="lowercase whitespace-nowrap truncate max-w-[10rem]"
      >
        {{ status.text }}
      </UBadge>
      <template #content>
        {{ status.tooltip }}
      </template>
    </UPopover>
  </div>
</template>

<script lang="ts" setup>
import type { ReactiveBranch } from "~/composables/branchSyncProvider"

const props = defineProps<{
  branch: ReactiveBranch
}>()

const status = useStatusBadge(toRef(props, "branch"))
</script>
