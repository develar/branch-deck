// local customisation: no fixed size: a `Button` or `Badge` sizes it, a standalone spinner passes `size-4` (`size-3` in `text-xs`)
import { cn } from "cn"
import { Loader2Icon } from "lucide-react"

function Spinner({ className, ...props }: React.ComponentProps<"svg">) {
  return (
    <Loader2Icon data-slot="spinner" role="status" aria-label="Loading" className={cn("animate-spin", className)} {...props} />
  )
}

export { Spinner }
