import { useState } from "react"
import type { ReactNode } from "react"
import { CircleHelpIcon, GitBranchIcon, TerminalIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"

interface WelcomeCardProps {
  hasRepository: boolean
  hasBranchPrefix: boolean
}

export function WelcomeCard({ hasRepository, hasBranchPrefix }: WelcomeCardProps) {
  const [helpOpen, setHelpOpen] = useState(false)
  return (
    <div>
      <Card data-testid="welcome-card">
        <CardHeader className="border-b">
          <div className="flex items-center gap-2">
            <GitBranchIcon className="size-5 text-primary" />
            <h3 className="text-lg font-semibold">
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
              <Step number={2} title="Configure your branch prefix">
                <p className="text-sm text-muted-foreground">
                  Set a default prefix for your branches to keep them organized. For example:
                  {" "}
                  <code className="rounded bg-muted px-1 py-0.5 text-xs">username/</code>
                </p>
                <div className="flex items-center gap-2 text-sm">
                  <TerminalIcon className="size-3.5 text-muted-foreground" />
                  <code className="rounded bg-muted px-2 py-1 text-xs">git config --global branchdeck.branchPrefix "your-prefix"</code>
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
    <div className="flex gap-3">
      <div className="mt-0.5 shrink-0">
        <div className="flex size-6 items-center justify-center rounded-full bg-primary/10 text-xs font-medium text-primary">
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
