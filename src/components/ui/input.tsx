// local customisation: the focus is the `bd-focus` token of src/index.css, an outline (no ring, no `border-ring`),
// inside the field (`bd-focus-inset`), destructive when invalid
// local customisation: `text-sm` at any width (was `text-base md:text-sm`, for phones): a zoomed page is narrower than `md`
import * as React from "react"
import { Input as InputPrimitive } from "@base-ui/react/input"
import { cn } from "cn"

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      className={cn(
        "h-8 w-full min-w-0 rounded-2xl border border-transparent bg-input/50 px-2.5 py-1 text-sm transition-[color,box-shadow] duration-200 outline-none bd-focus-inset file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground focus-visible:bd-focus disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 aria-invalid:[--bd-focus-color:var(--destructive)] dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40",
        className,
      )}
      {...props}
    />
  )
}

export { Input }
