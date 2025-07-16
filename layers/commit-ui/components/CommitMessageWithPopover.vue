<template>
  <div class="flex items-center gap-2">
    <p :class="messageClass">
      {{ subject }}
    </p>
    <!-- Show popover icon only if message has multiple lines -->
    <UPopover v-if="hasFullMessage" mode="hover">
      <UIcon name="i-lucide-rectangle-ellipsis" class="size-3 text-muted hover:text-highlighted" />
      <template #content>
        <div class="p-3 max-w-[500px] space-y-2">
          <p class="text-sm font-medium text-highlighted">{{ subject }}</p>
          <div v-if="body" class="text-xs text-toned whitespace-pre-wrap">{{ body }}</div>
        </div>
      </template>
    </UPopover>
  </div>
</template>

<script lang="ts" setup>
interface Props {
  subject: string
  message: string
  messageClass?: string
}

const props = withDefaults(defineProps<Props>(), {
  messageClass: "text-sm text-highlighted",
})

// Check if the message has more than just the subject line
const hasFullMessage = computed(() => {
  return props.message && props.message !== props.subject && props.message.includes("\n")
})

// Extract the body (everything after the first line)
const body = computed(() => {
  if (!hasFullMessage.value) return ""
  const lines = props.message.split("\n")
  // Skip the subject line and any empty lines immediately after
  let bodyStartIndex = 1
  while (bodyStartIndex < lines.length && lines[bodyStartIndex]?.trim() === "") {
    bodyStartIndex++
  }
  return lines.slice(bodyStartIndex).join("\n").trim() || ""
})
</script>