import { CircleXIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { CommandLine } from "@/components/shared/CommandLine"
import { getSelectedProject, triggerBranchPrefixLoad, useRepository } from "@/stores/repository"

/**
 * What blocks the first run before any repository work: git is missing or too old (destructive), or git cannot name
 * the user (warning: sync works until Branch Deck writes notes or rewrites a commit). `null` when git is fine.
 */
export function GitSetupAlert() {
  const gitError = useRepository(state => state.gitSetup?.git_error)
  const identityError = useRepository(state => state.gitSetup?.identity_error)
  if (gitError) {
    const [summary, ...hintLines] = gitError.split("\n")
    return (
      <Alert variant="destructive" data-testid="git-setup-alert">
        <CircleXIcon />
        <AlertTitle>{summary}</AlertTitle>
        <AlertDescription className="flex flex-col gap-2">
          {hintLines.map(line => <Hint key={line} line={line} />)}
          <CheckAgainButton />
        </AlertDescription>
      </Alert>
    )
  }
  if (identityError) {
    return (
      <Alert variant="warning" data-testid="git-setup-alert">
        <TriangleAlertIcon />
        <AlertTitle>Git does not know your name and email</AlertTitle>
        <AlertDescription className="flex flex-col gap-2">
          <div>Branch Deck records its notes and rewritten commits with them. Set them once for all repositories:</div>
          <CommandLine className="max-w-lg">git config --global user.name "Your Name"</CommandLine>
          <CommandLine className="max-w-lg">git config --global user.email you@example.com</CommandLine>
          <CheckAgainButton />
        </AlertDescription>
      </Alert>
    )
  }
  return null
}

/** "Install or upgrade Git with: brew install git": the command after the last ": " on a line of its own, to copy */
function Hint({ line }: { line: string }) {
  const separator = line.lastIndexOf(": ")
  const command = separator === -1 ? "" : line.slice(separator + 2)
  if (command === "" || command.startsWith("http")) {
    return <div>{line}</div>
  }
  return (
    <>
      <div>{`${line.slice(0, separator)}:`}</div>
      <CommandLine className="max-w-lg">{command}</CommandLine>
    </>
  )
}

function CheckAgainButton() {
  return (
    <Button variant="outline" size="sm" className="self-start" onClick={() => triggerBranchPrefixLoad(getSelectedProject())}>
      <RefreshCwIcon data-icon="inline-start" />
      Check Again
    </Button>
  )
}
