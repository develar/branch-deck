import { useState } from "react"
import { SettingsIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

interface BranchPrefixHelpProps {
  disabled: boolean
  configured: boolean
}

/** Settings button next to the branch prefix input; opens {@link BranchPrefixHelpDialog}. */
export function BranchPrefixHelp({ disabled, configured }: BranchPrefixHelpProps) {
  const [open, setOpen] = useState(false)
  const warn = !configured && !disabled
  return (
    <>
      <Tooltip disabled={configured}>
        <TooltipTrigger
          render={(
            <Button
              variant="outline"
              size="icon-sm"
              disabled={disabled}
              aria-label="Configure branch prefix"
              className={cn(warn && "border-warning/50 text-warning hover:bg-warning/10 hover:text-warning")}
              onClick={() => setOpen(true)}
            />
          )}
        >
          <SettingsIcon />
        </TooltipTrigger>
        <TooltipContent>Please configure the branch prefix</TooltipContent>
      </Tooltip>
      <BranchPrefixHelpDialog open={open} onOpenChange={setOpen} />
    </>
  )
}

interface BranchPrefixHelpDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const inlineCodeClass = "rounded bg-background px-1 py-0.5 text-xs"

export function BranchPrefixHelpDialog({ open, onOpenChange }: BranchPrefixHelpDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="gap-0 p-0 sm:max-w-lg">
        {/* rendered as h2 (Base UI default); E2E reads the dialog title from `h2, h3` */}
        <DialogTitle className="sr-only">Configure Branch Prefix</DialogTitle>
        <div className="flex max-h-[90vh] flex-col gap-6 overflow-y-auto p-6">
          <p className="text-sm text-foreground">
            The branch prefix is automatically prepended to your virtual branches. Setting a personal prefix (like your username) helps identify your branches in a shared repository.
          </p>

          <div className="flex flex-col gap-4">
            {/* prefix for all repositories */}
            <div className="flex flex-col gap-3 rounded-lg border border-border bg-muted/50 p-4">
              <h3 className="text-sm font-semibold text-foreground">
                Set your global branch prefix
              </h3>
              <p className="text-sm text-muted-foreground">
                Run this command in your terminal to set a global prefix for all repositories:
              </p>
              <div className="flex flex-col gap-2">
                <div className="overflow-x-auto rounded bg-background p-3">
                  <code className="font-mono text-xs">git config --global branchdeck.branchPrefix yourUsername</code>
                </div>
                <p className="text-xs text-muted-foreground">
                  Replace "yourUsername" with your preferred prefix (e.g., your name or initials)
                </p>
              </div>
            </div>

            {/* how it works */}
            <div className="flex flex-col gap-3 rounded-lg border border-info/20 bg-info/5 p-4">
              <h3 className="text-sm font-semibold text-foreground">
                How It Works
              </h3>
              <p className="text-sm text-muted-foreground">
                Branch Deck groups commits based on message patterns:
              </p>
              <ul className="flex flex-col gap-2 text-sm">
                <PatternExample label="Explicit prefix:" message="(feature-login)" branch="john/virtual/feature-login" />
                <PatternExample label="Issue numbers:" message="ABC-123: Fix bug" branch="john/virtual/ABC-123" />
                <PatternExample label="With subsystem:" message="[threading] ABC-123: Fix bug" branch="john/virtual/ABC-123" />
              </ul>
              <p className="text-xs text-muted-foreground">
                Explicit prefixes take precedence over issue numbers. The
                {" "}
                <code className={inlineCodeClass}>virtual</code>
                {" "}
                prefix is always automatically prepended.
              </p>
            </div>

            {/* repository-specific */}
            <div className="flex flex-col gap-2 border-t border-border pt-4">
              <p className="text-sm text-muted-foreground">
                You can also set this value per repository using:
              </p>
              <div className="overflow-x-auto rounded bg-background p-3">
                <code className="font-mono text-xs">git config branchdeck.branchPrefix repoSpecificPrefix</code>
              </div>
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function PatternExample({ label, message, branch }: { label: string, message: string, branch: string }) {
  return (
    <li className="flex items-start gap-2">
      <span className="text-muted-foreground">•</span>
      <div className="flex flex-1 flex-col gap-1">
        <span className="font-medium">{label}</span>
        <div className="text-muted-foreground">
          <code className={inlineCodeClass}>{message}</code>
          <span className="mx-2">→</span>
          <code className={inlineCodeClass}>{branch}</code>
        </div>
      </div>
    </li>
  )
}
