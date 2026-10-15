import { useId, useRef } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { PRIMARY_PALETTES } from "@/lib/theme"
import { setPrimaryColor, useAppSettings } from "@/stores/settings/appSettings"

interface ColorSelectorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

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
            {PRIMARY_PALETTES.map((palette) => {
              const isSelected = primaryColor === palette.name
              return (
                <Button
                  key={palette.name}
                  variant="outline"
                  size="xs"
                  aria-pressed={isSelected}
                  className={cn("justify-start", isSelected && "bg-muted")}
                  onClick={() => {
                    setPrimaryColor(palette.name)
                  }}
                >
                  <span className="inline-block size-2 shrink-0 rounded-full" style={{ backgroundColor: palette.shades["600"] }} />
                  {palette.label}
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
