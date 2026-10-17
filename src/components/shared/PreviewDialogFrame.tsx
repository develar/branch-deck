import { useEffect } from "react"
import type { ComponentProps, KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Kbd, KbdGroup } from "@/components/ui/kbd"

/**
 * `sm` a picker, `md` a question or a help text, `lg` (default) a preview of commits or files, `xl` a preview with a
 * side rail (a pull request), `full` the window (a review)
 */
export type PreviewDialogWidth = "sm" | "md" | "lg" | "xl" | "full"

const widthClasses: Record<PreviewDialogWidth, string> = {
  sm: "sm:max-w-xs",
  md: "sm:max-w-lg",
  lg: "sm:max-w-[min(56rem,calc(100%-4rem))]",
  xl: "sm:max-w-[min(72rem,calc(100%-4rem))]",
  full: "h-[calc(100dvh-2rem)] max-h-none sm:max-w-[calc(100%-2rem)]",
}

export interface PreviewDialogFrameProps {
  open: boolean
  onOpenChange: ComponentProps<typeof Dialog>["onOpenChange"]
  /** Focused when the dialog opens: the body, so PageUp/PageDown scroll it (or what a function picks, as Base UI takes it) */
  initialFocus: RefObject<HTMLElement | null> | (() => HTMLElement | boolean | null)
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
        showCloseButton={showCloseButton}
        className={cn("max-h-[min(80vh,48rem)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0", widthClasses[width])}
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
  /** The content fills the body and scrolls itself (the panes of a review): no padding, no scrolling */
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
 */
export function PreviewDialogFooter({ start, className, children, "data-testid": testId }: { start?: ReactNode; className?: string; children: ReactNode; "data-testid"?: string }) {
  return (
    <div data-testid={testId} className={cn("flex items-center justify-between gap-4 border-t border-border px-6 py-2 text-xs text-muted-foreground", className)}>
      {start}
      <div className="ml-auto flex items-center gap-3">{children}</div>
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
