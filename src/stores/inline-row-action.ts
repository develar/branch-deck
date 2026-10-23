import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { toast } from "@/components/ui/toast"
import type { MergeConflictInfo } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import type { ToastMessage } from "@/lib/toast-message"
import { showToast } from "@/lib/toast-message"
import type { OperationKind } from "@/stores/operations"
import { branchSubject, runOperation, useSurfaceBusy } from "@/stores/operations"

export type InlineActionType =
  | "issue-reference"
  | "rename"
  | "delete-archived"
  | "delete-unapplied"
  | "amend-changes"
  | "amend-conflict"
  | "unapply"
  | "squash-fixups"
  | "ungroup"
  | "close-stack-pull-request"
  | "park"

export interface ActiveInline {
  type: InlineActionType
  branchName: string
  /** For the amend-conflict type */
  conflictInfo?: MergeConflictInfo
}

/** The operation an inline form runs (`runOperation`); its row is its object, its form the surface that shows it */
export interface RowOperation {
  /** The id of the item that opened the form ("unapply") */
  id: string
  kind: OperationKind
  /** The busy state the form shows in place of itself, without the ellipsis: "Unapplying feature-auth" */
  label: string
  /**
   * The repository of the start: a rewrite that waits in the queue runs there, whichever the main window shows by then;
   * the selected one by default
   */
  repositoryPath?: string
}

export interface RowProcessingOptions<T> {
  success: (value: T) => ToastMessage
  /**
   * The title of the error toast ("Cannot unapply feature-auth"); `notifyError` makes the error its description. Null
   * for an error shown otherwise (the amend conflict opens its viewer below the row)
   */
  errorTitle: (error: unknown) => string | null
  /** An error shows the form again with its input, instead of closing it */
  keepFormOnError?: boolean
}

export interface RowProgressOptions {
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

/** The active inline action if it belongs to the given row, otherwise null */
export function useActiveInlineForRow(branchName: string): ActiveInline | null {
  return useStore(inlineRowActionStore, state => (state.activeInline?.branchName === branchName ? state.activeInline : null))
}

/** The surface of the inline form of a row: it shows the operation it started in place of itself */
export function inlineSurface(key: string): string {
  return `inline:${key}`
}

/** The busy state of the operation the inline form of the row started ("Unapplying feature-auth"), null while none runs */
export function useInlineProcessing(key: string): string | null {
  return useSurfaceBusy(inlineSurface(key))
}

// ---- queries ----

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
 * Opens an inline form below the row; replaces any other open form. The form components are keyed by
 * `${type}:${branchName}`, so a retarget remounts them with fresh state. Closing it gives the focus back to the element
 * that opened it, else to the row.
 */
export function openInline(type: InlineActionType, branchName: string, options: { conflictInfo?: MergeConflictInfo } = {}): void {
  const active = isActive(type, branchName)
  if (active) {
    return
  }
  rememberFocusOrigin(branchName)
  inlineRowActionStore.setState({ activeInline: { type, branchName, conflictInfo: options.conflictInfo } })
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

type Outcome<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: unknown }

/**
 * Runs `fn` as the operation of the row (`runOperation`), while the row's inline form shows its progress state, reports
 * the outcome with a toast (an error through `notifyError`: sticky, logged, in Activity) and closes the form (after an
 * error too, unless `keepFormOnError`). `fn` includes the sync that shows its result: the form shows the progress until
 * then. Resolves to the value of `fn`, or `undefined` if it failed or had to wait for another operation (said in a
 * toast, the form kept).
 */
export async function withRowProcessing<T>(key: string, operation: RowOperation, fn: () => Promise<T>, options: RowProcessingOptions<T>): Promise<T | undefined> {
  const outcome = await runOperation<Outcome<T>>(
    { ...operation, subject: branchSubject(key), objects: [key], surface: inlineSurface(key) },
    () =>
      fn().then(
        value => ({ ok: true, value }),
        (error: unknown) => ({ ok: false, error }),
      ),
    reason => toast.add({ type: "info", title: options.errorTitle(reason) ?? operation.label, description: reason }),
  )
  if (outcome === undefined) {
    return undefined
  }
  if (!outcome.ok) {
    if (options.keepFormOnError !== true) {
      // the error handler may open another inline (e.g. the amend conflict viewer), so close first
      closeInlineOf(key)
    }
    const errorTitle = options.errorTitle(outcome.error)
    if (errorTitle !== null) {
      notifyError(errorTitle, outcome.error)
    }
    return undefined
  }

  closeInlineOf(key)
  showToast(options.success(outcome.value))
  return outcome.value
}

/**
 * Runs `fn`, an operation that tells its outcome itself (`runRewrite`: a toast with Undo, an error toast) and is started
 * from the form (`inlineSurface(key)`), so that the form shows its progress state. Resolves to the result of `fn`: true
 * closes the form; false closes it too, or shows it again with its input (`keepFormOnFailure`).
 */
export async function withRowProgress(key: string, fn: () => Promise<boolean>, options: RowProgressOptions = {}): Promise<boolean> {
  let done = false
  try {
    done = await fn()
  } finally {
    if (done || options.keepFormOnFailure !== true) {
      closeInlineOf(key)
    }
  }
  return done
}
