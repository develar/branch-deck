import { ArrowLeftIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { modifierKey } from "@/lib/platform"

/**
 * Back at the start of the title bar of the review window, while it has a review it left: starts that one over where
 * it was, as ⌘[ does
 */
export function ReviewBackButton({ name, onClick }: { name: string; onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Button variant="ghost" size="icon-sm" aria-label={`Back to ${name}`} data-testid="review-back" onClick={onClick} />}>
        <ArrowLeftIcon />
      </TooltipTrigger>
      <TooltipContent className="flex items-center gap-1.5">
        {`Back to ${name}`}
        <Kbd>{`${modifierKey}[`}</Kbd>
      </TooltipContent>
    </Tooltip>
  )
}
