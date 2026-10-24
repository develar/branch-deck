import { useEffect } from "react"
import type { ComponentProps, ReactNode } from "react"
import { cn } from "cn"
import { WindowTitleBar } from "@/components/shared/WindowTitleBar"
import { markWindowContentReady } from "@/lib/sub-window"

export interface WindowPageProps extends Omit<ComponentProps<"div">, "title"> {
  /** The start of the title bar: where the user is */
  title: ReactNode
  /** The end of the title bar: the view and the actions */
  titleEnd?: ReactNode
  /**
   * `fill`: the islands fill the window and scroll inside themselves (a review); `scroll`: they scroll under the title
   * bar, which stays (the conflict windows)
   */
  body: "fill" | "scroll"
  /** Below the islands, on the canvas: the position and the keys */
  footer?: ReactNode
  /**
   * The content says itself when it settled (`markWindowContentReady`): a review, once its first diffs are prepared.
   * Otherwise the page's render is its content, and the window shows with it.
   */
  readyByContent?: boolean
}

/**
 * The page of a sub-window: the title bar (`WindowTitleBar`) on the canvas, the islands below it, the footer below
 * them. The root takes the test id, the focus and the keys of the window. Its render of each open shows the window, which
 * is built hidden (`markWindowContentReady`, once per open), unless the content says it (`readyByContent`).
 */
export function WindowPage({ title, titleEnd, body, footer, readyByContent = false, className, children, ...props }: WindowPageProps) {
  useEffect(() => {
    if (!readyByContent) {
      markWindowContentReady()
    }
  })
  return (
    <div className={cn("flex h-dvh flex-col bg-canvas text-sm", className)} {...props}>
      <WindowTitleBar end={titleEnd}>{title}</WindowTitleBar>
      {body === "fill" ? (
        <div className="flex min-h-0 flex-1 flex-col gap-2 px-2 pb-2">
          {children}
          {footer}
        </div>
      ) : (
        <>
          <main className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-2 pb-2">{children}</main>
          {footer}
        </>
      )}
    </div>
  )
}
