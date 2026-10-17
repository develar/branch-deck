import type { ReactNode } from "react"
import { cn } from "cn"
import { Kbd } from "@/components/ui/kbd"

/** What a toolbar button does and its key, for its tooltip: "Sync branches ⌘R", "Settings ⌘," */
export function ShortcutHint({ kbd, className, children }: { kbd: string; className?: string; children: ReactNode }) {
  return (
    <span className={cn("flex items-center gap-1.5", className)}>
      {children}
      <Kbd>{kbd}</Kbd>
    </span>
  )
}
