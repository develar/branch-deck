<template>
  <div class="flex items-center gap-2">
    <CopyButton
      :text="() => getFullBranchName(props.branch.name)"
      tooltip="Copy full branch name"
      size="xs"
      always-visible
    />

    <UPopover
      v-if="!branch.hasError && branch.commitCount > 0"
      mode="hover"
      :open-delay="300"
      :ui="{ content: 'whitespace-pre-line break-words px-3 py-2 text-sm leading-snug' }"
    >
      <UButton
        :disabled="isPushButtonDisabled"
        :loading="branch.isPushing"
        :color="pushButtonColor"
        icon="i-lucide-upload"
        size="xs"
        variant="outline"
        @click.stop="pushBranch(branch.name)"
      >
        {{ pushButtonText }}
      </UButton>
      <template #content>
        {{ pushButtonTooltip }}
      </template>
    </UPopover>
  </div>
</template>

<script lang="ts" setup>
import type { ReactiveBranch } from "~/composables/branchSyncProvider"
import { usePushButton } from "~/composables/usePushButton"
import { usePush } from "~/composables/git/push"

const props = defineProps<{
  branch: ReactiveBranch
}>()

const { vcsRequestFactory, getFullBranchName } = useRepository()
const { isSyncing, branches, baselineBranch } = useBranchSync()
const { pushBranch } = usePush(vcsRequestFactory, branches, baselineBranch)

// Use reactive push button state
const {
  pushButtonText,
  pushButtonColor,
  pushButtonTooltip,
  isPushButtonDisabled,
} = usePushButton(toRef(props, "branch"), isSyncing)
</script>