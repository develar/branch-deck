import type { ComponentProps } from "react"
import { cn } from "cn"

export function TableHeader({ className, ...props }: ComponentProps<"th">) {
  return <th className={cn("px-6 py-2 text-left text-xs font-medium text-muted-foreground", className)} {...props} />
}
