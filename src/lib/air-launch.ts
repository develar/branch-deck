import type { AirDraftAnswer } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { commandFailure } from "@/lib/errors"

/*
 * Start in Air (openspec/specs/branch/start-in-air/spec.md): the command that opens the New Session tab with a prompt in
 * the IDE with Air that has the repository open, and the words of the IDE and of a refusal. The IDE takes the default
 * agent of its New Session button; the user picks the agent, the model and the rest in the tab.
 */

/** The IDE of the answer: "IU-263.1234" */
export function formatIde(ide: { product: string; build: string }): string {
  return ide.build === "" ? ide.product : ide.build
}

const REFUSALS: Record<string, string> = {
  DENIED: "The IDE denied Branch Deck, or nobody answered its question in time",
  UNAVAILABLE: "The IDE cannot ask for consent now. Try again with the IDE window open",
  BUSY: "The IDE answers another request now. Try again in a moment",
  RATE_LIMITED: "Branch Deck asked the IDE too often in the last minute. Try again later",
  UNAUTHORIZED: "The IDE does not let local apps in now. Update Air, or restart the IDE",
  UNKNOWN_METHOD: "The IDE does not know this request. Update Air",
  BAD_REQUEST: "The IDE cannot read the request of Branch Deck",
  BAD_PARAMS: "The IDE cannot read the request of Branch Deck",
  NO_PROJECT: "No project open in the IDE holds the repository",
  NO_DEFAULT: "The New Session button of the IDE has no default agent. Pick an agent there, then try again",
  UNTRUSTED_PROJECT: "The IDE does not trust the project. Trust it in the IDE first",
  TARGET_UNAVAILABLE: "The default agent is not available in the IDE now",
  UNKNOWN_AGENT: "The IDE does not know the default agent now",
  LAUNCH_FAILED: "The IDE cannot open the New Session tab",
}

/** A refusal of the IDE by its code; the message of the IDE only for `LAUNCH_FAILED` and an unknown code */
export function formatRefusal(code: string, message: string | null): string {
  const words = REFUSALS[code]
  if (words === undefined) {
    return message === null ? `The IDE refused the request (${code})` : `The IDE refused the request (${code}): ${message}`
  }
  return code === "LAUNCH_FAILED" && message !== null ? `${words}: ${message}` : words
}

/** Opens the New Session tab with `prompt` in the IDE that has `project` open; the IDE may ask the user first */
export async function openDraftInAir(project: string, prompt: string): Promise<AirDraftAnswer> {
  const result = await commands.airOpenDraft({ project, prompt })
  if (result.status === "error") {
    throw commandFailure(result.error)
  }
  return result.data
}
