import type { ComponentProps } from "react"
import { cn } from "cn"

/** Joins adjacent controls (select/input + button) into one segmented control, like Nuxt UI `UFieldGroup`. */
export function FieldGroup({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="field-group"
      className={cn(
        "relative inline-flex items-center",
        "*:focus-visible:z-10 *:not-first:-ml-px *:not-first:rounded-l-none *:not-last:rounded-r-none",
        className,
      )}
      {...props}
    />
  )
}
