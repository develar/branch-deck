import type { ComponentProps } from "react"
import { cn } from "cn"

export function Container({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      className={cn("w-full px-2", className)}
      {...props}
    />
  )
}
