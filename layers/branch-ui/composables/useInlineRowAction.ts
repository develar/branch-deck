import { nextTick } from "vue"

export type InlineActionType = "issue-reference" | "delete-archived" | "amend-changes" | "amend-conflict"

export interface ActiveInline {
  type: InlineActionType
  branchName: string
  conflictInfo?: import("~/utils/bindings").MergeConflictInfo // For amend-conflict type
  processing?: boolean
  processingMessage?: string
}

export interface UseInlineRowActionReturn {
  activeInline: Ref<ActiveInline | null>
  openInline: (type: InlineActionType, branchName: string, conflictInfo?: import("~/utils/bindings").MergeConflictInfo) => void
  closeInline: () => void
  isActive: (type: InlineActionType, branchName: string) => boolean
  isActiveForRow: (branchName: string) => boolean
  portalTargetIdFor: (branchName: string) => string
  withPostSubmit: (fn: () => void | Promise<void>) => void
  // Row processing helpers
  isProcessing: (key: string) => boolean
  withRowProcessing: <T>(key: string, fn: () => Promise<T>, opts: {
    success: (value: T) => { title: string, description?: string, duration?: number }
    error: (err: unknown) => { title: string, description?: string, duration?: number }
    processingMessage?: string
  }) => Promise<T | undefined>
}

// Singleton instance to be shared across all components
let singletonInstance: UseInlineRowActionReturn | null = null

export function useInlineRowAction(): UseInlineRowActionReturn {
  // Return existing instance if it exists
  if (singletonInstance) {
    return singletonInstance
  }

  const activeInline = ref<ActiveInline | null>(null)

  // Row processing state based on activeInline
  function isProcessing(key: string) {
    return activeInline.value?.branchName === key && activeInline.value?.processing === true
  }

  async function withRowProcessing<T>(key: string, fn: () => Promise<T>, opts: {
    success: (value: T) => { title: string, description?: string, duration?: number }
    error: (err: unknown) => { title: string, description?: string, duration?: number }
    processingMessage?: string
  }): Promise<T | undefined> {
    const toast = useToast()

    // Set processing state instead of closing
    if (activeInline.value?.branchName === key) {
      activeInline.value.processing = true
      activeInline.value.processingMessage = opts.processingMessage
    }

    try {
      const value = await fn()
      const message = opts.success(value)
      toast.add({ color: "success", title: message.title, description: message.description, duration: message.duration })

      // Close inline on success
      closeInline()

      return value
    }
    catch (error) {
      const message = opts.error(error)
      toast.add({ color: "error", title: message.title, description: message.description, duration: message.duration })

      // Close inline on error
      closeInline()

      return undefined
    }
  }

  function openInline(type: InlineActionType, branchName: string, conflictInfo?: import("~/utils/bindings").MergeConflictInfo) {
    const shouldRetarget = activeInline.value && (
      activeInline.value.type !== type || activeInline.value.branchName !== branchName
    )
    if (shouldRetarget) {
      activeInline.value = null
      // noinspection JSIgnoredPromiseFromCall
      nextTick(() => {
        activeInline.value = { type, branchName, conflictInfo, processing: false }
      })
    }
    else if (!activeInline.value) {
      activeInline.value = { type, branchName, conflictInfo, processing: false }
    }
  }

  function closeInline() {
    activeInline.value = null
  }

  function isActive(type: InlineActionType, branchName: string) {
    return activeInline.value?.type === type && activeInline.value.branchName === branchName
  }

  function isActiveForRow(branchName: string) {
    return activeInline.value?.branchName === branchName
  }

  function portalTargetIdFor(branchName: string) {
    // Sanitize to be a valid CSS selector (used as #id). Replace chars that need escaping.
    const safe = branchName.replace(/[^A-Za-z0-9_-]/g, "_")
    return `inline-form-${safe}`
  }

  function withPostSubmit(fn: () => void | Promise<void>) {
    closeInline()
    // noinspection JSIgnoredPromiseFromCall
    nextTick(() => {
      // noinspection JSIgnoredPromiseFromCall
      fn()
    })
  }

  const result = {
    activeInline,
    openInline,
    closeInline,
    isActive,
    isActiveForRow,
    portalTargetIdFor,
    withPostSubmit,
    isProcessing,
    withRowProcessing,
  }

  // Store singleton instance
  singletonInstance = result

  return result
}
