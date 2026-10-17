import { useEffect, useState } from "react"
import { cn } from "cn"
import { Spinner } from "@/components/ui/spinner"

/** How long a read may take before it says it loads: a cached read never shows the line, so it never flashes */
export const LOADING_LINE_DELAY_MS = 300

/**
 * "Loading changes…" with a spinner, once the read took `LOADING_LINE_DELAY_MS`; nothing before. Keyed by what it
 * waits for, so the next subject waits again
 */
export function LoadingLine({ children, className }: { children: string; className?: string }) {
  const [slow, setSlow] = useState(false)
  useEffect(() => {
    const timer = setTimeout(() => setSlow(true), LOADING_LINE_DELAY_MS)
    return () => clearTimeout(timer)
  }, [])
  return slow ? (
    <p className={cn("flex items-center gap-2 text-sm text-muted-foreground", className)}>
      <Spinner className="size-4" />
      {children}
    </p>
  ) : null
}
