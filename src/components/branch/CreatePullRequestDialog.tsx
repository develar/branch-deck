import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { ExternalLinkIcon, PlusIcon, SearchIcon, ShieldOffIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { PRIMARY_KBD_CLASS_NAME } from "@/components/shared/MenuItemButton"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { ReviewerPicker } from "@/components/shared/ReviewerPicker"
import { useAvailabilityAction } from "@/hooks/useAvailabilityAction"
import { getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import { useCreatePullRequestContext, useStackPullRequestContext } from "@/hooks/useCreatePullRequestContext"
import type { ReviewRequirementState } from "@/hooks/usePatronusReviewRequirement"
import { useReviewerSearch } from "@/hooks/useReviewerSearch"
import type { GitHubUser, NewPullRequest } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { isLogin, preselectedReviewers, suggestedReviewers } from "@/lib/create-pull-request"
import type { SuggestedReviewer } from "@/lib/create-pull-request"
import { continueOnGitHub, submitPullRequest } from "@/lib/create-pull-request-context"
import type { CreatePullRequestContext, CreatePullRequestInput } from "@/lib/create-pull-request-context"
import { notifyError } from "@/lib/errors"
import { isMac } from "@/lib/platform"
import { formatAvailabilityHint, openPullRequest } from "@/lib/pull-request"
import type { CompareFields } from "@/lib/pull-request"
import { formatStackPushing } from "@/lib/stack"
import { showToast } from "@/lib/toast-message"
import { transferLabel } from "@/lib/transfer-progress"
import type { Branch } from "@/stores/branch-sync"
import { useBranchPushProgress, useBranchSync, useIsBranchPushing, useStackPush } from "@/stores/branch-sync"
import { closeCreatePullRequestDialog, useCreatePullRequestBranch, useCreatePullRequestGeneration, useCreatePullRequestWholeStack } from "@/stores/create-pull-request-dialog"
import { setCreatePullRequestAssignSelf, useAppSettings } from "@/stores/settings/appSettings"

/**
 * The "Create pull request" dialog of the branch the store names: the title and the description (the `bd describe`
 * text, else the commits it will show), the reviewers (the code owners Patronus suggests, one per group chosen at once,
 * and a search of the repository's users), "Assign to me"; ⌘↵ creates it through the GitHub API, "Continue on GitHub"
 * opens GitHub's form with the text instead (it takes no reviewers). Without a token GitHub accepts, it says why and
 * GitHub's form is the way.
 *
 * Loaded on the first open, then kept mounted (see `PreviewDialogFrame`); each opening starts with a new text.
 */
export function CreatePullRequestDialog() {
  const branchName = useCreatePullRequestBranch()
  const wholeStack = useCreatePullRequestWholeStack()
  const generation = useCreatePullRequestGeneration()
  // the last branch stays while the closing animation runs
  const [shown, setShown] = useState<{ branchName: string; wholeStack: boolean; generation: number } | null>(null)
  if (branchName !== null && shown?.generation !== generation) {
    setShown({ branchName, wholeStack, generation })
  }
  const [isCreating, setIsCreating] = useState(false)
  const titleRef = useRef<HTMLInputElement>(null)
  const branch = useBranchSync(state => (shown === null ? undefined : state.branches[shown.branchName]))
  const formProps = { titleRef, isCreating, setIsCreating }

  return (
    <CreatePullRequestDialogFrame open={branchName !== null} isCreating={isCreating} titleRef={titleRef} onClose={closeCreatePullRequestDialog}>
      {shown?.wholeStack === true && <MainStackPullRequestForm key={shown.generation} root={shown.branchName} {...formProps} />}
      {shown?.wholeStack === false && branch !== undefined && <MainCreatePullRequestForm key={shown.generation} branch={branch} {...formProps} />}
    </CreatePullRequestDialogFrame>
  )
}

/**
 * The dialog of a pull request around its form: a click outside keeps the text, and nothing closes it while GitHub
 * creates the pull request. Kept mounted while closed (see `PreviewDialogFrame`); the main window's dialog and the one
 * of the review window
 */
export function CreatePullRequestDialogFrame({
  open,
  isCreating,
  titleRef,
  onClose,
  children,
}: {
  open: boolean
  isCreating: boolean
  titleRef: RefObject<HTMLInputElement | null>
  onClose: () => void
  children: ReactNode
}) {
  return (
    <PreviewDialogFrame
      data-testid="create-pull-request-dialog"
      width="form"
      open={open}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        if (eventDetails.reason === "outside-press" || isCreating) {
          eventDetails.cancel()
          return
        }
        onClose()
      }}
      initialFocus={titleRef}
    >
      {children}
    </PreviewDialogFrame>
  )
}

/** What the form does, where it runs: the main window creates the pull request, the review window asks it to */
export interface CreatePullRequestActions {
  /** Pushes what the pull request needs, creates it and tells how it went: true when it was (the dialog closes then) */
  create: (input: CreatePullRequestInput) => Promise<boolean>
  /** Pushes what the pull request needs, then opens GitHub's form with the fields: true when it opened */
  continueOnGitHub: (fields: CompareFields) => Promise<boolean>
  /** "Safe Push Without Review…": the popover of the row's Safe Push, in the main window */
  safePushWithoutReview: () => void
  close: () => void
}

interface FormProps {
  titleRef: RefObject<HTMLInputElement | null>
  isCreating: boolean
  setIsCreating: (isCreating: boolean) => void
}

/** "Pushing 45%" while the push a creation starts with runs ("Pushing 2 branches 45%" with its parents), else null */
function usePushLabel(branchName: string): string | null {
  const pushing = useIsBranchPushing(branchName)
  const stackPush = useStackPush(branchName)
  const progress = useBranchPushProgress(stackPush?.[0] ?? branchName)
  if (!pushing && stackPush === null) {
    return null
  }
  return transferLabel(formatStackPushing(branchName, stackPush), progress)
}

/** The form of the main window's dialog: the context from its stores, the creation by its store, the toast here */
function MainCreatePullRequestForm({ branch, ...props }: FormProps & { branch: Branch }) {
  const context = useCreatePullRequestContext(branch)
  return <MainForm context={context} safePushWithoutReview={() => getSafePushWithoutReviewItem(branch)?.onSelect()} pushLabel={usePushLabel(branch.name)} {...props} />
}

/** The form of the main window's dialog of the whole stack: its stack branch, no Safe Push without review */
function MainStackPullRequestForm({ root, ...props }: FormProps & { root: string }) {
  return <MainForm context={useStackPullRequestContext(root)} safePushWithoutReview={() => {}} pushLabel={null} {...props} />
}

function MainForm({
  context,
  safePushWithoutReview,
  pushLabel,
  ...props
}: FormProps & { context: CreatePullRequestContext; safePushWithoutReview: () => void; pushLabel: string | null }) {
  const actions: CreatePullRequestActions = {
    create: async input => {
      const result = await submitPullRequest(context, input).catch((error: unknown) => {
        notifyError("Cannot create pull request", error)
        return null
      })
      if (result === null) {
        return false
      }
      showToast({ ...result.toast, action: { children: "Open Pull Request", onClick: () => openPullRequest(result.pullRequest) } })
      return true
    },
    continueOnGitHub: fields =>
      continueOnGitHub(context, fields).then(
        () => true,
        (error: unknown) => {
          notifyError("Cannot open GitHub", error)
          return false
        },
      ),
    safePushWithoutReview,
    close: closeCreatePullRequestDialog,
  }
  return <CreatePullRequestForm context={context} actions={actions} pushLabel={pushLabel} {...props} />
}

type TextState = { status: "loading" } | { status: "loaded"; text: NewPullRequest } | { status: "error"; message: string }

/**
 * The title and the description, the reviewers, "Assign to me", "Safe Push Without Review…", "Continue on GitHub" and
 * Create, from the context of the main window (`CreatePullRequestContext`); it reads the text and searches the users
 * itself, in any window
 */
export function CreatePullRequestForm({
  context,
  actions,
  titleRef,
  isCreating,
  setIsCreating,
  pushLabel = null,
}: FormProps & {
  context: CreatePullRequestContext
  actions: CreatePullRequestActions
  /** The progress of the push the creation starts with, where the window knows it (the main window) */
  pushLabel?: string | null
}) {
  const { repositoryPath, branchName, label, head, base, includesText, wholeStack } = context

  const [text, setText] = useState<TextState>({ status: "loading" })
  const [title, setTitle] = useState("")
  const [body, setBody] = useState("")
  // the text of the branch fills only what the user hasn't typed yet
  const [edited, setEdited] = useState({ title: false, body: false })
  // the whole stack's text names its branches: the members as their rows show them
  const stackText = wholeStack === null ? null : JSON.stringify(wholeStack)
  useEffect(() => {
    let current = true
    const stack = stackText === null ? null : (JSON.parse(stackText) as NonNullable<CreatePullRequestContext["wholeStack"]>)
    const read =
      stack === null
        ? commands.getNewPullRequest({ repositoryPath, head, base })
        : commands.getNewStackPullRequest({ repositoryPath, branchPrefix: stack.branchPrefix, root: branchName, members: [...stack.members], target: base })
    read.then(
      result => {
        if (!current) {
          return
        }
        setText(result.status === "ok" ? { status: "loaded", text: result.data } : { status: "error", message: result.error.message })
      },
      (error: unknown) => {
        if (current) {
          setText({ status: "error", message: error instanceof Error ? error.message : String(error) })
        }
      },
    )
    return () => {
      current = false
    }
  }, [repositoryPath, branchName, head, base, stackText])
  const [filled, setFilled] = useState(false)
  if (!filled && text.status === "loaded") {
    setFilled(true)
    if (!edited.title) {
      setTitle(text.text.title)
    }
    if (!edited.body) {
      setBody(text.text.body)
    }
  }
  const viewerLogin = text.status === "loaded" ? text.text.viewerLogin : null

  const availability = context.availability
  // never asked yet is no reason to refuse: GitHub answers the creation
  const unavailable = availability !== null && availability.status !== "available"
  const availabilityHint = unavailable ? formatAvailabilityHint(availability) : null
  const availabilityAction = useAvailabilityAction(unavailable ? availability : null)

  const patronusAvailable = context.patronusAvailable
  // the code owners of the changes, whatever a pull request would count
  const review = context.review
  const suggestions = review.status === "loaded" ? suggestedReviewers(review.requirement, viewerLogin) : []

  const [reviewers, setReviewers] = useState<GitHubUser[]>([])
  const [preselected, setPreselected] = useState(false)
  // once the code owners and the user are known: the user is never asked to review the own pull request
  if (!preselected && review.status !== "loading" && text.status !== "loading") {
    setPreselected(true)
    const chosen = preselectedReviewers(suggestions).filter(user => !reviewers.some(it => isLogin(it.login, user.login)))
    if (chosen.length > 0) {
      setReviewers([...reviewers, ...chosen])
    }
  }

  const assignSelf = useAppSettings(state => state.createPullRequestAssignSelf)
  // the approvals count only in a pull request: a Safe Push with a profile that skips the review is the other way
  const withoutReview = context.withoutReview

  // a pull request that appeared meanwhile (created elsewhere, or by the refresh after the push)
  const liveExisting = context.existing

  // GitHub's form and the creation alike; the title only for the creation
  const newPullRequestBlocker = context.newPullRequestBlocker
  const submitBlocker = newPullRequestBlocker ?? formatTitleBlocker(title, text.status === "loading")
  const canCreate = !isCreating && !unavailable && submitBlocker === null

  const create = async () => {
    if (!canCreate) {
      return
    }
    setIsCreating(true)
    const input = { title: title.trim(), body, reviewers: reviewers.map(user => user.login), assignSelf, viewerLogin }
    // `.finally`, not try/finally: React Compiler cannot compile a try without a catch
    const created = await actions.create(input).finally(() => setIsCreating(false))
    if (created) {
      actions.close()
    }
  }

  const continueOnGitHub = async () => {
    if (isCreating || newPullRequestBlocker !== null) {
      return
    }
    setIsCreating(true)
    const fields = { title: title.trim(), body, assignee: assignSelf ? (viewerLogin ?? undefined) : undefined }
    const opened = await actions.continueOnGitHub(fields).finally(() => setIsCreating(false))
    if (opened) {
      actions.close()
    }
  }

  const search = useReviewerSearch(repositoryPath)
  const choose = (suggestion: SuggestedReviewer) => {
    if (suggestion.login === null) {
      // Patronus doesn't say the login: the user picks the person from what GitHub knows
      search.lookUp(suggestion.fullName)
      return
    }
    if (!reviewers.some(user => isLogin(user.login, suggestion.login))) {
      setReviewers([...reviewers, { login: suggestion.login, name: suggestion.fullName }])
    }
  }
  const unchosen = suggestions.filter(suggestion => suggestion.login === null || !reviewers.some(user => isLogin(user.login, suggestion.login)))

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      void create()
    }
  }

  return (
    <>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Create pull request</DialogTitle>
        <p data-testid="create-pull-request-base" className="truncate text-xs text-muted-foreground">
          {`${label} → ${base}`}
        </p>
        {includesText !== null && (
          <p data-testid="create-pull-request-includes" className="text-xs text-muted-foreground">
            {includesText}
          </p>
        )}
      </PreviewDialogHeader>

      <PreviewDialogBody>
        {/* ⌘↵ creates from any field, the reviewer search too (its Enter picks a user) */}
        <div className="flex flex-col gap-4" onKeyDown={handleKeyDown}>
          {availabilityHint !== null && (
            <Alert variant="warning" data-testid="create-pull-request-unavailable">
              <TriangleAlertIcon />
              <AlertDescription className="flex flex-wrap items-center gap-x-2">
                {`${availabilityHint}: GitHub's form opens the pull request instead.`}
                {availabilityAction !== null && (
                  <Button variant="link" size="xs" className="h-auto p-0" onClick={availabilityAction.onClick}>
                    {availabilityAction.label}
                  </Button>
                )}
              </AlertDescription>
            </Alert>
          )}
          {liveExisting !== null && (
            <Alert variant="info" data-testid="create-pull-request-exists">
              <AlertDescription className="flex flex-wrap items-center gap-x-2">
                {`${label} already has pull request #${liveExisting.number}.`}
                <Button variant="link" size="xs" className="h-auto p-0" onClick={() => openPullRequest(liveExisting)}>
                  Open Pull Request
                </Button>
              </AlertDescription>
            </Alert>
          )}
          {text.status === "error" && <p className="text-xs text-destructive">{`Cannot read the commits of ${label}: ${text.message}`}</p>}

          <label className="flex flex-col gap-1.5">
            <span className="text-xs text-muted-foreground">Title</span>
            <Input
              ref={titleRef}
              data-testid="create-pull-request-title"
              value={title}
              placeholder={text.status === "loading" ? "Reading the commits…" : undefined}
              disabled={isCreating}
              onChange={event => {
                setEdited({ ...edited, title: true })
                setTitle(event.target.value)
              }}
            />
          </label>
          <label className="flex flex-col gap-1.5">
            <span className="text-xs text-muted-foreground">Description</span>
            <Textarea
              data-testid="create-pull-request-body"
              className="min-h-28"
              value={body}
              disabled={isCreating}
              spellCheck
              onChange={event => {
                setEdited({ ...edited, body: true })
                setBody(event.target.value)
              }}
            />
          </label>

          {/* GitHub's form takes no reviewers, and without a token nobody can be searched */}
          {!unavailable && (
            <div className="flex flex-col gap-1.5">
              <span className="text-xs text-muted-foreground">Reviewers</span>
              {patronusAvailable && <CodeOwnerSuggestions state={review} suggestions={unchosen} onChoose={choose} />}
              <ReviewerPicker
                testIdPrefix="create-pull-request"
                reviewers={reviewers}
                setReviewers={setReviewers}
                search={search}
                viewerLogin={viewerLogin}
                disabled={isCreating}
              />
            </div>
          )}

          {/* GitHub's form assigns a login: without a token the app knows none */}
          {(!unavailable || viewerLogin !== null) && (
            <label className="flex items-center gap-2">
              <Switch data-testid="create-pull-request-assign" checked={assignSelf} disabled={isCreating} onCheckedChange={setCreatePullRequestAssignSelf} />
              <span>Assign to me</span>
              {viewerLogin !== null && <span className="text-xs text-muted-foreground">{viewerLogin}</span>}
            </label>
          )}
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter
        start={
          withoutReview !== null && (
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    data-testid="create-pull-request-without-review"
                    variant="ghost"
                    size="xs"
                    className={cn("text-muted-foreground", withoutReview.disabled && "opacity-50")}
                    disabled={isCreating || withoutReview.disabled}
                    focusableWhenDisabled
                    onClick={actions.safePushWithoutReview}
                  />
                }
              >
                <ShieldOffIcon data-icon="inline-start" />
                {withoutReview.label}
              </TooltipTrigger>
              <TooltipContent>{withoutReview.description}</TooltipContent>
            </Tooltip>
          )
        }
      >
        <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
        <Tooltip>
          <TooltipTrigger
            render={
              <Button
                data-testid="create-pull-request-on-github"
                variant={unavailable ? "default" : "ghost"}
                size="xs"
                disabled={isCreating || newPullRequestBlocker !== null}
                // the hover says why; Base UI keeps the button focusable, so the dimming is ours
                focusableWhenDisabled
                className={cn(newPullRequestBlocker !== null && "opacity-50")}
                onClick={() => void continueOnGitHub()}
              />
            }
          >
            <ExternalLinkIcon data-icon="inline-start" />
            Continue on GitHub
          </TooltipTrigger>
          <TooltipContent>{newPullRequestBlocker ?? "GitHub's form takes the title, the description and the assignee, not the reviewers"}</TooltipContent>
        </Tooltip>
        {!unavailable && (
          <Tooltip disabled={canCreate || isCreating}>
            <TooltipTrigger
              render={
                <Button
                  data-testid="create-pull-request-submit"
                  size="xs"
                  disabled={!canCreate}
                  focusableWhenDisabled
                  className={cn(!canCreate && !isCreating && "opacity-50")}
                  onClick={() => void create()}
                />
              }
            >
              {isCreating && <Spinner data-icon="inline-start" />}
              {pushLabel ?? "Create Pull Request"}
              <Kbd className={PRIMARY_KBD_CLASS_NAME}>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
            </TooltipTrigger>
            <TooltipContent>{submitBlocker}</TooltipContent>
          </Tooltip>
        )}
      </PreviewDialogFooter>
    </>
  )
}

/** Why the title can't create the pull request: none yet (the commits are read), or more than one line */
function formatTitleBlocker(title: string, loading: boolean): string | null {
  if (title.trim() === "") {
    return loading ? "Reading the commits of the branch…" : "Enter a title"
  }
  return title.includes("\n") ? "The title must be one line" : null
}

/** "Suggested by code owners": a chip per suggested reviewer of each group whose approval is missing */
function CodeOwnerSuggestions({
  state,
  suggestions,
  onChoose,
}: {
  state: ReviewRequirementState
  suggestions: readonly SuggestedReviewer[]
  onChoose: (suggestion: SuggestedReviewer) => void
}) {
  if (state.status === "loading") {
    return (
      <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Spinner className="size-3" />
        Asking Patronus for the code owners…
      </span>
    )
  }
  if (state.status === "error") {
    return <span className="text-xs text-muted-foreground">{`Cannot read the code owners: ${state.message}`}</span>
  }
  if (state.requirement.skipped) {
    return <span className="text-xs text-muted-foreground">The code owners of so many commits are named when the Safe Push starts</span>
  }
  if (suggestions.length === 0) {
    return null
  }
  return (
    <div data-testid="create-pull-request-suggestions" className="flex flex-wrap items-center gap-1.5">
      <span className="text-xs text-muted-foreground">Suggested by code owners:</span>
      {suggestions.map(suggestion => (
        <Tooltip key={`${suggestion.group}\n${suggestion.fullName}`}>
          <TooltipTrigger render={<Button variant="outline" size="xs" onClick={() => onChoose(suggestion)} />}>
            {suggestion.login === null ? <SearchIcon data-icon="inline-start" /> : <PlusIcon data-icon="inline-start" />}
            {suggestion.login === null ? suggestion.fullName : `${suggestion.fullName} (${suggestion.login})`}
          </TooltipTrigger>
          <TooltipContent>
            {suggestion.login === null ? `${suggestion.group}: Patronus doesn't say the GitHub login, find ${suggestion.fullName}` : suggestion.group}
          </TooltipContent>
        </Tooltip>
      ))}
    </div>
  )
}
