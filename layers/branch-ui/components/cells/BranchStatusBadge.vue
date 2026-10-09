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
import { toRef } from "vue"
import UPopover from "@nuxt/ui/components/Popover.vue"
import UBadge from "@nuxt/ui/components/Badge.vue"
import type { ReactiveBranch } from "~/composables/branchSyncProvider"
import { useStatusBadge } from "../../composables/useStatusBadge"

const props = defineProps<{
  branch: ReactiveBranch
}>()

const status = useStatusBadge(toRef(props, "branch"))
</script>
