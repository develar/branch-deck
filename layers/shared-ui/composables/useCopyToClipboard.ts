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
  // Track copied state as a simple boolean
  const isCopied = ref(false)
  // Track tooltip open state
  const tooltipOpen = ref(false)

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      isCopied.value = true
      tooltipOpen.value = true

      // Get timeout from test config or use default 2 seconds
      const timeout = window.__BRANCH_DECK_TEST_CONFIG__?.copyTimeout ?? 2000

      // Reset after timeout
      setTimeout(() => {
        isCopied.value = false
        tooltipOpen.value = false
      }, timeout)
    }
    catch (err) {
      notifyError("Copy Failed", err)
    }
  }

  return {
    isCopied,
    tooltipOpen,
    copyToClipboard,
  }
}