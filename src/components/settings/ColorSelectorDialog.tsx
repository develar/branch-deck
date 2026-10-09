import colors from "tailwindcss/colors"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { PRIMARY_COLOR_NAMES } from "@/lib/theme"
import { setPrimaryColor, useAppSettings } from "@/stores/settings/appSettings"

interface ColorSelectorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface ColorOption {
  name: string
  hex: string
}

const PRIMARY_COLOR_OPTIONS: readonly ColorOption[] = PRIMARY_COLOR_NAMES.map(name => ({
  name,
  hex: (colors[name as keyof typeof colors] as Record<string, string | undefined>)["500"] ?? colors.blue[500],
}))

/**
 * "Theme…" menu dialog: picks the primary colour.
 * The choice is stored in the app settings; `App` applies the stored colour to the CSS variables.
 */
export function ColorSelectorDialog({ open, onOpenChange }: ColorSelectorDialogProps) {
  const primaryColor = useAppSettings(state => state.primaryColor)
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-72" data-testid="color-selector-dialog">
        <DialogHeader>
          <DialogTitle>Theme</DialogTitle>
        </DialogHeader>
        <fieldset>
          <legend className="mb-2 text-[11px] leading-none font-semibold">Primary</legend>
          <div className="grid grid-cols-3 gap-1">
            {PRIMARY_COLOR_OPTIONS.map((color) => {
              const isSelected = primaryColor === color.name
              return (
                <Button
                  key={color.name}
                  variant="outline"
                  size="sm"
                  aria-pressed={isSelected}
                  className={cn("justify-start rounded-sm text-[11px] capitalize", isSelected && "bg-muted")}
                  onClick={() => {
                    setPrimaryColor(color.name)
                  }}
                >
                  <span className="inline-block size-2 shrink-0 rounded-full" style={{ backgroundColor: color.hex }} />
                  {color.name}
                </Button>
              )
            })}
          </div>
        </fieldset>
      </DialogContent>
    </Dialog>
  )
}
