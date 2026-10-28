import { useEffect } from "react"
import type { ComponentProps, KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Kbd, KbdGroup } from "@/components/ui/kbd"

/**
 * `sm` a picker, `md` a question or a help text, `form` a form with a text field and several actions (the pull
 * request), `lg` (default) a preview of commits or files; a review is a window
 */
export type PreviewDialogWidth = "sm" | "md" | "form" | "lg"

const widthClasses: Record<PreviewDialogWidth, string> = {
  sm: "sm:max-w-xs",
  md: "sm:max-w-lg",
  // any `sm:max-w-*` replaces the 1rem margin of the base class: the review window at 150% is 640 CSS px wide
  form: "sm:max-w-[min(48rem,calc(100%-2rem))]",
  lg: "sm:max-w-[min(56rem,calc(100%-4rem))]",
}

export interface PreviewDialogFrameProps {
  open: boolean
  onOpenChange: ComponentProps<typeof Dialog>["onOpenChange"]
  /** Focused when the dialog opens: the body, so PageUp/PageDown scroll it (or what a function picks, as Base UI takes it) */
  initialFocus: RefObject<HTMLElement | null> | (() => HTMLElement | boolean | null)
  /**
   * Focused when the dialog closes, as Base UI takes it: the row of what the preview showed last (↑/↓ stepped), so the
   * keys of a row go on from there; the element focused before it opened when not set
   */
  finalFocus?: () => HTMLElement | boolean | null
  width?: PreviewDialogWidth
  onKeyDown?: (event: KeyboardEvent<HTMLDivElement>) => void
  /** The built-in × in the corner; off for a dialog whose toolbar has its own */
  showCloseButton?: boolean
  "data-testid"?: string
  /** `PreviewDialogHeader`, `PreviewDialogBody` and `PreviewDialogFooter`, in this order */
  children: ReactNode
}

/**
 * Centered dialog in the layout of the Quick Look-like previews: a header above a `border-border` rule, a body that
 * alone scrolls, and a footer of key hints below a rule.
 *
 * Keep it mounted while closed: Base UI moves the focus into a dialog (and back) only when `open` changes.
 */
export function PreviewDialogFrame({
  open,
  onOpenChange,
  initialFocus,
  finalFocus,
  width = "lg",
  onKeyDown,
  showCloseButton = true,
  "data-testid": testId,
  children,
}: PreviewDialogFrameProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        data-testid={testId}
        initialFocus={initialFocus}
        finalFocus={finalFocus}
        showCloseButton={showCloseButton}
        // the column is the dialog's width: an `auto` track grows with its widest row, so a footer wider than the
        // dialog would widen every row past its card
        className={cn("max-h-[min(80vh,48rem)] grid-cols-[minmax(0,1fr)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0", widthClasses[width])}
        onKeyDown={onKeyDown}
      >
        {children}
      </DialogContent>
    </Dialog>
  )
}

/** The title and what goes with it; `end` on the right, before the close button: the view of the body (Unified/Split) */
export function PreviewDialogHeader({ children, end }: { children: ReactNode; end?: ReactNode }) {
  // one structure with or without `end`: a dialog that shows it in one mode (the review) keeps its title mounted
  return (
    // pr-14: room for the close button
    <div className="flex items-start gap-4 border-b border-border py-4 pr-14 pl-6">
      <div className="flex min-w-0 flex-1 flex-col gap-1">{children}</div>
      {end !== undefined && <div className="flex shrink-0 items-center gap-2">{end}</div>}
    </div>
  )
}

export interface PreviewDialogBodyProps {
  ref?: RefObject<HTMLDivElement | null>
  /** The shown item: a newly shown item starts at the top */
  itemKey?: string
  /** The content fills the body and scrolls itself (the panes of the review of a pull request): no padding, no scrolling */
  fill?: boolean
  "data-testid"?: string
  children: ReactNode
}

export function PreviewDialogBody({ ref, itemKey, fill = false, "data-testid": testId, children }: PreviewDialogBodyProps) {
  useEffect(() => {
    ref?.current?.scrollTo({ top: 0 })
  }, [ref, itemKey])

  return (
    <div ref={ref} tabIndex={-1} data-testid={testId} className={fill ? "min-h-0 overflow-hidden outline-none" : "overflow-y-auto overscroll-contain px-6 py-5 outline-none"}>
      {children}
    </div>
  )
}

/**
 * `start` on the left (the position or a hint), the key hints and the actions (`children`) on the right. `className`
 * lays it out elsewhere: on the canvas of a window, below its islands.
 *
 * Where they don't fit in one line, a text `start` wraps beside the actions first (its basis is 0, so the actions
 * don't jump to the next line while a hint grows); only what can't wrap (a button) moves the actions below it. The
 * actions keep to the bottom line: a `start` that wraps when an action comes (Create Pull Request… once its facts
 * answer) grows the footer upwards, so a button at the bottom of a dialog as tall as it gets stays where it was.
 */
export function PreviewDialogFooter({ start, className, children, "data-testid": testId }: { start?: ReactNode; className?: string; children: ReactNode; "data-testid"?: string }) {
  return (
    <div data-testid={testId} className={cn("flex flex-wrap items-end gap-x-4 gap-y-2 border-t border-border px-6 py-2 text-xs text-muted-foreground", className)}>
      {start !== undefined && start !== null && start !== false && <div className="flex flex-1 items-center self-center">{start}</div>}
      <div className="ml-auto flex flex-wrap items-center justify-end gap-x-3 gap-y-2">{children}</div>
    </div>
  )
}

export function KeyHint({ keys, children }: { keys: ReactNode; children: ReactNode }) {
  return (
    <span className="flex items-center gap-1.5">
      <KbdGroup>{keys}</KbdGroup>
      {children}
    </span>
  )
}

/** `↑ ↓ Previous/next` (not for a list of one) and `Space Close` (`Esc Close` in a window, where Space scrolls). */
export function PreviewNavigationHints({ count, closeKey = "Space" }: { count: number; closeKey?: "Space" | "Esc" }) {
  return (
    <>
      {count > 1 && (
        <KeyHint
          keys={
            <>
              <Kbd>↑</Kbd>
              <Kbd>↓</Kbd>
            </>
          }
        >
          Previous/next
        </KeyHint>
      )}
      <KeyHint keys={<Kbd>{closeKey}</Kbd>}>Close</KeyHint>
    </>
  )
}
