import type { ComponentProps } from "react"
import { cn } from "cn"

/**
 * A path that gives way from its start: `…src/main/kotlin` keeps its last folder while it fits. `dir="rtl"` moves the
 * cut to the start, `bdi` keeps the path itself left to right. A path that fits starts at the left of the box however
 * wide the box is: the start of a rtl box is its right edge, so without `text-left` a box wider than its text (`flex-1`)
 * pushes the path to the right; a path wider than its box overflows from the start edge whatever the alignment, so the
 * "…" stays at the left.
 */
export function StartTruncatedText({ className, children, ...props }: Omit<ComponentProps<"span">, "dir">) {
  return (
    <span {...props} className={cn("min-w-0 truncate text-left", className)} dir="rtl">
      <bdi dir="ltr">{children}</bdi>
    </span>
  )
}
