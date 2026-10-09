import { useEffect, useId, useRef } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useInlineRowAction } from "@/stores/inline-row-action"

export interface InlineFormProps {
  title: string
  description?: string
  dataTestId?: string
  contentClassName?: string
  children: ReactNode

  showActions?: boolean
  /** Replaces the default Cancel/Submit buttons */
  actions?: ReactNode
  cancelText?: string
  submitText?: string
  submitIcon?: ReactNode
  canSubmit?: boolean
  loading?: boolean
  onCancel: () => void
  onSubmit?: () => void

  /** Element focused when the form mounts; defaults to the form container */
  autoFocusRef?: RefObject<HTMLElement | null>
}

// Escape on these elements is handled by them (popups) or by the form itself
const ownEscapeSelector = "[data-inline-form], [role=\"menu\"], [role=\"dialog\"], [role=\"listbox\"]"

/**
 * Non-modal form rendered in place (e.g. in the `<td>` of the row below a table row).
 * While the active inline row action is processing, the form is replaced by a slim progress row.
 * Escape cancels: inside the form, or anywhere outside other popups (so it works before the user focuses the form).
 */
export function InlineForm({
  title,
  description = "Enter the required information",
  dataTestId,
  contentClassName = "flex flex-col gap-4",
  children,
  showActions = true,
  actions,
  cancelText = "Cancel",
  submitText = "Submit",
  submitIcon,
  canSubmit = true,
  loading = false,
  onCancel,
  onSubmit,
  autoFocusRef,
}: InlineFormProps) {
  const processing = useInlineRowAction(state => state.activeInline?.processing === true)
  const processingMessage = useInlineRowAction(state => state.activeInline?.processing ? state.activeInline.processingMessage : undefined)

  const containerRef = useRef<HTMLDivElement>(null)
  const titleId = useId()
  const descriptionId = useId()

  const onCancelRef = useRef(onCancel)
  useEffect(() => {
    onCancelRef.current = onCancel
  })

  // focus on open
  useEffect(() => {
    (autoFocusRef?.current ?? containerRef.current)?.focus()
    // only on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Escape while the focus is outside the form (e.g. still on the page after choosing a context menu item)
  useEffect(() => {
    const handler = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) {
        return
      }
      if (event.target instanceof Element && event.target.closest(ownEscapeSelector) !== null) {
        return
      }
      event.preventDefault()
      // e.g. a focused commit list must not clear its selection
      event.stopPropagation()
      onCancelRef.current()
    }
    window.addEventListener("keydown", handler, true)
    return () => window.removeEventListener("keydown", handler, true)
  }, [])

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault()
      // e.g. a commit list must not clear its selection
      event.stopPropagation()
      onCancel()
    }
  }

  return (
    <div
      ref={containerRef}
      role="dialog"
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      tabIndex={-1}
      data-inline-form=""
      className="outline-none"
      onKeyDown={handleKeyDown}
    >
      <h2 id={titleId} className="sr-only">{title}</h2>
      <p id={descriptionId} className="sr-only">{description}</p>

      {processing
        ? (
            <div
              key="progress"
              className="border-b border-border bg-muted/50 bd-padding-content py-3 animate-in duration-200 fade-in-0 slide-in-from-top-1"
              data-testid={dataTestId ? `${dataTestId}-progress` : undefined}
            >
              <div className="flex items-center gap-3">
                <Spinner className="size-4 shrink-0 text-primary" />
                <p className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">
                  {processingMessage || "Processing"}
                </p>
              </div>
            </div>
          )
        : (
            <div key="form" className="border-b border-border bg-muted/50 bd-padding-content" data-testid={dataTestId}>
              <div className={contentClassName}>
                {children}
                {actions ?? (showActions && (
                  <div className="flex items-center justify-end gap-2 pt-2">
                    <Button size="xs" variant="ghost" onClick={onCancel}>{cancelText}</Button>
                    <Button size="xs" disabled={!canSubmit || loading} aria-busy={loading || undefined} onClick={onSubmit}>
                      {loading ? <Spinner data-icon="inline-start" /> : submitIcon}
                      {submitText}
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          )}
    </div>
  )
}

/** Thin indeterminate progress bar overlaying the top border of an inline form row. */
export function InlineRowProgressBar({ className }: { className?: string }) {
  return (
    <div className={cn("absolute top-0 left-0 h-px w-full overflow-hidden bg-accent", className)}>
      <div className="absolute inset-y-0 w-1/2 animate-carousel bg-primary" />
    </div>
  )
}
