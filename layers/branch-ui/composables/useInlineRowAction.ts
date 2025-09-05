import { nextTick } from "vue"

export type InlineActionType = "issue-reference" | "delete-archived" | "amend-changes" | "amend-conflict"

export interface ActiveInline {
  type: InlineActionType
  branchName: string
  conflictInfo?: import("~/utils/bindings").MergeConflictInfo // For amend-conflict type
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
  processingKey: Ref<string | null>
  isProcessing: (key: string) => boolean
  processingClass: (key: string) => string
  withRowProcessing: <T>(key: string, fn: () => Promise<T>, opts: {
    success: (value: T) => { title: string, description?: string, duration?: number }
    error: (err: unknown) => { title: string, description?: string, duration?: number }
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

  // Row processing state shared by inline actions
  const processingKey = ref<string | null>(null)
  function isProcessing(key: string) {
    return processingKey.value === key
  }
  function processingClass(key: string) {
    return isProcessing(key) ? "processing-border" : ""
  }

  async function withRowProcessing<T>(key: string, fn: () => Promise<T>, opts: {
    success: (value: T) => { title: string, description?: string, duration?: number }
    error: (err: unknown) => { title: string, description?: string, duration?: number }
  }): Promise<T | undefined> {
    const toast = useToast()
    // close any active inline UI before starting processing
    closeInline()
    processingKey.value = key
    try {
      const value = await fn()
      const message = opts.success(value)
      toast.add({ color: "success", title: message.title, description: message.description, duration: message.duration })
      return value
    }
    catch (error) {
      const message = opts.error(error)
      toast.add({ color: "error", title: message.title, description: message.description, duration: message.duration })
      return undefined
    }
    finally {
      processingKey.value = null
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
        activeInline.value = { type, branchName, conflictInfo }
      })
    }
    else if (!activeInline.value) {
      activeInline.value = { type, branchName, conflictInfo }
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
    processingKey,
    isProcessing,
    processingClass,
    withRowProcessing,
  }

  // Store singleton instance
  singletonInstance = result

  return result
}
