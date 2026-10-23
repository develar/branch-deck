import { useEffect, useEffectEvent, useId, useRef } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { StopWaitingButton } from "@/components/shared/LockWaitNotice"
import { inlineSurface, useInlineProcessing, useInlineRowAction } from "@/stores/inline-row-action"
import { useIndicatorClaim } from "@/stores/operations"

export interface InlineFormProps {
  title: string
  /** Muted, below the title */
  description?: ReactNode
  dataTestId?: string
  /** Of the form and its progress row, after their own: a place that draws its own border below drops theirs */
  className?: string
  contentClassName?: string
  children: ReactNode

  showActions?: boolean
  /** Replaces the default Cancel/Submit buttons */
  actions?: ReactNode
  cancelText?: string
  submitText?: string
  submitIcon?: ReactNode
  /** `destructive` for a submit that deletes */
  submitVariant?: "default" | "destructive"
  canSubmit?: boolean
  /** Why the submit is disabled, on its hover; it keeps the focus for it (`focusableWhenDisabled`) */
  submitDisabledReason?: string | null
  loading?: boolean
  onCancel: () => void
  onSubmit?: () => void

  /** Element focused when the form mounts; defaults to the form container */
  autoFocusRef?: RefObject<HTMLElement | null>
  /** A confirmation without an input: the submit button takes the focus once it is enabled, so Enter confirms */
  focusSubmit?: boolean
}

// Escape on these elements is handled by them (popups) or by the form itself
const ownEscapeSelector = '[data-inline-form], [role="menu"], [role="dialog"], [role="listbox"]'

function isFocusLost(): boolean {
  return document.activeElement === null || document.activeElement === document.body
}

/**
 * Non-modal form rendered in place (e.g. in the `<td>` of the row below a table row): the title, the muted description,
 * the content and Cancel/Submit. While the active inline row action is processing, the form is replaced by a slim
 * progress row; when the form shows again (a failure), its input gets the focus back.
 * Escape cancels: inside the form, or anywhere outside other popups (so it works before the user focuses the form).
 */
export function InlineForm({
  title,
  description,
  dataTestId,
  className,
  contentClassName = "flex max-w-3xl flex-col gap-4",
  children,
  showActions = true,
  actions,
  cancelText = "Cancel",
  submitText = "Submit",
  submitIcon,
  submitVariant = "default",
  canSubmit = true,
  submitDisabledReason = null,
  loading = false,
  onCancel,
  onSubmit,
  autoFocusRef,
  focusSubmit = false,
}: InlineFormProps) {
  // the form shows the operation it started in place of itself (`withRowProcessing`, `withRowProgress`)
  const rowKey = useInlineRowAction(state => state.activeInline?.branchName ?? "")
  useIndicatorClaim(rowKey === "" ? undefined : inlineSurface(rowKey))
  const busy = useInlineProcessing(rowKey)
  const processing = busy !== null
  const processingMessage = busy === null ? undefined : `${busy}…`

  const containerRef = useRef<HTMLDivElement>(null)
  const submitRef = useRef<HTMLButtonElement>(null)
  const titleId = useId()
  const descriptionId = useId()
  const isSubmitEnabled = canSubmit && !loading

  const onCancelRef = useRef(onCancel)
  useEffect(() => {
    onCancelRef.current = onCancel
  })

  // the submit button of a confirmation (if enabled), else the input, else the container
  const focusForm = useEffectEvent(() => {
    const submit = focusSubmit ? submitRef.current : null
    // a submit disabled with a reason stays focusable: it is no target for Enter
    const target = submit !== null && !submit.disabled && submit.getAttribute("aria-disabled") !== "true" ? submit : (autoFocusRef?.current ?? containerRef.current)
    target?.focus()
  })

  // focus on open
  useEffect(() => {
    focusForm()
  }, [])

  // a confirmation enabled later (its data loaded, the push before it failed): Enter confirms, unless the user moved on
  useEffect(() => {
    if (focusSubmit && isSubmitEnabled && (isFocusLost() || document.activeElement === containerRef.current)) {
      submitRef.current?.focus()
    }
  }, [focusSubmit, isSubmitEnabled])

  // the progress row replaces the focused control: the form keeps the focus (Escape, the focus return on close); a
  // failure shows the form again
  const wasProcessingRef = useRef(processing)
  useEffect(() => {
    if (wasProcessingRef.current === processing) {
      return
    }
    wasProcessingRef.current = processing
    if (!isFocusLost() && containerRef.current?.contains(document.activeElement) !== true) {
      return
    }
    if (processing) {
      containerRef.current?.focus()
    } else {
      focusForm()
    }
  }, [processing])

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

  // the progress row keeps it for the screen reader
  const heading = (
    <div className={cn("flex flex-col gap-1", processing && "sr-only")}>
      <h2 id={titleId} className="text-sm font-medium">
        {title}
      </h2>
      {description !== undefined && (
        <p id={descriptionId} className="text-sm text-muted-foreground">
          {description}
        </p>
      )}
    </div>
  )

  return (
    <div
      ref={containerRef}
      role="dialog"
      aria-labelledby={titleId}
      aria-describedby={description === undefined ? undefined : descriptionId}
      tabIndex={-1}
      data-inline-form=""
      className="outline-none"
      onKeyDown={handleKeyDown}
    >
      {processing ? (
        <div
          key="progress"
          className={cn("animate-in border-b border-border bg-muted/50 bd-padding-content py-3 duration-200 fade-in-0 slide-in-from-top-1", className)}
          data-testid={dataTestId ? `${dataTestId}-progress` : undefined}
        >
          {heading}
          <div className="flex items-center gap-3">
            <Spinner className="size-4 shrink-0 text-primary" />
            <p className="min-w-0 flex-1 truncate text-sm font-medium text-foreground">{processingMessage || "Processing"}</p>
            <StopWaitingButton surface={inlineSurface(rowKey)} />
          </div>
        </div>
      ) : (
        <div key="form" className={cn("border-b border-border bg-muted/50 bd-padding-content", className)} data-testid={dataTestId}>
          <div className={contentClassName}>
            {heading}
            {children}
            {actions ??
              (showActions && (
                <div className="flex items-center justify-end gap-2 pt-2">
                  <Button size="xs" variant="ghost" onClick={onCancel}>
                    {cancelText}
                  </Button>
                  <SubmitButton
                    submitRef={submitRef}
                    variant={submitVariant}
                    enabled={isSubmitEnabled}
                    loading={loading}
                    // a reason only for a disabled submit that waits for nothing
                    reason={isSubmitEnabled || loading ? null : submitDisabledReason}
                    onSubmit={onSubmit}
                  >
                    {loading ? <Spinner data-icon="inline-start" /> : submitIcon}
                    {submitText}
                  </SubmitButton>
                </div>
              ))}
          </div>
        </div>
      )}
    </div>
  )
}

/** The submit of a form; disabled with a reason, it keeps the focus and the hover, and says why */
function SubmitButton({
  submitRef,
  variant,
  enabled,
  loading,
  reason,
  onSubmit,
  children,
}: {
  submitRef: RefObject<HTMLButtonElement | null>
  variant: "default" | "destructive"
  enabled: boolean
  loading: boolean
  reason: string | null
  onSubmit?: () => void
  children: ReactNode
}) {
  if (reason === null) {
    return (
      <Button ref={submitRef} size="xs" variant={variant} disabled={!enabled} aria-busy={loading || undefined} onClick={onSubmit}>
        {children}
      </Button>
    )
  }
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            ref={submitRef}
            size="xs"
            variant={variant}
            disabled
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            className="opacity-50"
            data-testid="inline-form-submit"
            data-disabled-reason={reason}
          />
        }
      >
        {children}
      </TooltipTrigger>
      <TooltipContent className="max-w-xs">{reason}</TooltipContent>
    </Tooltip>
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
