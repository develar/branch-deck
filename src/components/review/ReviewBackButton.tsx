import { ArrowLeftIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { formatKbds } from "@/lib/menu"

/** The key of Back, as the review windows take it (`⌘[`, `Ctrl+[`) */
const BACK_KEY = formatKbds(["mod", "["])

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
      <TooltipContent className="flex items-center gap-1.5" description={`Shortcut: ${BACK_KEY}`}>
        {`Back to ${name}`}
        <Kbd>{BACK_KEY}</Kbd>
      </TooltipContent>
    </Tooltip>
  )
}
