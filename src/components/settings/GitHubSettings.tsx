import { openUrl } from "@tauri-apps/plugin-opener"
import { ExternalLinkIcon, LogInIcon, LogOutIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ReadFailure } from "@/components/settings/SettingRow"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { useNow } from "@/hooks/useNow"
import type { GitHubAuthStatus } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatTokenSource, ORGANIZATION_APPROVAL } from "@/lib/pull-request"
import { cancelSignIn, copyCodeAndOpenGitHub, runSignIn, signOut } from "@/lib/sign-in-run"
import { retryAuthStatus, useAuthReadError, useAuthStatus, useSignInPhase } from "@/stores/auth"

/**
 * Settings → GitHub: where the token comes from, and the app's own sign-in (spec/repository/github-sign-in.spec.md).
 * Signing in shows the code to enter on GitHub until GitHub answers. Until the first answer the page says it reads the
 * sign-in; a failed read says why, with Retry, above the last status.
 */
export function GitHubSettings() {
  const status = useAuthStatus()?.github ?? null
  const readError = useAuthReadError()
  const signIn = useSignInPhase("gitHub")
  const failure = readError === null ? null : <ReadFailure title="Cannot read the GitHub sign-in" message={readError} testId="github-auth-error" onRetry={retryAuthStatus} />
  if (status === null) {
    return failure ?? <LoadingLine>Reading the GitHub sign-in…</LoadingLine>
  }
  if (signIn.phase === "code") {
    return <SignInCode userCode={signIn.code} expiresAt={signIn.expiresAt} />
  }
  const approvalNote = describeApproval(status)
  return (
    <>
      {failure}
      <div className="flex flex-col gap-2 text-sm" data-testid="github-auth" data-status={status.signedInAs === null ? "signedOut" : "signedIn"}>
        <p>{describeToken(status)}</p>
        {approvalNote !== null && (
          <p className="text-xs text-muted-foreground" data-testid="github-approval-note">
            {approvalNote}
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          {status.signedInAs === null ? (
            status.signInAvailable && (
              <Button size="xs" variant="outline" disabled={signIn.phase !== "idle"} onClick={() => void runSignIn({ provider: "gitHub" })}>
                {signIn.phase === "idle" ? <LogInIcon data-icon="inline-start" /> : <Spinner data-icon="inline-start" />}
                Sign In to GitHub
              </Button>
            )
          ) : (
            <Button size="xs" variant="outline" onClick={() => void signOut("gitHub")}>
              <LogOutIcon data-icon="inline-start" />
              Sign Out
            </Button>
          )}
          {status.manageUrl !== null && status.signedInAs !== null && (
            <Button size="xs" variant="ghost" onClick={() => void openUrl(status.manageUrl!).catch((error: unknown) => notifyError("Cannot open GitHub", error))}>
              <ExternalLinkIcon data-icon="inline-start" />
              Manage on GitHub
            </Button>
          )}
        </div>
      </div>
    </>
  )
}

/** "Signed in as @octocat", else which token the app takes, else what gives it one */
function describeToken(status: GitHubAuthStatus): string {
  const source = status.tokenSource
  if (status.signedInAs !== null) {
    return source === null || source === "branchDeck"
      ? `Signed in as @${status.signedInAs}`
      : `Signed in as @${status.signedInAs}; the token of ${formatTokenSource(source)} comes first while GitHub accepts it`
  }
  if (source !== null) {
    return `Uses the token of ${formatTokenSource(source)}`
  }
  return status.signInAvailable
    ? "No GitHub token: run gh auth login, set GH_TOKEN, store HTTPS credentials of github.com in git, or sign in here"
    : "No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git"
}

/**
 * What the app's own sign-in can't do by itself: an organization that restricts OAuth apps has to approve Branch Deck,
 * and has often approved gh already. Before the sign-in, and while its token is the one the app takes.
 */
function describeApproval(status: GitHubAuthStatus): string | null {
  if (!status.signInAvailable || (status.tokenSource !== null && status.tokenSource !== "branchDeck")) {
    return null
  }
  return status.signedInAs === null ? `${ORGANIZATION_APPROVAL}; gh is often approved there already` : `${ORGANIZATION_APPROVAL}: request it in Manage on GitHub`
}

function SignInCode({ userCode, expiresAt }: { userCode: string; expiresAt: number }) {
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border p-3 text-sm" data-testid="github-sign-in-code">
      <p className="text-muted-foreground">Enter this code on GitHub to sign in Branch Deck</p>
      <span className="font-mono text-lg tracking-widest select-all" data-testid="github-user-code">
        {userCode}
      </span>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="xs" onClick={() => void copyCodeAndOpenGitHub()}>
          <ExternalLinkIcon data-icon="inline-start" />
          Copy Code and Open GitHub
        </Button>
        <Button size="xs" variant="ghost" onClick={() => void cancelSignIn("gitHub")}>
          Cancel
        </Button>
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Spinner className="size-3" />
          <Countdown expiresAt={expiresAt} />
        </span>
      </div>
    </div>
  )
}

/** "Waiting for GitHub · expires in 14:32", renewed every second */
function Countdown({ expiresAt }: { expiresAt: number }) {
  const now = useNow(1000)
  const seconds = Math.max(0, Math.round((expiresAt - now) / 1000))
  return `Waiting for GitHub · expires in ${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
}
