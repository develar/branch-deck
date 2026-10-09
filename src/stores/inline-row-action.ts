import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import type { MergeConflictInfo } from "@/lib/bindings"

export type InlineActionType = "issue-reference" | "delete-archived" | "amend-changes" | "amend-conflict" | "unapply"

export interface ActiveInline {
  type: InlineActionType
  branchName: string
  /** For the amend-conflict type */
  conflictInfo?: MergeConflictInfo
  processing: boolean
  processingMessage?: string
}

export interface ToastMessage {
  title: string
  description?: string
  /** Toast timeout in ms */
  duration?: number
}

export interface RowProcessingOptions<T> {
  success: (value: T) => ToastMessage
  error: (error: unknown) => ToastMessage
  processingMessage?: string
}

export interface InlineRowActionState {
  /** The single inline form (or conflict viewer) open below a table row, if any */
  activeInline: ActiveInline | null
}

export const inlineRowActionStore = createStore<InlineRowActionState>(() => ({
  activeInline: null,
}))

export function useInlineRowAction<T>(selector: (state: InlineRowActionState) => T): T {
  return useStore(inlineRowActionStore, selector)
}

/** The active inline action if it belongs to the given row, otherwise null. */
export function useActiveInlineForRow(branchName: string): ActiveInline | null {
  return useStore(inlineRowActionStore, state => state.activeInline?.branchName === branchName ? state.activeInline : null)
}

export function useIsRowProcessing(key: string): boolean {
  return useStore(inlineRowActionStore, state => isProcessingIn(state, key))
}

// ---- queries ----

function isProcessingIn(state: InlineRowActionState, key: string): boolean {
  return state.activeInline?.branchName === key && state.activeInline.processing
}

export function isProcessing(key: string): boolean {
  return isProcessingIn(inlineRowActionStore.getState(), key)
}

export function isActive(type: InlineActionType, branchName: string): boolean {
  const activeInline = inlineRowActionStore.getState().activeInline
  return activeInline?.type === type && activeInline.branchName === branchName
}

export function isActiveForRow(branchName: string): boolean {
  return inlineRowActionStore.getState().activeInline?.branchName === branchName
}

/** Id of the `<td>` the inline form of a row renders into; sanitized to be usable as a CSS `#id` selector. */
export function portalTargetIdFor(branchName: string): string {
  return `inline-form-${branchName.replace(/[^\w-]/g, "_")}`
}

// ---- actions ----

/**
 * Opens an inline form below the row; replaces any other open form.
 * The form components are keyed by `${type}:${branchName}`, so a retarget remounts them with fresh state.
 */
export function openInline(type: InlineActionType, branchName: string, conflictInfo?: MergeConflictInfo): void {
  if (isActive(type, branchName)) {
    return
  }
  inlineRowActionStore.setState({ activeInline: { type, branchName, conflictInfo, processing: false } })
}

export function closeInline(): void {
  inlineRowActionStore.setState({ activeInline: null })
}

/** Closes the inline form, then runs `fn` (after the form has unmounted). */
export function withPostSubmit(fn: () => void | Promise<void>): void {
  closeInline()
  queueMicrotask(() => {
    void fn()
  })
}

/**
 * Runs `fn` while the row's inline form shows its progress state, reports the outcome with a toast and closes the form.
 * Resolves to the value of `fn`, or `undefined` if it failed.
 */
export async function withRowProcessing<T>(key: string, fn: () => Promise<T>, options: RowProcessingOptions<T>): Promise<T | undefined> {
  const activeInline = inlineRowActionStore.getState().activeInline
  if (activeInline?.branchName === key) {
    inlineRowActionStore.setState({ activeInline: { ...activeInline, processing: true, processingMessage: options.processingMessage } })
  }

  let value: T
  try {
    value = await fn()
  }
  catch (error) {
    // the error handler may open another inline (e.g. the amend conflict viewer), so close first
    closeInline()
    showToast("error", options.error(error))
    return undefined
  }

  closeInline()
  showToast("success", options.success(value))
  return value
}

function showToast(type: "success" | "error", message: ToastMessage): void {
  toast.add({
    type,
    title: message.title,
    description: message.description,
    ...(message.duration === undefined ? {} : { timeout: message.duration }),
  })
}
