import { useState } from "react"
import { Trash2Icon } from "lucide-react"
import { InlineInputDialog } from "@/components/shared/InlineInputDialog"
import { getSimpleBranchName } from "@/lib/branch"

export interface InlineDeleteConfirmationInputProps {
  branchName: string
  title: string
  description: string
  onSubmit: () => void
  onCancel: () => void
}

/** Confirms the deletion of an archived branch by typing its simple name. */
export function InlineDeleteConfirmationInput({ branchName, title, description, onSubmit, onCancel }: InlineDeleteConfirmationInputProps) {
  const [inputText, setInputText] = useState("")
  const simpleName = getSimpleBranchName(branchName)
  const isValid = inputText === simpleName
  const errorMessage = isValid ? undefined : `Type the exact name: ${simpleName}`

  return (
    <InlineInputDialog
      value={inputText}
      onValueChange={setInputText}
      placeholder={`Type ${simpleName} to confirm deletion`}
      validationMessage={errorMessage}
      validationColor={errorMessage ? "error" : "primary"}
      validationTextClassName={errorMessage ? "text-destructive" : ""}
      canSubmit={isValid}
      title={title}
      description={description}
      submitText="Delete"
      dataTestId="inline-delete-input"
      leadingIcon={<Trash2Icon className="size-4 text-muted-foreground" />}
      helpText="This action will permanently delete the archived branch. This cannot be undone."
      onSubmit={onSubmit}
      onCancel={onCancel}
    />
  )
}
