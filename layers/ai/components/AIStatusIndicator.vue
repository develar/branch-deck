<template>
  <!-- AI indicator icon with integrated help -->
  <UPopover v-if="!isDownloading" mode="hover">
    <UIcon
      name="i-lucide-sparkles"
      :class="[
        'size-3.5 cursor-pointer transition-all',
        aiStatus.iconClass,
      ]"
      data-testid="ai-status-icon"
      @click="handleClick"
    />
    <template #content>
      <div class="p-3 space-y-2 text-xs w-xs">
        <!-- Error state -->
        <template v-if="aiError">
          <div class="flex items-center gap-2">
            <UIcon name="i-lucide-alert-triangle" class="size-4 text-error" />
            <p class="font-semibold text-error">
              AI Malfunction
            </p>
          </div>
          <div class="space-y-2 mt-2">
            <div class="p-2 bg-error/10 border border-error/20 rounded-md">
              <p class="font-medium text-error mb-1">
                Error:
              </p>
              <p class="text-toned break-words">
                {{ aiError.message }}
              </p>
            </div>
            <details class="cursor-pointer">
              <summary class="text-muted hover:text-highlighted">
                View full details
              </summary>
              <pre class="mt-2 p-2 bg-subtle rounded text-xs overflow-x-auto whitespace-pre-wrap break-words">{{ aiError.details }}</pre>
            </details>
            <div class="flex items-center justify-between text-xs text-muted">
              <span>{{ new Date(aiError.timestamp).toLocaleTimeString() }}</span>
              <a
                href="https://github.com/develar/branch-deck/issues"
                target="_blank"
                class="text-primary hover:underline flex items-center gap-1"
              >
                <UIcon name="i-lucide-external-link" class="size-2.5" />
                Report issue
              </a>
            </div>
          </div>
          <div class="pt-2 border-t border-default">
            <p class="text-muted mb-2">
              Click the icon to retry or disable AI
            </p>
          </div>
        </template>
        <!-- Normal state -->
        <template v-else>
          <p class="font-semibold text-highlighted">
            AI {{ aiMode === 'enabled' ? 'Enabled' : 'Disabled' }}
            <span class="text-muted font-normal ml-1">(click to {{ aiMode === 'enabled' ? 'disable' : 'enable' }})</span>
          </p>
          <p>
            AI analyzes only commit metadata to suggest branch names:
          </p>
          <ul class="list-disc list-inside space-y-1 ml-2">
            <li>Commit messages (title and body)</li>
            <li>Modified file names and their status</li>
          </ul>
          <p>
            This is equivalent to: <code class="bg-subtle px-1 py-0.5 rounded">git log --name-status</code>
          </p>
          <div class="pt-2 border-t border-default">
            <p class="text-success flex items-center gap-1">
              <UIcon name="i-lucide-shield-check" class="size-3" />
              <span class="font-medium">100% Local & Private</span>
            </p>
            <p class="text-toned mt-1">
              Uses the <a href="https://huggingface.co/Qwen/Qwen3-1.7B-GGUF" target="_blank" class="text-primary hover:underline">Qwen3-1.7B</a> model running entirely on your machine. No data is sent to any external service.
            </p>
          </div>
        </template>
      </div>
    </template>
  </UPopover>
  <UPopover
    v-else
    mode="hover"
  >
    <UIcon
      name="i-lucide-sparkles"
      class="size-3.5 text-primary animate-pulse"
      data-testid="ai-status-icon-downloading"
    />
    <template #content>
      <ModelDownloadProgress
        :download-progress="lastProgressEvent"
        :show-cancel-button="true"
        @cancel="cancelDownload()"
      />
    </template>
  </UPopover>
</template>

<script lang="ts" setup>
// AI composables are auto-imported from ai layer

const { aiMode, aiStatus, aiError } = useAIToggle()

// Click handler to toggle between enabled/disabled
function handleClick() {
  aiMode.value = aiMode.value === "enabled" ? "disabled" : "enabled"
}
const { isDownloading, lastProgressEvent, cancelDownload } = useModelState()
</script>
