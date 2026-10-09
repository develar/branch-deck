import type { ComponentProps } from "react"
import { cn } from "cn"

export function TableHeader({ className, ...props }: ComponentProps<"th">) {
  return <th className={cn("px-6 py-3 text-left text-sm font-medium text-foreground", className)} {...props} />
}
