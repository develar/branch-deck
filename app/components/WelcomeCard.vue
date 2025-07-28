<template>
  <div>
    <UCard data-testid="welcome-card">
      <template #header>
        <div class="flex items-center gap-2">
          <UIcon name="i-lucide-git-branch" class="size-5 text-primary" />
          <h3 class="text-lg font-semibold">Welcome to Branch Deck!</h3>
        </div>
      </template>

      <div class="space-y-4">
        <p class="text-muted">
          {{ hasBranchPrefix
            ? 'You\'re almost ready! Just select a Git repository to start managing your branches.'
            : 'Let\'s get you started with managing your Git branches. Follow these simple steps:' }}
        </p>

        <div class="space-y-3">
          <!-- Step 1: Select Repository -->
          <div class="flex gap-3">
            <div class="flex-shrink-0 mt-0.5">
              <div class="size-6 rounded-full bg-primary/10 flex items-center justify-center text-xs font-medium text-primary">
                {{ hasBranchPrefix ? '✓' : '1' }}
              </div>
            </div>
            <div class="flex-1">
              <h4 class="font-medium mb-1">Select a Git repository</h4>
              <p class="text-sm text-muted">
                Use the repository selector above or click the folder icon to browse for your project.
              </p>
            </div>
          </div>

          <!-- Step 2: Configure Branch Prefix -->
          <div v-if="!hasBranchPrefix" class="flex gap-3">
            <div class="flex-shrink-0 mt-0.5">
              <div class="size-6 rounded-full bg-primary/10 flex items-center justify-center text-xs font-medium text-primary">
                2
              </div>
            </div>
            <div class="flex-1">
              <h4 class="font-medium mb-1">Configure your branch prefix</h4>
              <p class="text-sm text-muted mb-2">
                Set a default prefix for your branches to keep them organized. For example: <code class="text-xs bg-muted/20 px-1 py-0.5 rounded">username/</code>
              </p>
              <div class="flex items-center gap-2 text-sm">
                <UIcon name="i-lucide-terminal" class="size-3.5 text-muted" />
                <code class="bg-muted/20 px-2 py-1 rounded text-xs">git config --global branchdeck.branchPrefix "your-prefix"</code>
                <UButton
                  variant="link"
                  size="xs"
                  icon="i-lucide-help-circle"
                  @click="openBranchPrefixHelp"
                >
                  View guide
                </UButton>
              </div>
            </div>
          </div>
        </div>

        <div class="pt-2">
          <p class="text-sm text-muted">
            {{ hasBranchPrefix
              ? 'Once you select a repository, you can sync your branches and start organizing your commits.'
              : 'Once configured, you can sync your branches and start organizing your commits.' }}
          </p>
        </div>
      </div>
    </UCard>

  </div>
</template>

<script lang="ts" setup>
import { BranchPrefixHelp } from "#components"

defineProps<{
  hasBranchPrefix: boolean
}>()

const overlay = useOverlay()

const openBranchPrefixHelp = () => {
  const modal = overlay.create(BranchPrefixHelp, {
    props: {
      disabled: false,
      configured: true,
    },
  })
  modal.open()
}
</script>