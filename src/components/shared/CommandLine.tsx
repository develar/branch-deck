import { cn } from "cn"
import { CopyButton } from "@/components/shared/CopyButton"

/** A shell command on its own line, scrolled rather than wrapped, with a copy button at its end */
export function CommandLine({ children, className }: { children: string; className?: string }) {
  return (
    <div className={cn("flex items-center gap-1 rounded-lg bg-muted py-1 pr-1 pl-3", className)}>
      <pre className="min-w-0 flex-1 overflow-x-auto py-1 font-mono text-xs text-foreground">{children}</pre>
      <CopyButton text={() => children} tooltip="Copy command" alwaysVisible />
    </div>
  )
}
