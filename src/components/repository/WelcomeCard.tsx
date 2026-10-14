import { useState } from "react"
import type { ReactNode } from "react"
import { CircleHelpIcon, GitBranchIcon, TerminalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { InlineCode } from "@/components/shared/InlineCode"

interface WelcomeCardProps {
  hasRepository: boolean
  hasBranchPrefix: boolean
}

export function WelcomeCard({ hasRepository, hasBranchPrefix }: WelcomeCardProps) {
  const [helpOpen, setHelpOpen] = useState(false)
  // the steps shown are numbered from 1: with a repository selected, the prefix is the first step
  const prefixStep = hasRepository ? 1 : 2
  return (
    <div>
      <Card data-testid="welcome-card">
        <CardHeader className="border-b">
          <div className="flex items-center gap-2">
            <GitBranchIcon className="size-5 text-primary" />
            {/* the one island title above text-sm (docs/design/ui-theme.md, "Type sizes") */}
            <h3 className="text-base font-semibold">
              Welcome to Branch Deck
            </h3>
          </div>
        </CardHeader>

        <CardContent className="flex flex-col gap-4">
          <p className="text-muted-foreground">
            {hasRepository
              ? "You're almost ready: configure your branch prefix to start managing your branches."
              : "Let's get you started with managing your Git branches. Follow these simple steps:"}
          </p>

          <div className="flex flex-col gap-3">
            {!hasRepository && (
              <Step number={1} title="Select a Git repository">
                <p className="text-sm text-muted-foreground">
                  Use the repository selector above or click the folder icon to browse for your project.
                </p>
              </Step>
            )}

            {!hasBranchPrefix && (
              <Step number={prefixStep} title="Configure your branch prefix">
                <p className="text-sm text-muted-foreground">
                  Set a default prefix for your branches to keep them organized. For example:
                  {" "}
                  <InlineCode>username/</InlineCode>
                </p>
                <div className="flex items-center gap-2 text-sm">
                  <TerminalIcon className="size-3.5 text-muted-foreground" />
                  <InlineCode className="py-0.5">git config --global branchdeck.branchPrefix "your-prefix"</InlineCode>
                  <Button variant="link" size="xs" onClick={() => setHelpOpen(true)}>
                    <CircleHelpIcon data-icon="inline-start" />
                    View guide
                  </Button>
                </div>
              </Step>
            )}
          </div>

          <div className="pt-2">
            <p className="text-sm text-muted-foreground">
              {hasBranchPrefix
                ? "Once you select a repository, you can sync your branches and start organizing your commits."
                : "Once configured, you can sync your branches and start organizing your commits."}
            </p>
          </div>
        </CardContent>
      </Card>
      <BranchPrefixHelpDialog open={helpOpen} onOpenChange={setHelpOpen} />
    </div>
  )
}

function Step({ number, title, children }: { number: number, title: string, children: ReactNode }) {
  return (
    <div className="flex gap-3" data-testid="welcome-step">
      <div className="mt-0.5 shrink-0">
        <div className="flex size-6 items-center justify-center rounded-full bg-primary/10 text-xs font-medium text-primary" data-testid="welcome-step-number">
          {number}
        </div>
      </div>
      <div className="flex flex-1 flex-col gap-1">
        <h4 className="font-medium">{title}</h4>
        <div className="flex flex-col gap-2">
          {children}
        </div>
      </div>
    </div>
  )
}
