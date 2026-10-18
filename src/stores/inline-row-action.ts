import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { MergeConflictInfo } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { ToastMessage } from "@/lib/toast-message"
import { showToast } from "@/lib/toast-message"

export type InlineActionType =
  | "issue-reference"
  | "rename"
  | "delete-archived"
  | "amend-changes"
  | "amend-conflict"
  | "unapply"
  | "squash-fixups"
  | "ungroup"
  | "safe-push"
  | "dry-run"

/**
 * Where an inline form renders: below the row of its branch (`row`), or under the header of the stack island whose main
 * line the branch ends (`stack-header`, the Safe Push of the island), also while the island is collapsed. The key, the
 * processing and the holds of the form stay the branch's either way.
 */
export type InlinePlacement = "row" | "stack-header"

export interface ActiveInline {
  type: InlineActionType
  branchName: string
  /** Below the row when absent */
  placement?: InlinePlacement
  /** For the amend-conflict type */
  conflictInfo?: MergeConflictInfo
  /** For the safe-push type: a Safe Push without review, with a merge profile that skips the code-owner review */
  bypassReview?: boolean
  processing: boolean
  processingMessage?: string
  /**
   * The form pushes the branch as the first step of its action (a Safe Push start, the push alone of its form): the
   * button of that action shows the push, the push button of the row is disabled without it
   */
  pushing?: boolean
}

export interface RowProcessingOptions<T> {
  success: (value: T) => ToastMessage
  /**
   * The title of the error toast ("Cannot unapply feature-auth"); `notifyError` makes the error its description. Null
   * for an error shown otherwise (the amend conflict opens its viewer below the row)
   */
  errorTitle: (error: unknown) => string | null
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

/** The active inline action if it belongs to the given row, otherwise null; one under an island header too (`placement`). */
export function useActiveInlineForRow(branchName: string): ActiveInline | null {
  return useStore(inlineRowActionStore, state => (state.activeInline?.branchName === branchName ? state.activeInline : null))
}

/** Where the form of the active inline action renders (`InlinePlacement`) */
export function placementOf(activeInline: ActiveInline): InlinePlacement {
  return activeInline.placement ?? "row"
}

/** The active inline action of `branchName` that renders under the header of its stack island, otherwise null */
export function useStackHeaderInline(branchName: string | undefined): ActiveInline | null {
  return useStore(inlineRowActionStore, state =>
    state.activeInline !== null && state.activeInline.branchName === branchName && placementOf(state.activeInline) === "stack-header" ? state.activeInline : null,
  )
}

export function useIsRowProcessing(key: string): boolean {
  return useStore(inlineRowActionStore, state => isProcessingIn(state, key))
}

/** The form of the row pushes the branch as a step of its action (`ActiveInline.pushing`) */
export function useIsInlinePushing(branchName: string): boolean {
  return useStore(inlineRowActionStore, state => state.activeInline?.branchName === branchName && state.activeInline.pushing === true)
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
let focusOrigin: { branchName: string; opener: HTMLElement | null } | null = null

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
 * Opens an inline form below the row (or under the header of its stack island, `placement`); replaces any other open
 * form. The form components are keyed by `${type}:${branchName}`, so a retarget remounts them with fresh state.
 * Closing it gives the focus back to the element that opened it, else to the row.
 */
export function openInline(
  type: InlineActionType,
  branchName: string,
  options: { conflictInfo?: MergeConflictInfo; bypassReview?: boolean; placement?: InlinePlacement } = {},
): void {
  const bypassReview = options.bypassReview === true
  const placement = options.placement ?? "row"
  const current = inlineRowActionStore.getState().activeInline
  const active = isActive(type, branchName)
  // the same form in another mode (a Safe Push without review asked while the Safe Push form is open) or in another
  // place (the header of the island asks for the form open below the row) is opened again
  if (active && current !== null && (current.bypassReview === true) === bypassReview && placementOf(current) === placement) {
    return
  }
  // a form of the row replaced by itself keeps the focus origin of the first opening: the focus is inside it now
  if (!active) {
    rememberFocusOrigin(branchName)
  }
  inlineRowActionStore.setState({
    activeInline: { type, branchName, conflictInfo: options.conflictInfo, bypassReview, processing: false, ...(placement === "stack-header" && { placement }) },
  })
}

/** Marks the push of the form of the row as running or done; nothing when another form replaced it meanwhile */
export function setInlinePushing(branchName: string, pushing: boolean): void {
  const activeInline = inlineRowActionStore.getState().activeInline
  if (activeInline?.branchName === branchName && (activeInline.pushing === true) !== pushing) {
    inlineRowActionStore.setState({ activeInline: { ...activeInline, pushing } })
  }
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
 * Runs `fn` while the row's inline form shows its progress state, reports the outcome with a toast (an error through
 * `notifyError`: sticky, logged, in Activity) and closes the form (after an error too, unless `keepFormOnError`).
 * Resolves to the value of `fn`, or `undefined` if it failed.
 */
export async function withRowProcessing<T>(key: string, fn: () => Promise<T>, options: RowProcessingOptions<T>): Promise<T | undefined> {
  setRowProcessing(key, true, options.processingMessage)

  let value: T
  try {
    value = await fn()
  } catch (error) {
    if (options.keepFormOnError === true) {
      setRowProcessing(key, false)
    } else {
      // the error handler may open another inline (e.g. the amend conflict viewer), so close first
      closeInlineOf(key)
    }
    const errorTitle = options.errorTitle(error)
    if (errorTitle !== null) {
      notifyError(errorTitle, error)
    }
    return undefined
  }

  closeInlineOf(key)
  showToast(options.success(value))
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
  } finally {
    if (done || options.keepFormOnFailure !== true) {
      closeInlineOf(key)
    } else {
      setRowProcessing(key, false)
    }
  }
  return done
}
