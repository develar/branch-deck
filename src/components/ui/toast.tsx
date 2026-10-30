// local customisation: typed `data.secondaryAction`, a body with the actions below the text when there are two (the
// primary one, then the secondary one, e.g. "Open in Patronus") or a description (the text keeps the width); the viewport `z-[60]` above the dialog overlays (`z-50`); the toast text `select-text`; the title
// and description `wrap-break-word`, so a path or branch name wider than the text column breaks instead of running
// under the action; `renderText` of the `Toaster` for a text title and description (the app links their issue keys,
// `App.tsx`; this file imports nothing of the app, which imports `toast` everywhere); the default `render` of the
// action and the close button in the body, not a parameter default (React Compiler cannot compile a JSX parameter
// default); an action whose label ends with "…" (it opens a dialog or a popover) closes its toast when pressed
// (`opensDialog`); `data.running` disables the primary action, `secondaryAction.disabled` the secondary one, while the
// operation it started runs ("Updating…", `src/lib/event-toast.ts`); the icon, the action beside the text and the close
// button align with the first line of the title (`items-start`), not with the middle of a long description
"use client"

import * as React from "react"
import { Toast as ToastPrimitive } from "@base-ui/react/toast"
import { cn } from "cn"
import { XIcon, CircleCheckIcon, InfoIcon, TriangleAlertIcon, OctagonXIcon, Loader2Icon } from "lucide-react"
import { Button } from "@/components/ui/button"

const toast = ToastPrimitive.createToastManager()

function ToastProvider({ ...props }: ToastPrimitive.Provider.Props) {
  return <ToastPrimitive.Provider {...props} />
}

function ToastPortal({ ...props }: ToastPrimitive.Portal.Props) {
  return <ToastPrimitive.Portal data-slot="toast-portal" {...props} />
}

function ToastViewport({ className, ...props }: ToastPrimitive.Viewport.Props) {
  return (
    <ToastPrimitive.Viewport
      data-slot="toast-viewport"
      className={cn("pointer-events-none fixed inset-x-4 bottom-4 z-[60] mx-auto w-auto max-w-sm outline-none sm:right-4 sm:left-auto sm:mx-0 sm:w-full", className)}
      {...props}
    />
  )
}

function Toast({ className, ...props }: ToastPrimitive.Root.Props) {
  return (
    <ToastPrimitive.Root
      data-slot="toast"
      className={cn(
        "group/toast pointer-events-auto absolute right-0 bottom-0 z-[calc(1000-var(--toast-index))] w-full origin-bottom rounded-2xl! border bg-popover text-popover-foreground shadow-lg will-change-transform outline-none select-text focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
        "[--gap:0.75rem] [--height:var(--toast-frontmost-height,var(--toast-height))] [--offset-y:calc(var(--toast-offset-y)*-1+calc(var(--toast-index)*var(--gap)*-1)+var(--toast-swipe-movement-y))] [--peek:0.75rem] [--scale:calc(max(0,1-(var(--toast-index)*0.1)))] [--shrink:calc(1-var(--scale))]",
        "h-(--height) [transform:translateX(var(--toast-swipe-movement-x))_translateY(calc(var(--toast-swipe-movement-y)-(var(--toast-index)*var(--peek))-(var(--shrink)*var(--height))))_scale(var(--scale))] [transition:transform_500ms_cubic-bezier(0.22,1,0.36,1),opacity_500ms,height_150ms]",
        "after:absolute after:top-full after:left-0 after:h-[calc(var(--gap)+1px)] after:w-full after:content-['']",
        "data-expanded:h-(--toast-height) data-expanded:[transform:translateX(var(--toast-swipe-movement-x))_translateY(var(--offset-y))]",
        "data-limited:opacity-0 data-starting-style:[transform:translateY(150%)]",
        "[&[data-ending-style]:not([data-limited]):not([data-swipe-direction])]:[transform:translateY(150%)]",
        "data-ending-style:data-[swipe-direction=down]:[transform:translateY(calc(var(--toast-swipe-movement-y)+150%))]",
        "data-ending-style:data-[swipe-direction=left]:[transform:translateX(calc(var(--toast-swipe-movement-x)-150%))_translateY(var(--offset-y))]",
        "data-ending-style:data-[swipe-direction=right]:[transform:translateX(calc(var(--toast-swipe-movement-x)+150%))_translateY(var(--offset-y))]",
        "data-ending-style:data-[swipe-direction=up]:[transform:translateY(calc(var(--toast-swipe-movement-y)-150%))]",
        "data-expanded:data-ending-style:data-[swipe-direction=down]:[transform:translateY(calc(var(--toast-swipe-movement-y)+150%))]",
        "data-expanded:data-ending-style:data-[swipe-direction=left]:[transform:translateX(calc(var(--toast-swipe-movement-x)-150%))_translateY(var(--offset-y))]",
        "data-expanded:data-ending-style:data-[swipe-direction=right]:[transform:translateX(calc(var(--toast-swipe-movement-x)+150%))_translateY(var(--offset-y))]",
        "data-expanded:data-ending-style:data-[swipe-direction=up]:[transform:translateY(calc(var(--toast-swipe-movement-y)-150%))]",
        className,
      )}
      {...props}
    />
  )
}

function ToastContent({ className, ...props }: ToastPrimitive.Content.Props) {
  return (
    <ToastPrimitive.Content
      data-slot="toast-content"
      className={cn(
        "flex h-full items-start gap-3 overflow-hidden p-4 transition-opacity duration-250 ease-[cubic-bezier(0.22,1,0.36,1)] data-behind:opacity-0 data-expanded:opacity-100",
        className,
      )}
      {...props}
    />
  )
}

function ToastTitle({ className, ...props }: ToastPrimitive.Title.Props) {
  return <ToastPrimitive.Title data-slot="toast-title" className={cn("text-sm font-medium wrap-break-word", className)} {...props} />
}

function ToastDescription({ className, ...props }: ToastPrimitive.Description.Props) {
  return <ToastPrimitive.Description data-slot="toast-description" className={cn("text-sm wrap-break-word text-muted-foreground", className)} {...props} />
}

function ToastAction({ className, render, ...props }: ToastPrimitive.Action.Props) {
  return <ToastPrimitive.Action data-slot="toast-action" render={render ?? <Button variant="outline" size="sm" />} className={cn("shrink-0", className)} {...props} />
}

function ToastClose({ className, children, render, ...props }: ToastPrimitive.Close.Props) {
  return (
    <ToastPrimitive.Close
      data-slot="toast-close"
      aria-label="Close toast"
      render={render ?? <Button variant="ghost" size="icon-sm" />}
      className={cn("relative -my-1 shrink-0 text-muted-foreground after:absolute after:-inset-2 after:content-[''] hover:text-foreground", className)}
      {...props}
    >
      {children ?? <XIcon aria-hidden="true" />}
    </ToastPrimitive.Close>
  )
}

function ToastIcon({ type }: { type: string | undefined }) {
  let icon: React.ReactNode = null

  if (type === "success") {
    icon = <CircleCheckIcon aria-hidden="true" />
  }

  if (type === "info") {
    icon = <InfoIcon aria-hidden="true" />
  }

  if (type === "warning") {
    icon = <TriangleAlertIcon aria-hidden="true" />
  }

  if (type === "error") {
    icon = <OctagonXIcon className="text-destructive" aria-hidden="true" />
  }

  if (type === "loading") {
    icon = <Loader2Icon className="animate-spin" aria-hidden="true" />
  }

  if (!icon) {
    return null
  }

  return (
    <span data-slot="toast-icon" className="flex h-5 shrink-0 items-center [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4">
      {icon}
    </span>
  )
}

/** Custom toast data */
interface ToastData {
  /** A second action after the primary one (`actionProps`): both go below the text, e.g. Fetch, then "Open in Patronus" */
  secondaryAction?: { label: string; onClick: () => void; disabled?: boolean }
  /** The operation of the primary action runs (from any surface): the action is disabled, its label says so */
  running?: boolean
}

/** Renders a text title or description of a toast, e.g. with its issue keys as links */
type ToastTextRenderer = (text: string, part: "title" | "description") => React.ReactNode

/** A text title or description through `renderText`; any other content (or none) is the toast's own */
function renderToastText(text: React.ReactNode, part: "title" | "description", renderText: ToastTextRenderer | undefined): React.ReactNode {
  return renderText !== undefined && typeof text === "string" && text !== "" ? renderText(text, part) : undefined
}

/**
 * An action that asks for more before it acts ends with "…" (docs/contributing/ux-writing.md) and opens a dialog or a
 * popover: its toast closes when it is pressed, as the toasts lie above the dialogs (`z-[60]`) and would cover the
 * footer of the dialog it opened until they time out
 */
function opensDialog(label: React.ReactNode): boolean {
  return typeof label === "string" && label.endsWith("…")
}

function ToastList({ renderText }: { renderText?: ToastTextRenderer }) {
  const { toasts, close } = ToastPrimitive.useToastManager<ToastData>()

  return toasts.map(toastItem => {
    const closeOnPress = opensDialog(toastItem.actionProps?.children) ? () => close(toastItem.id) : undefined
    const action = <ToastAction onClick={closeOnPress} disabled={toastItem.data?.running === true} />
    const secondaryAction = toastItem.data?.secondaryAction
    // an action beside a title and a description squeezes them into a narrow column: it goes below them, as two do
    const stacked = secondaryAction !== undefined || (toastItem.description != null && toastItem.actionProps != null)
    return (
      <Toast key={toastItem.id} toast={toastItem}>
        <ToastContent>
          <ToastIcon type={toastItem.type} />
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <ToastTitle>{renderToastText(toastItem.title, "title", renderText)}</ToastTitle>
            <ToastDescription>{renderToastText(toastItem.description, "description", renderText)}</ToastDescription>
            {stacked && (
              // the primary action first
              <div className="flex gap-2 pt-1">
                {action}
                {secondaryAction !== undefined && (
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={secondaryAction.disabled === true}
                    onClick={() => {
                      secondaryAction.onClick()
                      if (opensDialog(secondaryAction.label)) {
                        close(toastItem.id)
                      }
                    }}
                  >
                    {secondaryAction.label}
                  </Button>
                )}
              </div>
            )}
          </div>
          {!stacked && <ToastAction onClick={closeOnPress} disabled={toastItem.data?.running === true} className="-my-1" />}
          <ToastClose />
        </ToastContent>
      </Toast>
    )
  })
}

interface ToasterProps extends ToastPrimitive.Provider.Props {
  renderText?: ToastTextRenderer
}

function Toaster({ children, toastManager = toast, renderText, ...props }: ToasterProps) {
  return (
    <ToastProvider toastManager={toastManager} {...props}>
      {children}
      <ToastPortal>
        <ToastViewport>
          <ToastList renderText={renderText} />
        </ToastViewport>
      </ToastPortal>
    </ToastProvider>
  )
}

const createToastManager = ToastPrimitive.createToastManager
const useToastManager = ToastPrimitive.useToastManager

export {
  Toaster,
  Toast,
  ToastAction,
  ToastClose,
  ToastContent,
  ToastDescription,
  ToastPortal,
  ToastProvider,
  ToastTitle,
  ToastViewport,
  createToastManager,
  toast,
  useToastManager,
}
