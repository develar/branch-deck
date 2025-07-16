import { ref } from "vue"
// notifyError is auto-imported

export function useCopyToClipboard() {
  // Track copied items
  const copiedItems = ref<Set<string>>(new Set())

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text)
      copiedItems.value.add(text)

      // Remove the item from copied set after 2 seconds
      setTimeout(() => {
        copiedItems.value.delete(text)
      }, 2000)
    }
    catch (err) {
      notifyError("Copy Failed", err)
    }
  }

  return {
    copiedItems,
    copyToClipboard,
  }
}