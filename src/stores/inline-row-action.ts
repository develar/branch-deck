import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import type { MergeConflictInfo } from "@/lib/bindings"

export type InlineActionType = "issue-reference" | "rename" | "delete-archived" | "amend-changes" | "amend-conflict" | "unapply" | "squash-fixups" | "ungroup" | "safe-push" | "dry-run"

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
  /** An error shows the form again with its input, instead of closing it */
  keepFormOnError?: boolean
}

export interface RowProgressOptions {
  processingMessage?: string
  /** A failure shows the form again with its input, instead of closing it */
  keepFormOnFailure?: boolean
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

// ---- focus ----

const inlineFormSelector = "[data-inline-form]"

/** The element that opened the form; null for a menu item (gone with its menu): the row gets the focus back instead */
let focusOrigin: { branchName: string, opener: HTMLElement | null } | null = null

function rememberFocusOrigin(branchName: string): void {
  if (typeof document === "undefined") {
    return
  }
  const active = document.activeElement
  const isOpener = active instanceof HTMLElement && active !== document.body && active.closest(`[role=menu], ${inlineFormSelector}`) === null
  focusOrigin = { branchName, opener: isOpener ? active : null }
}

function isFocusLost(): boolean {
  return document.activeElement === null || document.activeElement === document.body
}

/**
 * Gives the focus back to the opener of the closed form of `branchName`, else to its row (`BranchContextMenuRow` is
 * focusable); only a focus the form had: the user may have moved on while it processed.
 */
function returnFocus(branchName: string): void {
  const origin = focusOrigin
  focusOrigin = null
  if (typeof document === "undefined" || origin?.branchName !== branchName) {
    return
  }
  if (!isFocusLost() && document.activeElement!.closest(inlineFormSelector) === null) {
    return
  }
  // once the form is gone; something may have taken the focus by then (another form, a dialog)
  requestAnimationFrame(() => {
    if (!isFocusLost()) {
      return
    }
    const opener = origin.opener
    if (opener?.isConnected === true) {
      opener.focus({ preventScroll: true })
      // a hidden opener (a button of a closed popover or dialog) can't take it
      if (document.activeElement === opener) {
        return
      }
    }
    document.querySelector<HTMLElement>(`tr[data-branch-name="${CSS.escape(branchName)}"]`)?.focus({ preventScroll: true })
  })
}

// ---- actions ----

/**
 * Opens an inline form below the row; replaces any other open form.
 * The form components are keyed by `${type}:${branchName}`, so a retarget remounts them with fresh state.
 * Closing it gives the focus back to the element that opened it, else to the row.
 */
export function openInline(type: InlineActionType, branchName: string, conflictInfo?: MergeConflictInfo): void {
  if (isActive(type, branchName)) {
    return
  }
  rememberFocusOrigin(branchName)
  inlineRowActionStore.setState({ activeInline: { type, branchName, conflictInfo, processing: false } })
}

export function closeInline(): void {
  const closed = inlineRowActionStore.getState().activeInline
  inlineRowActionStore.setState({ activeInline: null })
  if (closed !== null) {
    returnFocus(closed.branchName)
  }
}

/** Closes the inline form of the row, unless the form of another row replaced it meanwhile */
function closeInlineOf(key: string): void {
  if (isActiveForRow(key)) {
    closeInline()
  }
}

/** The progress row of the row's inline form (`processing`), or the form again */
function setRowProcessing(key: string, processing: boolean, processingMessage?: string): void {
  const activeInline = inlineRowActionStore.getState().activeInline
  if (activeInline?.branchName === key) {
    inlineRowActionStore.setState({ activeInline: { ...activeInline, processing, processingMessage } })
  }
}

/**
 * Runs `fn` while the row's inline form shows its progress state, reports the outcome with a toast and closes the form
 * (after an error too, unless `keepFormOnError`). Resolves to the value of `fn`, or `undefined` if it failed.
 */
export async function withRowProcessing<T>(key: string, fn: () => Promise<T>, options: RowProcessingOptions<T>): Promise<T | undefined> {
  setRowProcessing(key, true, options.processingMessage)

  let value: T
  try {
    value = await fn()
  }
  catch (error) {
    if (options.keepFormOnError === true) {
      setRowProcessing(key, false)
    }
    else {
      // the error handler may open another inline (e.g. the amend conflict viewer), so close first
      closeInlineOf(key)
    }
    showToast("error", options.error(error))
    return undefined
  }

  closeInlineOf(key)
  showToast("success", options.success(value))
  return value
}

/**
 * Runs `fn`, which tells its outcome itself (a toast with Undo, an error toast), while the row's inline form shows its
 * progress state. Resolves to the result of `fn`: true closes the form; false closes it too, or shows it again with its
 * input (`keepFormOnFailure`).
 */
export async function withRowProgress(key: string, fn: () => Promise<boolean>, options: RowProgressOptions = {}): Promise<boolean> {
  setRowProcessing(key, true, options.processingMessage)
  let done = false
  try {
    done = await fn()
  }
  finally {
    if (done || options.keepFormOnFailure !== true) {
      closeInlineOf(key)
    }
    else {
      setRowProcessing(key, false)
    }
  }
  return done
}

function showToast(type: "success" | "error", message: ToastMessage): void {
  toast.add({
    type,
    title: message.title,
    description: message.description,
    ...(message.duration === undefined ? {} : { timeout: message.duration }),
  })
}
