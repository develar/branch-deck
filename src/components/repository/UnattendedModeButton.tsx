import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { FROM_MENU } from "@/lib/menu"
import { ghostButtonColorClass } from "@/lib/ui-types"
import { unattendedModeItem } from "@/lib/unattended"
import { UNATTENDED_MARK } from "@/presenters/unattended"
import { useAppSettings } from "@/stores/settings/appSettings"

/**
 * The mark of Unattended Mode in the title bar while it is on (openspec/specs/repository/unattended-mode/spec.md): the
 * moon and "Unattended" in the warning tone (the word where the header has room for it), so the user sees from across
 * the room that agents are on their own; a click turns the mode off, the item of the command palette. Nothing while the
 * mode is off: the palette and Settings → General turn it on
 */
export function UnattendedModeButton() {
  const on = useAppSettings(state => state.unattended)
  if (!on) {
    return null
  }
  const item = unattendedModeItem(true)
  const Icon = item.icon
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="sm"
            // the name of the mark; its tooltip says what a click does
            aria-label="Unattended Mode"
            data-testid="unattended-mode-button"
            // the word from 1088 px of the header (68rem), the tinted moon alone below, so the row keeps one line
            className={`bg-warning/10 pr-2 @min-[68rem]:pr-3 ${ghostButtonColorClass("warning")}`}
            onClick={() => item.onSelect(FROM_MENU)}
          />
        }
      >
        {Icon !== undefined && <Icon data-icon="inline-start" />}
        <span className="hidden @min-[68rem]:inline">{UNATTENDED_MARK}</span>
      </TooltipTrigger>
      <TooltipContent description={item.description}>{item.label}</TooltipContent>
    </Tooltip>
  )
}
