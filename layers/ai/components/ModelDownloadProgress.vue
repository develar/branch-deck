<template>
  <!-- no padding for toast -->
  <div
    :class="['space-y-2 text-xs w-xs', showCancelButton ? 'p-3' : '']"
    :data-testid="showCancelButton ? 'model-download-progress-popover' : 'model-download-progress-toast'"
  >
    <!-- Progress content based on event type -->
    <div v-if="downloadProgress">
      <!-- Started, FileStarted or Progress -->
      <template v-if="downloadProgress.type === 'Started' || downloadProgress.type === 'FileStarted' || downloadProgress.type === 'Progress'">
        <div class="space-y-2">
          <!-- Title: either filename or preparing message -->
          <p v-if="downloadProgress.type === 'Started'" class="text-muted">
            Preparing {{ downloadProgress.data.totalFiles }} {{ downloadProgress.data.totalFiles === 1 ? 'file' : 'files' }}...
          </p>
          <p v-else-if="'fileName' in downloadProgress.data" class="font-semibold">
            {{ downloadProgress.data.fileName }}
          </p>

          <div class="space-y-1">
            <!-- FileStarted: show file size -->
            <p v-if="downloadProgress.type === 'FileStarted' && downloadProgress.data.fileSize" class="text-muted">
              File size: {{ formatBytes(downloadProgress.data.fileSize) }}
            </p>
            <!-- Progress: show download status -->
            <div v-else-if="downloadProgress.type === 'Progress'" class="flex justify-between text-muted">
              <span>{{ formatBytes(downloadProgress.data.downloaded) }} / {{ formatBytes(downloadProgress.data.total) }}</span>
              <span>{{ downloadProgress.data.total > 0 ? Math.round((downloadProgress.data.downloaded / downloadProgress.data.total) * 100) : 0 }}%</span>
            </div>

            <!-- Progress bar: indeterminate for FileStarted, determinate for Progress -->
            <UProgress
              :model-value="downloadProgress.type === 'Progress' && downloadProgress.data.total > 0
                ? Math.round((downloadProgress.data.downloaded / downloadProgress.data.total) * 100)
                : null"
            />
          </div>

          <!-- Speed/time info only for Progress -->
          <div v-if="downloadProgress.type === 'Progress'" class="flex gap-3 text-muted min-h-[1rem]">
            <span v-if="downloadProgress.data.bytesPerSecond">
              {{ formatBytes(downloadProgress.data.bytesPerSecond) }}/s
            </span>
            <span v-if="downloadProgress.data.secondsRemaining !== null && downloadProgress.data.secondsRemaining !== undefined">
              {{ formatTime(downloadProgress.data.secondsRemaining) }} remaining
            </span>
          </div>

          <!-- Cancel button -->
          <div v-if="showCancelButton" class="pt-2 border-t border-default flex justify-end">
            <UButton
              size="xs"
              color="neutral"
              variant="outline"
              @click="$emit('cancel')"
            >
              Pause download
            </UButton>
          </div>
        </div>
      </template>

      <!-- FileCompleted -->
      <template v-else-if="downloadProgress.type === 'FileCompleted'">
        <p class="text-success">
          ✓ {{ downloadProgress.data.fileName }} completed
        </p>
      </template>

      <!-- Completed -->
      <template v-else-if="downloadProgress.type === 'Completed'">
        <p class="text-success font-semibold">
          All files downloaded successfully!
        </p>
      </template>

      <!-- Error -->
      <template v-else-if="downloadProgress.type === 'Error'">
        <div class="space-y-2">
          <div class="flex items-center gap-2">
            <UIcon name="i-lucide-alert-triangle" class="size-4 text-error" />
            <p class="font-semibold text-error">
              Download Failed
            </p>
          </div>
          <p class="text-muted">
            {{ downloadProgress.data.message }}
          </p>
        </div>
      </template>

      <!-- Cancelled -->
      <template v-else-if="downloadProgress.type === 'Cancelled'">
        <p class="text-muted">
          Download cancelled
        </p>
      </template>
    </div>

    <!-- Fallback when no progress data -->
    <div v-else class="text-muted">
      Initializing download...
    </div>
  </div>
</template>

<script setup lang="ts">
import type { DownloadProgress } from "~/utils/bindings"

defineProps<{
  downloadProgress?: DownloadProgress | null
  showCancelButton?: boolean
}>()

defineEmits<{
  cancel: []
}>()

// Helper functions
function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024)
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${(bytes / 1024).toFixed(1)} KB`
}

function formatTime(seconds: number): string {
  if (seconds === 0) {
    return "Finishing..."
  }
  if (seconds < 60) {
    return `${seconds}s`
  }
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`
}
</script>
