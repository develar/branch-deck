/**
 * The answers of one form by input (`useDebouncedRead`), for what `reset` named when they were asked; the latest answer
 * apart. An answer the read does not keep (a failed call of its command) shows as the latest, for the input it failed
 * for, and is asked again: by the next pause of the typing at that input and by a submit (`settled`)
 */
export interface Answers<T> {
  readonly reset: string
  readonly byInput: ReadonlyMap<string, T>
  readonly latest: T | undefined
  /**
   * The answer not kept, with its input: no answer is pending for that input, though none is known, and it is the
   * answer of that input until one of the same input is kept. A kept answer of another input (a slower window of the
   * debounce that ends after it) leaves it
   */
  readonly unkept: { readonly input: string; readonly answer: T } | null
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
    return { ...answers, latest: answer, unkept: { input, answer } }
  }
  const byInput = new Map(answers.byInput)
  byInput.set(input, answer)
  return { reset, byInput, latest: answer, unkept: answers.unkept?.input === input ? null : answers.unkept }
}

/** The kept answer of `input` for `reset`; undefined when it was never asked, failed or `reset` changed */
export function keptAnswer<T>(answers: Answers<T>, reset: string, input: string | null): T | undefined {
  return input === null || answers.reset !== reset ? undefined : answers.byInput.get(input)
}

/** The answer not kept of `input` for `reset`, if its latest one was not kept */
function unkeptAnswer<T>(answers: Answers<T>, reset: string, input: string | null): T | undefined {
  return input !== null && answers.reset === reset && answers.unkept?.input === input ? answers.unkept.answer : undefined
}

/**
 * The answer shown for `input`: its kept one, else the one not kept of it, else the latest of an earlier input while it
 * is asked; undefined before any
 */
export function shownAnswer<T>(answers: Answers<T>, reset: string, input: string | null): T | undefined {
  return keptAnswer(answers, reset, input) ?? unkeptAnswer(answers, reset, input) ?? answers.latest
}

/** Whether the answer of `input` is still to come: neither kept nor one not kept of it */
export function isPending<T>(answers: Answers<T>, reset: string, input: string | null): boolean {
  return input !== null && keptAnswer(answers, reset, input) === undefined && unkeptAnswer(answers, reset, input) === undefined
}
