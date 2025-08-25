<template>
  <UModal title="Configure Branch Prefix" class="max-w-2xl">
    <UTooltip :disabled="configured" text="Please configure the branch prefix">
      <UButton
        :color="configured || disabled ? undefined : 'warning'"
        :disabled="disabled"
        icon="i-lucide-settings"
        variant="outline"
        size="sm"
      />
    </UTooltip>

    <template #content>
      <div class="p-6 space-y-6 max-h-[90vh] overflow-y-auto">
        <p class="text-sm text-default">
          The branch prefix is automatically prepended to your virtual branches. Setting a personal prefix (like your username) helps identify your branches in a shared repository.
        </p>

        <div class="space-y-4">
          <!-- Global Configuration -->
          <div class="rounded-lg border border-default bg-subtle p-4">
            <h3 class="text-sm font-semibold text-default mb-3">
              Set your global branch prefix
            </h3>
            <p class="text-sm text-muted mb-3">
              Run this command in your terminal to set a global prefix for all repositories:
            </p>
            <div class="bg-default rounded p-3 overflow-x-auto">
              <code class="text-xs text-mono">git config --global branchdeck.branchPrefix yourUsername</code>
            </div>
            <p class="text-xs text-muted mt-2">
              Replace "yourUsername" with your preferred prefix (e.g., your name or initials)
            </p>
          </div>

          <!-- How It Works -->
          <div class="rounded-lg border border-info/20 bg-info/5 p-4">
            <h3 class="text-sm font-semibold text-default mb-3">
              How It Works
            </h3>
            <p class="text-sm text-muted mb-3">
              Branch Deck groups commits based on message patterns:
            </p>
            <ul class="space-y-2 text-sm">
              <li class="flex items-start gap-2">
                <span class="text-muted">•</span>
                <div class="flex-1">
                  <span class="font-medium">Explicit prefix:</span>
                  <div class="mt-1 text-muted">
                    <code class="text-xs bg-default px-1 py-0.5 rounded">(feature-login)</code>
                    <span class="mx-2">→</span>
                    <code class="text-xs bg-default px-1 py-0.5 rounded">john/virtual/feature-login</code>
                  </div>
                </div>
              </li>
              <li class="flex items-start gap-2">
                <span class="text-muted">•</span>
                <div class="flex-1">
                  <span class="font-medium">Issue numbers:</span>
                  <div class="mt-1 text-muted">
                    <code class="text-xs bg-default px-1 py-0.5 rounded">ABC-123: Fix bug</code>
                    <span class="mx-2">→</span>
                    <code class="text-xs bg-default px-1 py-0.5 rounded">john/virtual/ABC-123</code>
                  </div>
                </div>
              </li>
              <li class="flex items-start gap-2">
                <span class="text-muted">•</span>
                <div class="flex-1">
                  <span class="font-medium">With subsystem:</span>
                  <div class="mt-1 text-muted">
                    <code class="text-xs bg-default px-1 py-0.5 rounded">[threading] ABC-123: Fix bug</code>
                    <span class="mx-2">→</span>
                    <code class="text-xs bg-default px-1 py-0.5 rounded">john/virtual/ABC-123</code>
                  </div>
                </div>
              </li>
            </ul>
            <p class="text-xs text-muted mt-3">
              Explicit prefixes take precedence over issue numbers. The <code class="bg-default px-1 py-0.5 rounded">virtual</code> prefix is always automatically prepended.
            </p>
          </div>

          <!-- Repository-specific -->
          <div class="border-t border-default pt-4">
            <p class="text-sm text-muted mb-2">
              You can also set this value per repository using:
            </p>
            <div class="bg-default rounded p-3 overflow-x-auto">
              <code class="text-xs text-mono">git config branchdeck.branchPrefix repoSpecificPrefix</code>
            </div>
          </div>
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
