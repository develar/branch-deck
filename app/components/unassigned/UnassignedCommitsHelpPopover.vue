<template>
  <UPopover mode="hover">
    <UIcon name="i-lucide-info" class="size-3 text-muted hover:text-highlighted transition-colors cursor-help" data-testid="unassigned-commits-help-icon" />
    <template #content>
      <div class="p-3 space-y-3 text-xs">
        <div class="font-medium text-highlighted">Help</div>

        <!-- Selection shortcuts section -->
        <div class="space-y-1.5">
          <div class="text-toned font-medium">Selection shortcuts</div>
          <div>Click to select a commit</div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">{{ isMac ? '⌘' : 'Ctrl' }}</UKbd>
            <span>+ Click for multiple</span>
          </div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">Shift</UKbd>
            <span>+ Click for range</span>
          </div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">{{ isMac ? '⌘' : 'Ctrl' }}</UKbd>
            <UKbd size="sm">A</UKbd>
            <span>Select all</span>
          </div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">Esc</UKbd>
            <span>Clear selection</span>
          </div>
        </div>

        <!-- Divider -->
        <div class="border-t border-default"/>

        <!-- Unassigned commits explanation -->
        <div class="space-y-1.5">
          <div class="text-toned font-medium">About unassigned commits</div>
          <p class="text-muted">
            These commits won't be assigned to virtual branches because they don't match Branch Deck's grouping patterns:
          </p>
          <ul class="space-y-1 text-muted">
            <li class="flex items-start gap-1">
              <span>•</span>
              <span><code class="text-[10px] bg-default px-1 rounded">(feature-name)</code> - Explicit prefix</span>
            </li>
            <li class="flex items-start gap-1">
              <span>•</span>
              <span><code class="text-[10px] bg-default px-1 rounded">ABC-123</code> - Issue numbers</span>
            </li>
          </ul>
        </div>
      </div>
    </template>
  </UPopover>
</template>

<script lang="ts" setup>
// Platform detection
const isMac = computed(() => navigator.userAgent.toLowerCase().includes("mac"))
</script>