<template>
  <UPopover mode="hover">
    <UIcon name="i-lucide-info" class="size-3 text-muted hover:text-highlighted transition-colors cursor-help" data-testid="archived-branches-help-icon" />
    <template #content>
      <div class="p-3 divide-y divide-default [&>div]:py-3 [&>div:first-child]:pt-0 [&>div:last-child]:pb-0 text-xs max-w-sm">
        <!-- Archived branches explanation -->
        <div class="space-y-1.5">
          <div class="text-toned font-medium">
            Automatic archiving
          </div>
          <p>
            Virtual branches are automatically archived when they become inactive (no longer match commits in your working directory).
          </p>
        </div>

        <!-- Cleanup policy -->
        <div class="space-y-1.5">
          <div class="text-toned font-medium">
            Automatic cleanup
          </div>
          <p>
            Archived branches are automatically deleted after <strong>7 days</strong> to keep your repository clean.
          </p>
          <p class="text-muted">
            This cleanup happens during branch synchronization.
          </p>
        </div>
      </div>
    </template>
  </UPopover>
</template>

<script lang="ts" setup>
// No props or logic needed for this simple help popover
</script>