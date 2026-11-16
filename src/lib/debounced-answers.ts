/**
 * The answers of one form by input (`useDebouncedRead`), for what `reset` named when they were asked; the latest answer
 * apart. An answer the read does not keep (a failed call of its command) shows as the latest, for the input it failed
 * for, and is asked again: by the next pause of the typing at that input and by a submit (`settled`)
 */
export interface Answers<T> {
  readonly reset: string
  readonly byInput: ReadonlyMap<string, T>
  readonly latest: T | undefined
  /** The input whose latest answer is not kept: no answer is pending for it, though none is known */
  readonly unkept: string | null
}

export function noAnswers<T>(reset: string, latest: T | undefined): Answers<T> {
  return { reset, byInput: new Map(), latest, unkept: null }
}

/**
 * `answers` with the answer of `input`, asked for what `reset` names; an answer for another `reset` is dropped, one not
 * `kept` (the caller decided it before the state update, outside the render) shows as the latest without being kept
 */
export function withAnswer<T>(answers: Answers<T>, reset: string, input: string, answer: T, kept: boolean): Answers<T> {
  if (answers.reset !== reset) {
    return answers
  }
  if (!kept) {
    return { ...answers, latest: answer, unkept: input }
  }
  const byInput = new Map(answers.byInput)
  byInput.set(input, answer)
  return { reset, byInput, latest: answer, unkept: null }
}

/** The kept answer of `input` for `reset`; undefined when it was never asked, failed or `reset` changed */
export function keptAnswer<T>(answers: Answers<T>, reset: string, input: string | null): T | undefined {
  return input === null || answers.reset !== reset ? undefined : answers.byInput.get(input)
}

/** Whether the answer of `input` is still to come: neither kept nor the latest one that was not kept */
export function isPending<T>(answers: Answers<T>, reset: string, input: string | null): boolean {
  return input !== null && keptAnswer(answers, reset, input) === undefined && !(answers.reset === reset && answers.unkept === input)
}
