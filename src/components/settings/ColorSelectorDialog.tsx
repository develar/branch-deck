import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"

interface ColorSelectorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

// TODO(Phase 9): port ColorSelectorModal.vue (primary colour grid → applyPrimaryColor + setPrimaryColor)
export function ColorSelectorDialog({ open, onOpenChange }: ColorSelectorDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-72">
        <DialogHeader>
          <DialogTitle>Theme</DialogTitle>
        </DialogHeader>
        <p className="text-muted-foreground">Colour selection is not implemented yet.</p>
      </DialogContent>
    </Dialog>
  )
}
