<template>
  <UModal title="Configure Branch Prefix">
    <UTooltip :disabled="configured" text="Please configure the branch prefix">
      <UButton
        :color="configured ? undefined : 'warning'"
        :disabled="disabled"
        icon="i-heroicons-cog-6-tooth"
        variant="outline"
      />
    </UTooltip>
    <template #body>
      <div class="space-y-4">
        <p>
          The branch prefix is automatically prepended to your virtual branches. Setting a personal prefix (like your username) helps identify your branches in a shared
          repository.
        </p>

        <div class="p-4 bg-neutral-100 dark:bg-neutral-800 rounded-lg">
          <h3 class="font-medium mb-2">
            Set your global branch prefix
          </h3>
          <p class="text-sm mb-3">
            Run this command in your terminal to set a global prefix for all repositories:
          </p>
          <div class="bg-black text-white p-3 rounded font-mono text-sm overflow-x-auto">
            git config --global v-branch.branchPrefix yourUsername
          </div>
          <p class="text-xs text-neutral-500 mt-2">
            Replace "yourUsername" with your preferred prefix (e.g., your name or initials)
          </p>
        </div>

        <div class="p-4 bg-info-50 dark:bg-info-900/20 rounded-lg">
          <h3 class="font-medium mb-2">
            Example
          </h3>
          <p class="text-sm">
            If you set <span class="font-mono">v-branch.branchPrefix</span> to <span class="font-mono">john</span> and create a virtual branch from commits with prefix <span
              class="font-mono"
            >(feature-login)</span>, the branch will be named:
          </p>
          <p class="font-mono font-medium mt-2">
            john/virtual/feature-login
          </p>
          <p class="text-xs text-neutral-500 mt-2">
            Note: The <span class="font-mono">virtual</span> prefix is always automatically prepended to ensure virtual branches don't override regular Git branches
          </p>
        </div>

        <div class="text-sm text-neutral-500">
          <p>You can also set this value per repository using:</p>
          <p class="font-mono mt-1">
            git config v-branch.branchPrefix repoSpecificPrefix
          </p>
        </div>
      </div>
    </template>
  </UModal>
</template>

<script lang="ts" setup>
defineProps<{
  disabled: boolean
  configured: boolean
}>()
</script>
