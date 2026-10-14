import { useId, useRef } from "react"
import colors from "tailwindcss/colors"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
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
  const bodyRef = useRef<HTMLDivElement>(null)
  const labelId = useId()
  return (
    <PreviewDialogFrame data-testid="color-selector-dialog" width="sm" open={open} onOpenChange={onOpenChange} initialFocus={bodyRef}>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Theme</DialogTitle>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef}>
        <div role="group" aria-labelledby={labelId} className="flex flex-col gap-2">
          <p id={labelId} className="text-xs font-medium text-muted-foreground">Primary</p>
          <div className="grid grid-cols-3 gap-1">
            {PRIMARY_COLOR_OPTIONS.map((color) => {
              const isSelected = primaryColor === color.name
              return (
                <Button
                  key={color.name}
                  variant="outline"
                  size="xs"
                  aria-pressed={isSelected}
                  className={cn("justify-start capitalize", isSelected && "bg-muted")}
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
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
