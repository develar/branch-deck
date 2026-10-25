// local customisation: literal `alert` class in the cva base, `success | warning | info` variants and a tinted `destructive` variant;
// the tint of a tone lies on the card colour, and the description keeps the full tone; no text size (the container sets it);
// the padding and the `size-4` icon of a block (`px-4 py-3`), not the dense `px-2 py-1.5` of the stock: an alert is a block of
// the window or a dialog, and its 13px text needs the room
import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

const alertVariants = cva(
  // literal "alert" class: E2E helpers locate alerts with `[role=alert], .alert`
  "alert group/alert relative grid w-full gap-0.5 rounded-lg border px-4 py-3 text-left has-data-[slot=alert-action]:relative has-data-[slot=alert-action]:pr-18 has-[>svg]:grid-cols-[auto_1fr] has-[>svg]:gap-x-2.5 *:[svg]:row-span-2 *:[svg]:translate-y-0.5 *:[svg]:text-current *:[svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "bg-card text-card-foreground",
        // subtle semantic variants: tinted background + coloured text. An alert lies on the canvas: the tint is mixed with
        // the card, so the alert is an island and its text keeps 5:1 (docs/design/ui-theme.md, "Palette")
        destructive: "border-destructive/25 bg-[color-mix(in_srgb,var(--destructive)_10%,var(--card))] text-destructive *:data-[slot=alert-description]:text-destructive",
        success: "border-success/25 bg-[color-mix(in_srgb,var(--success)_10%,var(--card))] text-success *:data-[slot=alert-description]:text-success",
        warning: "border-warning/25 bg-[color-mix(in_srgb,var(--warning)_10%,var(--card))] text-warning *:data-[slot=alert-description]:text-warning",
        info: "border-info/25 bg-[color-mix(in_srgb,var(--info)_10%,var(--card))] text-info *:data-[slot=alert-description]:text-info",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
)

function Alert({ className, variant, ...props }: React.ComponentProps<"div"> & VariantProps<typeof alertVariants>) {
  return <div data-slot="alert" role="alert" className={cn(alertVariants({ variant }), className)} {...props} />
}

function AlertTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-title"
      className={cn("font-medium group-has-[>svg]/alert:col-start-2 [&_a]:underline [&_a]:underline-offset-3 [&_a]:hover:text-foreground", className)}
      {...props}
    />
  )
}

function AlertDescription({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-description"
      className={cn(
        "text-balance text-muted-foreground md:text-pretty [&_a]:underline [&_a]:underline-offset-3 [&_a]:hover:text-foreground [&_p:not(:last-child)]:mb-4",
        className,
      )}
      {...props}
    />
  )
}

function AlertAction({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="alert-action" className={cn("absolute top-2.5 right-3", className)} {...props} />
}

export { Alert, AlertTitle, AlertDescription, AlertAction }
