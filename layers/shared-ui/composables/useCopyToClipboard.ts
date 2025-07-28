import { ref } from "vue"
// notifyError is auto-imported

interface TestConfig {
  copyTimeout?: number
}

declare global {
  interface Window {
    __BRANCH_DECK_TEST_CONFIG__?: TestConfig
  }
}

export function useCopyToClipboard() {
  // Track copied items
  const copiedItems = ref<Set<string>>(new Set())
  // Track tooltip open state
  const tooltipOpen = ref(false)

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      copiedItems.value.add(text)
      tooltipOpen.value = true

      // Get timeout from test config or use default 2 seconds
      const timeout = window.__BRANCH_DECK_TEST_CONFIG__?.copyTimeout ?? 2000

      // Remove the item from copied set and close tooltip after timeout
      setTimeout(() => {
        copiedItems.value.delete(text)
        tooltipOpen.value = false
      }, timeout)
    }
    catch (err) {
      notifyError("Copy Failed", err)
    }
  }

  return {
    copiedItems,
    tooltipOpen,
    copyToClipboard,
  }
}