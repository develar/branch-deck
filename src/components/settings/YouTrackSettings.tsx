import { useId, useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { ExternalLinkIcon, KeyRoundIcon, LogInIcon, LogOutIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import type { YouTrackAuthStatus } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { formatYouTrackHost, suggestYouTrackUrls, youTrackTokensPage } from "@/lib/youtrack"
import { useRecentProjects } from "@/stores/settings/repositorySettings"
import {
  cancelYouTrackSignIn,
  openYouTrackSignInPage,
  saveYouTrackToken,
  signOutOfYouTrack,
  startYouTrackSignIn,
  useYouTrackAuthStatus,
  useYouTrackSignIn,
} from "@/stores/youtrack"
import type { YouTrackSignInPhase } from "@/stores/youtrack"

/**
 * Settings → YouTrack (spec/repository/youtrack.spec.md): who is signed in to which YouTrack, else the URL to sign in
 * to, offered from the issue links of `.idea/vcs.xml`. Sign In opens the Hub of the YouTrack in the browser; a token
 * created in YouTrack can be pasted instead.
 */
export function YouTrackSettings() {
  const status = useYouTrackAuthStatus()
  const signIn = useYouTrackSignIn()
  if (status === null) {
    return null
  }
  if (status.url !== null && status.problem === null && signIn.phase === "idle") {
    return <SignedIn status={status} url={status.url} />
  }
  // stays mounted while the browser signs in: a cancel keeps the URL typed
  return <SignInForm status={status} phase={signIn.phase} />
}

function SignedIn({ status, url }: { status: YouTrackAuthStatus; url: string }) {
  const host = formatYouTrackHost(url)
  const user = status.signedInAs
  return (
    <div className="flex flex-col gap-2 text-sm" data-testid="youtrack-auth" data-status="signedIn">
      <p>
        {status.source === "environment"
          ? `Uses the token of YOUTRACK_TOKEN for ${host}`
          : `Signed in to ${host} as ${user?.fullName ?? "?"}${user !== null && user.login !== user.fullName ? ` (${user.login})` : ""}`}
      </p>
      {status.source === "signIn" && (
        <p className="text-xs text-muted-foreground">Renewed by itself while the Hub keeps the sign-in; Branch Deck asks you to sign in again when it ends</p>
      )}
      <div className="flex flex-wrap gap-2">
        {status.source !== "environment" && (
          <Button size="xs" variant="outline" onClick={() => void signOutOfYouTrack()}>
            <LogOutIcon data-icon="inline-start" />
            Sign Out
          </Button>
        )}
        <Button size="xs" variant="ghost" onClick={() => openPage(youTrackTokensPage(url))}>
          <ExternalLinkIcon data-icon="inline-start" />
          Manage Tokens
        </Button>
      </div>
    </div>
  )
}

function SignInForm({ status, phase }: { status: YouTrackAuthStatus; phase: YouTrackSignInPhase["phase"] }) {
  const starting = phase === "starting"
  // the list itself: a selector that builds a new array would render again and again
  const projects = useRecentProjects(items => items)
  const candidates = suggestYouTrackUrls(projects)
  const [url, setUrl] = useState(() => status.url ?? candidates[0]?.url ?? "")
  const [pasting, setPasting] = useState(false)
  const [token, setToken] = useState("")
  const [saving, setSaving] = useState(false)
  const urlId = useId()
  const tokenId = useId()
  const source = candidates.find(candidate => candidate.url === url)
  const save = () => {
    setSaving(true)
    void saveYouTrackToken(url, token).then(saved => {
      setSaving(false)
      if (saved) {
        setToken("")
        setPasting(false)
      }
    })
  }
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="youtrack-auth" data-status={status.problem ?? "signedOut"}>
      {status.problem !== null && (
        <p className="text-warning" data-testid="youtrack-auth-problem">
          {status.problem === "expired" ? "Your YouTrack sign-in expired: sign in again" : "YouTrack rejected the token: sign in again"}
        </p>
      )}
      <div className="flex flex-col gap-1">
        <label htmlFor={urlId} className="text-xs text-muted-foreground">
          YouTrack URL
        </label>
        <Input
          id={urlId}
          value={url}
          placeholder="https://youtrack.example.com"
          spellCheck={false}
          autoComplete="off"
          data-settings-focus="youtrack-url"
          disabled={phase === "waiting"}
          onChange={event => setUrl(event.target.value)}
        />
        {source !== undefined && <p className="text-xs text-muted-foreground">From .idea/vcs.xml of {source.repository}</p>}
        {candidates.length > 1 && (
          <div className="flex flex-wrap gap-1" data-testid="youtrack-url-candidates">
            {candidates.map(candidate => (
              <Button key={candidate.url} size="xs" variant="ghost" aria-pressed={candidate.url === url} onClick={() => setUrl(candidate.url)}>
                {formatYouTrackHost(candidate.url)}
              </Button>
            ))}
          </div>
        )}
      </div>
      {phase === "waiting" ? (
        <WaitingForBrowser />
      ) : (
        <div className="flex flex-wrap gap-2">
          <Button size="xs" variant="outline" disabled={starting || url.trim() === ""} onClick={() => void startYouTrackSignIn(url)}>
            {starting ? <Spinner data-icon="inline-start" /> : <LogInIcon data-icon="inline-start" />}
            Sign In to YouTrack
          </Button>
          {!pasting && (
            <Button size="xs" variant="ghost" onClick={() => setPasting(true)}>
              <KeyRoundIcon data-icon="inline-start" />
              Use a Token Instead
            </Button>
          )}
        </div>
      )}
      {pasting && phase !== "waiting" && (
        <div className="flex flex-col gap-2 rounded-lg border border-border p-3" data-testid="youtrack-token-form">
          <label htmlFor={tokenId} className="text-xs text-muted-foreground">
            A token created in Account Security of your YouTrack profile
          </label>
          <Input id={tokenId} type="password" value={token} autoComplete="off" spellCheck={false} placeholder="perm-…" onChange={event => setToken(event.target.value)} />
          <div className="flex flex-wrap gap-2">
            <Button size="xs" disabled={saving || token.trim() === "" || url.trim() === ""} onClick={save}>
              {saving && <Spinner data-icon="inline-start" />}
              Save Token
            </Button>
            <Button size="xs" variant="ghost" disabled={url.trim() === ""} onClick={() => openPage(youTrackTokensPage(url.trim()))}>
              <ExternalLinkIcon data-icon="inline-start" />
              Create Token…
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setPasting(false)}>
              Cancel
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}

function WaitingForBrowser() {
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-border p-3 text-sm" data-testid="youtrack-sign-in-waiting">
      <p className="text-muted-foreground">Sign in to YouTrack in the browser tab that opened, then return here</p>
      <div className="flex flex-wrap items-center gap-2">
        <Button size="xs" variant="outline" onClick={() => void openYouTrackSignInPage()}>
          <ExternalLinkIcon data-icon="inline-start" />
          Open the Page Again
        </Button>
        <Button size="xs" variant="ghost" onClick={() => void cancelYouTrackSignIn()}>
          Cancel
        </Button>
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Spinner className="size-3" />
          Waiting for the browser…
        </span>
      </div>
    </div>
  )
}

function openPage(url: string): void {
  void openUrl(url).catch((error: unknown) => notifyError("Cannot open YouTrack", error))
}
