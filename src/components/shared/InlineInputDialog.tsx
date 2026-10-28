import { useImperativeHandle, useRef, useState } from "react"
import type { ReactNode, Ref } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { Spinner } from "@/components/ui/spinner"
import { InlineForm } from "@/components/shared/InlineForm"

export interface InlineInputDialogHandle {
  selectText: () => void
  focusInput: () => void
}

export interface InlineInputDialogProps {
  ref?: Ref<InlineInputDialogHandle>
  title: string
  description?: string
  dataTestId?: string
  cancelText?: string
  submitText?: string
  submitIcon?: ReactNode
  /** `destructive` for a submit that deletes */
  submitVariant?: "default" | "destructive"
  canSubmit?: boolean
  loading?: boolean
  /**
   * Called on a submit while `canSubmit`; a form whose check answers later returns whether it submitted, and the
   * validation message shows when it refused
   */
  onSubmit: (() => void) | (() => Promise<boolean>)
  onCancel: () => void

  value: string
  onValueChange: (value: string) => void
  placeholder?: string
  /** Shown once the user has tried to submit */
  validationMessage?: string
  validationColor?: "primary" | "error"
  validationTextClassName?: string
  leadingIcon?: ReactNode

  /** Replaces the default input */
  input?: ReactNode
  /** Next to the buttons (e.g. AI controls) */
  extraActions?: ReactNode
  /** Replaces the validation message text */
  validationContent?: ReactNode
  helpText?: ReactNode
  /** Below the controls (e.g. suggestions) */
  afterControls?: ReactNode
}

/**
 * Inline form with a single text input (`max-w-md`, a name or a key is short) and inline Cancel/Submit buttons; Enter
 * submits, Escape cancels.
 */
export function InlineInputDialog({
  ref,
  title,
  description,
  dataTestId,
  cancelText = "Cancel",
  submitText = "Submit",
  submitIcon,
  submitVariant = "default",
  canSubmit = true,
  loading = false,
  onSubmit,
  onCancel,
  value,
  onValueChange,
  placeholder = "",
  validationMessage,
  validationColor = "primary",
  validationTextClassName = "",
  leadingIcon,
  input,
  extraActions,
  validationContent,
  helpText,
  afterControls,
}: InlineInputDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  // the validation message is shown only after a rejected submit attempt
  const [hasInteracted, setHasInteracted] = useState(false)

  useImperativeHandle(
    ref,
    () => ({
      selectText: () => inputRef.current?.select(),
      focusInput: () => inputRef.current?.focus(),
    }),
    [],
  )

  const handleSubmit = () => {
    if (!canSubmit) {
      setHasInteracted(true)
      return
    }
    const submitted = onSubmit()
    if (submitted instanceof Promise) {
      void submitted.then(done => {
        if (!done) {
          setHasInteracted(true)
        }
      })
    }
  }

  return (
    <InlineForm
      title={title}
      description={description}
      dataTestId={dataTestId}
      contentClassName="flex max-w-3xl flex-col gap-2"
      showActions={false}
      onCancel={onCancel}
      onSubmit={handleSubmit}
      autoFocusRef={input === undefined ? inputRef : undefined}
    >
      <div className="flex items-center gap-2">
        <div className="relative max-w-md flex-1">
          {input ?? (
            <InputGroup
              className={cn(
                "bg-background",
                validationColor === "error" &&
                  "border-destructive has-[[data-slot=input-group-control]:focus-visible]:border-destructive has-[[data-slot=input-group-control]:focus-visible]:ring-destructive/20",
              )}
            >
              {leadingIcon && <InputGroupAddon>{leadingIcon}</InputGroupAddon>}
              <InputGroupInput
                ref={inputRef}
                type="text"
                value={value}
                placeholder={placeholder}
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                aria-invalid={validationColor === "error" && hasInteracted ? true : undefined}
                onChange={event => onValueChange(event.target.value)}
                onKeyDown={event => {
                  if (event.key === "Enter") {
                    event.preventDefault()
                    handleSubmit()
                  }
                }}
              />
            </InputGroup>
          )}
        </div>

        {extraActions}

        <div className="flex items-center gap-2">
          <Button size="xs" variant="ghost" onClick={onCancel}>
            {cancelText}
          </Button>
          <Button size="xs" variant={submitVariant} disabled={!canSubmit || loading} aria-busy={loading || undefined} onClick={handleSubmit}>
            {loading ? <Spinner data-icon="inline-start" /> : submitIcon}
            {submitText}
          </Button>
        </div>
      </div>

      {hasInteracted && validationMessage && <div className={cn("text-xs", validationTextClassName)}>{validationContent ?? validationMessage}</div>}

      {helpText && <div className="text-xs text-muted-foreground">{helpText}</div>}

      {afterControls}
    </InlineForm>
  )
}
