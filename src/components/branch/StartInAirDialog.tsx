import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { CheckIcon, ClipboardCopyIcon, RotateCcwIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ItemSelect } from "@/components/shared/ItemSelect"
import { PRIMARY_KBD_CLASS_NAME } from "@/components/shared/MenuItemButton"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { useCopyToClipboard } from "@/hooks/useCopyToClipboard"
import {
  choiceOf,
  effortsOf,
  findTarget,
  formatEffort,
  formatIde,
  formatModel,
  formatRefusal,
  formatStartHint,
  initialForm,
  launchInAir,
  launchRequest,
  readAirTargets,
  targetKey,
  withModel,
  withTarget,
} from "@/lib/air-launch"
import type { AirLaunchForm, AirRefusal } from "@/lib/air-launch"
import type { AirIde, AirStartKind, AirTarget, PatronusRobot } from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { formatPushMode, formatRobotSubject } from "@/lib/patronus"
import { isMac } from "@/lib/platform"
import { readRobotPrompt } from "@/lib/robot-prompt-copy"
import { showToast } from "@/lib/toast-message"
import { closeAirLaunchDialog, useAirLaunchDialog } from "@/stores/air-launch-dialog"
import { updateProject, useRecentProjects } from "@/stores/settings/repositorySettings"

/** The default of the agent in the effort picker: a launch without an effort */
const DEFAULT_EFFORT = "default"

type TargetsState =
  | { status: "loading" }
  | { status: "noIde" }
  | { status: "listed"; ide: AirIde; targets: AirTarget[] }
  | { status: "refused"; ide: AirIde; code: string; message: string | null }
  | { status: "error"; message: string }

type PromptState = { status: "loading" } | { status: "loaded" } | { status: "error"; message: string }

/**
 * "Start in Air" of a failed robot: the agents of the IDE with Air that has the repository open, a model and an effort
 * of the agent, Run or Draft, and the prompt the robot's "Copy Prompt for Agent" copies, to edit; ⌘↵ starts it. The IDE
 * may ask the user first. Without an IDE it says so, and Copy Prompt stays.
 *
 * Loaded on the first open, then kept mounted (see `PreviewDialogFrame`); each opening reads the agents and the prompt
 * anew.
 */
export function StartInAirDialog() {
  const { robot, repositoryPath, generation } = useAirLaunchDialog()
  // the last robot stays while the closing animation runs
  const [shown, setShown] = useState<{ robot: PatronusRobot; repositoryPath: string; generation: number } | null>(null)
  if (robot !== null && shown?.generation !== generation) {
    setShown({ robot, repositoryPath, generation })
  }
  const [launching, setLaunching] = useState(false)
  const promptRef = useRef<HTMLTextAreaElement>(null)
  return (
    <PreviewDialogFrame
      data-testid="start-in-air-dialog"
      width="form"
      open={robot !== null}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        // a click outside keeps the edited prompt, and nothing closes it while the IDE answers
        if (eventDetails.reason === "outside-press" || launching) {
          eventDetails.cancel()
          return
        }
        closeAirLaunchDialog()
      }}
      initialFocus={promptRef}
    >
      {shown !== null && (
        <StartInAirForm key={shown.generation} robot={shown.robot} repositoryPath={shown.repositoryPath} promptRef={promptRef} launching={launching} setLaunching={setLaunching} />
      )}
    </PreviewDialogFrame>
  )
}

function StartInAirForm({
  robot,
  repositoryPath,
  promptRef,
  launching,
  setLaunching,
}: {
  robot: PatronusRobot
  repositoryPath: string
  promptRef: RefObject<HTMLTextAreaElement | null>
  launching: boolean
  setLaunching: (launching: boolean) => void
}) {
  const subject = formatRobotSubject(robot)
  const title = `Fix the ${formatPushMode(robot.pushMode)} of ${subject}`
  const remembered = useRecentProjects(projects => projects.find(project => project.path === repositoryPath)?.airLaunch)

  // each Try Again asks the IDE anew
  const [attempt, setAttempt] = useState(0)
  const [targets, setTargets] = useState<TargetsState>({ status: "loading" })
  useEffect(() => {
    let current = true
    readAirTargets(repositoryPath).then(
      answer => {
        if (current) {
          setTargets(answer)
        }
      },
      (error: unknown) => {
        if (current) {
          setTargets({ status: "error", message: getErrorDetails(error) })
        }
      },
    )
    return () => {
      current = false
    }
  }, [repositoryPath, attempt])

  const [promptState, setPromptState] = useState<PromptState>({ status: "loading" })
  const [prompt, setPrompt] = useState("")
  useEffect(() => {
    let current = true
    readRobotPrompt(robot, false).then(
      read => {
        if (current) {
          setPrompt(read.prompt)
          setPromptState({ status: "loaded" })
        }
      },
      (error: unknown) => {
        if (current) {
          setPromptState({ status: "error", message: getErrorDetails(error) })
        }
      },
    )
    return () => {
      current = false
    }
  }, [robot])

  const listed = targets.status === "listed" ? targets.targets : null
  // the pickers start from the remembered choice once the IDE listed its agents, and on each new list
  const [form, setForm] = useState<AirLaunchForm | null>(null)
  const [formOf, setFormOf] = useState<AirTarget[] | null>(null)
  if (listed !== null && formOf !== listed) {
    setFormOf(listed)
    setForm(initialForm(listed, remembered))
  }
  const target = form === null || listed === null ? undefined : findTarget(listed, form.target)
  const [refusal, setRefusal] = useState<AirRefusal | null>(null)

  const hint = form === null ? null : formatStartHint(form, prompt, refusal)
  const canStart = !launching && form !== null && target !== undefined && promptState.status === "loaded" && hint === null

  const start = async () => {
    if (!canStart) {
      return
    }
    setLaunching(true)
    const request = launchRequest(repositoryPath, form, target, prompt, title)
    // `.finally`, not try/finally: React Compiler cannot compile a try without a catch
    const answer = await launchInAir(request)
      .catch((error: unknown) => {
        notifyError("Cannot start the session in Air", error)
        return null
      })
      .finally(() => setLaunching(false))
    if (answer === null) {
      return
    }
    switch (answer.status) {
      case "noIde":
        setTargets({ status: "noIde" })
        return
      case "refused":
        setRefusal({ code: answer.code, message: answer.message, form, prompt })
        return
      case "accepted":
        updateProject(repositoryPath, { airLaunch: choiceOf(form, target) })
        closeAirLaunchDialog()
        showToast({
          title: form.start === "run" ? `Started ${target.name} in Air` : `Opened a draft for ${target.name} in Air`,
          description: `${title} · ${formatIde(answer.ide)}`,
        })
    }
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      void start()
    }
  }

  const ide = targets.status === "listed" || targets.status === "refused" ? targets.ide : null
  return (
    <>
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">Start in Air</DialogTitle>
        <p data-testid="start-in-air-subject" className="truncate text-xs text-muted-foreground">
          {ide === null ? title : `${title} · ${formatIde(ide)} · ${ide.project}`}
        </p>
      </PreviewDialogHeader>

      <PreviewDialogBody>
        <div className="flex flex-col gap-4" onKeyDown={handleKeyDown}>
          <TargetsNotice state={targets} onRetry={() => setAttempt(attempt + 1)} />
          {refusal !== null && (
            <Alert variant="destructive" data-testid="start-in-air-refusal">
              <TriangleAlertIcon />
              <AlertDescription>{formatRefusal(refusal.code, refusal.message)}</AlertDescription>
            </Alert>
          )}
          {listed !== null && form !== null && target !== undefined && <Pickers targets={listed} target={target} form={form} disabled={launching} onChange={setForm} />}
          <label className="flex flex-col gap-1.5">
            <span className="text-xs text-muted-foreground">Prompt</span>
            <Textarea
              ref={promptRef}
              data-testid="start-in-air-prompt"
              className="min-h-40 font-mono text-xs"
              value={prompt}
              placeholder={promptState.status === "loading" ? "Reading the failure…" : undefined}
              disabled={launching || promptState.status === "loading"}
              onChange={event => setPrompt(event.target.value)}
            />
          </label>
          {promptState.status === "error" && <p className="text-xs text-destructive">{`Cannot read the failure of ${subject}: ${promptState.message}`}</p>}
        </div>
      </PreviewDialogBody>

      <PreviewDialogFooter start={<CopyPromptButton prompt={prompt} disabled={promptState.status !== "loaded"} />}>
        <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
        <Tooltip disabled={canStart || launching || hint === null}>
          <TooltipTrigger
            render={
              <Button
                data-testid="start-in-air-submit"
                size="xs"
                disabled={!canStart}
                focusableWhenDisabled
                className={cn(!canStart && !launching && "opacity-50")}
                onClick={() => void start()}
              />
            }
          >
            {launching && <Spinner data-icon="inline-start" />}
            {form?.start === "draft" ? "Open Draft" : "Start"}
            <Kbd className={PRIMARY_KBD_CLASS_NAME}>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
          </TooltipTrigger>
          <TooltipContent>{hint}</TooltipContent>
        </Tooltip>
      </PreviewDialogFooter>
    </>
  )
}

/** What the IDE answered instead of its agents: the wait, no IDE, a refusal, an error; with Try Again */
function TargetsNotice({ state, onRetry }: { state: TargetsState; onRetry: () => void }) {
  if (state.status === "loading") {
    return (
      <p data-testid="start-in-air-loading" className="flex items-center gap-2 text-xs text-muted-foreground">
        <Spinner />
        Asking the IDE for its agents… It may ask you first.
      </p>
    )
  }
  if (state.status === "listed") {
    return null
  }
  const text =
    state.status === "noIde"
      ? "No IDE with Air has the repository open. Open it in an IDE with Air and try again, or copy the prompt."
      : state.status === "refused"
        ? formatRefusal(state.code, state.message)
        : `Cannot ask the IDE for its agents: ${state.message}`
  return (
    <Alert variant={state.status === "noIde" ? "info" : "destructive"} data-testid="start-in-air-unavailable">
      <TriangleAlertIcon />
      <AlertDescription className="flex flex-wrap items-center gap-x-2">
        {text}
        <Button variant="link" size="xs" className="h-auto p-0" onClick={onRetry}>
          <RotateCcwIcon data-icon="inline-start" />
          Try Again
        </Button>
      </AlertDescription>
    </Alert>
  )
}

/** The agent, its model and effort, and Run or Draft */
function Pickers({
  targets,
  target,
  form,
  disabled,
  onChange,
}: {
  targets: AirTarget[]
  target: AirTarget
  form: AirLaunchForm
  disabled: boolean
  onChange: (form: AirLaunchForm) => void
}) {
  const agents = targets.map(it => ({ value: targetKey(it), label: it.name }))
  const models = (target.models ?? []).map(it => ({ value: it.id, label: formatModel(it) }))
  const efforts = [{ value: DEFAULT_EFFORT, label: "Default" }, ...effortsOf(target, form.model).map(it => ({ value: it, label: formatEffort(it) }))]
  return (
    <div className="flex flex-wrap items-end gap-4">
      <Field label="Agent">
        <ItemSelect
          items={agents}
          value={form.target}
          disabled={disabled}
          className="w-44"
          testId="start-in-air-agent"
          onChange={next => onChange(withTarget(form, targets, next))}
        />
      </Field>
      <Field label="Model">
        <ItemSelect
          items={models}
          value={form.model}
          disabled={disabled || models.length === 0}
          className="w-52"
          testId="start-in-air-model"
          placeholder="Default of the agent"
          onChange={next => onChange(withModel(form, target, next))}
        />
      </Field>
      <Field label="Effort">
        <ItemSelect
          items={efforts}
          value={form.effort ?? DEFAULT_EFFORT}
          disabled={disabled || efforts.length === 1}
          className="w-32"
          testId="start-in-air-effort"
          onChange={next => onChange({ ...form, effort: next === DEFAULT_EFFORT ? null : next })}
        />
      </Field>
      <Field label="Start">
        <ToggleGroup
          aria-label="Start"
          data-testid="start-in-air-start"
          variant="outline"
          size="sm"
          spacing={0}
          value={[form.start]}
          disabled={disabled}
          onValueChange={next => {
            const chosen = next.find(it => it !== form.start)
            if (chosen !== undefined) {
              onChange({ ...form, start: chosen as AirStartKind })
            }
          }}
        >
          <ToggleGroupItem value="run">Run</ToggleGroupItem>
          <ToggleGroupItem value="draft">Draft</ToggleGroupItem>
        </ToggleGroup>
      </Field>
    </div>
  )
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      {children}
    </div>
  )
}

/** Copy Prompt: the prompt as edited, "Copied" in place */
function CopyPromptButton({ prompt, disabled }: { prompt: string; disabled: boolean }) {
  const { isCopied, copyToClipboard } = useCopyToClipboard()
  return (
    <Button data-testid="start-in-air-copy" variant="ghost" size="xs" disabled={disabled} onClick={() => void copyToClipboard(prompt)}>
      {isCopied ? <CheckIcon data-icon="inline-start" /> : <ClipboardCopyIcon data-icon="inline-start" />}
      {isCopied ? "Copied" : "Copy Prompt"}
    </Button>
  )
}
