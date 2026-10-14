import type { MouseEvent } from "react"
import { CopyCheckIcon, CopyIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useCopyToClipboard } from "@/hooks/useCopyToClipboard"

export interface CopyButtonProps {
  /** Evaluated only on click */
  text: () => string
  tooltip?: string
  size?: "icon-xs" | "icon-sm" | "icon"
  variant?: "ghost" | "outline"
  /** Otherwise the button is transparent until the parent (`.group`) is hovered */
  alwaysVisible?: boolean
  className?: string
}

/** Icon button copying text; its tooltip switches to "Copied" in place for a moment. */
export function CopyButton({ text, tooltip = "Copy to clipboard", size = "icon-xs", variant = "ghost", alwaysVisible = false, className }: CopyButtonProps) {
  const { isCopied, tooltipOpen, setTooltipOpen, copyToClipboard } = useCopyToClipboard()
  const tooltipText = isCopied ? "Copied" : tooltip

  const handleClick = (event: MouseEvent) => {
    event.stopPropagation()
    void copyToClipboard(text())
  }

  return (
    <div className={cn("copy-button transition-opacity", !alwaysVisible && "opacity-0", className)}>
      <Tooltip open={tooltipOpen} onOpenChange={setTooltipOpen}>
        <TooltipTrigger
          // keep the tooltip open on click, so its text switches to "Copied" in place
          closeOnClick={false}
          render={(
            <Button
              size={size}
              variant={variant}
              aria-label={tooltipText}
              className={cn(alwaysVisible && "text-muted-foreground")}
              onClick={handleClick}
            />
          )}
        >
          {isCopied ? <CopyCheckIcon /> : <CopyIcon />}
        </TooltipTrigger>
        <TooltipContent>{tooltipText}</TooltipContent>
      </Tooltip>
    </div>
  )
}
