<template>
  <UPopover v-if="committerTime && authorTime !== committerTime" mode="hover">
    <span>{{ formatTimestamp(authorTime) }}</span>
    <template #content>
      <div class="p-3">
        <table class="text-xs">
          <tbody>
            <tr>
              <td class="text-muted pr-3">Authored</td>
              <td class="text-highlighted">{{ formatTimestamp(authorTime) }}</td>
            </tr>
            <tr>
              <td class="text-muted pr-3">Committed</td>
              <td class="text-highlighted">{{ formatTimestamp(committerTime) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </UPopover>
  <span v-else>{{ formatTimestamp(authorTime) }}</span>
</template>

<script lang="ts" setup>
import { formatTimestamp } from '~/utils/time'

defineProps<{
  authorTime: number
  committerTime?: number
}>()
</script>