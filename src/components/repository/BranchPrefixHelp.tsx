import { useRef, useState } from "react"
import type { ReactNode } from "react"
import { Settings2Icon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { InlineCode } from "@/components/shared/InlineCode"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"

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
      <Tooltip>
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
          <Settings2Icon />
        </TooltipTrigger>
        <TooltipContent>{configured ? "Configure branch prefix" : "Please configure the branch prefix"}</TooltipContent>
      </Tooltip>
      <BranchPrefixHelpDialog open={open} onOpenChange={setOpen} />
    </>
  )
}

interface BranchPrefixHelpDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** How to set the branch prefix: the global command, the grouping patterns, the per-repository command. */
export function BranchPrefixHelpDialog({ open, onOpenChange }: BranchPrefixHelpDialogProps) {
  const bodyRef = useRef<HTMLDivElement>(null)
  return (
    <PreviewDialogFrame data-testid="branch-prefix-help-dialog" width="md" open={open} onOpenChange={onOpenChange} initialFocus={bodyRef}>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Configure the branch prefix</DialogTitle>
        <p className="text-xs text-muted-foreground">
          The branch prefix is automatically prepended to your virtual branches. Setting a personal prefix (like your username) helps identify your branches in a shared repository.
        </p>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef}>
        <div className="flex flex-col gap-6 text-sm">
          <HelpBlock title="Set your global branch prefix">
            <p className="text-muted-foreground">Run this command in your terminal to set a global prefix for all repositories:</p>
            <CommandLine>git config --global branchdeck.branchPrefix yourUsername</CommandLine>
            <p className="text-xs text-muted-foreground">
              Replace "yourUsername" with your preferred prefix (e.g., your name or initials)
            </p>
          </HelpBlock>

          <HelpBlock title="How it works">
            <p className="text-muted-foreground">Branch Deck groups commits based on message patterns:</p>
            <ul className="flex flex-col gap-2">
              <PatternExample label="Explicit prefix:" message="(feature-login)" branch="john/virtual/feature-login" />
              <PatternExample label="Issue numbers:" message="ABC-123: Fix bug" branch="john/virtual/ABC-123" />
              <PatternExample label="With subsystem:" message="[threading] ABC-123: Fix bug" branch="john/virtual/ABC-123" />
            </ul>
            <p className="text-xs text-muted-foreground">
              {"Explicit prefixes take precedence over issue numbers. The "}
              <InlineCode>virtual</InlineCode>
              {" prefix is always automatically prepended."}
            </p>
          </HelpBlock>

          <HelpBlock title="Per repository">
            <p className="text-muted-foreground">You can also set this value per repository using:</p>
            <CommandLine>git config branchdeck.branchPrefix repoSpecificPrefix</CommandLine>
          </HelpBlock>
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Close</KeyHint>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}

function HelpBlock({ title, children }: { title: string, children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="font-medium">{title}</h3>
      {children}
    </section>
  )
}

/** A command on its own line, scrolled rather than wrapped */
function CommandLine({ children }: { children: string }) {
  return <pre className="overflow-x-auto rounded-lg bg-muted px-3 py-2 font-mono text-xs">{children}</pre>
}

function PatternExample({ label, message, branch }: { label: string, message: string, branch: string }) {
  return (
    <li className="flex flex-col gap-1">
      <span className="font-medium">{label}</span>
      <span className="text-muted-foreground">
        <InlineCode>{message}</InlineCode>
        <span className="px-2">→</span>
        <InlineCode>{branch}</InlineCode>
      </span>
    </li>
  )
}
