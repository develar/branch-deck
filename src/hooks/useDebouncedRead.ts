import { useEffect, useEffectEvent, useState } from "react"
import pDebounce from "p-debounce"

/** How long the typing pauses before Rust is asked */
export const READ_DELAY_MS = 100

/** The answers of one form by input, for what `reset` named when they were asked; the latest answer apart */
interface Answers<T> {
  readonly reset: string
  readonly byInput: ReadonlyMap<string, T>
  readonly latest: T | undefined
}

export interface DebouncedRead<T> {
  /** The answer for the current input; the latest answer of an earlier input while it is asked, undefined before any */
  readonly answer: T | undefined
  /** The answer is not for the current input yet */
  readonly pending: boolean
  /** The answer for the current input, asked now when it is not there yet: a submit right after typing waits for it */
  readonly settled: () => Promise<T | undefined>
}

/** `answers` with the answer of `input`, asked for what `reset` names; an answer for another `reset` is dropped */
function withAnswer<T>(answers: Answers<T>, reset: string, input: string, answer: T): Answers<T> {
  if (answers.reset !== reset) {
    return answers
  }
  const byInput = new Map(answers.byInput)
  byInput.set(input, answer)
  return { reset, byInput, latest: answer }
}

/** A read of one pause of the typing: the calls of the pause settle with the answer of its latest input */
function debouncedRead<T>() {
  return pDebounce(async (input: string, reset: string, ask: (input: string) => Promise<T>) => ({ input, reset, answer: await ask(input) }), READ_DELAY_MS)
}

/**
 * A read of Rust for what the user types (`input`), asked once the typing pauses for `READ_DELAY_MS` (`p-debounce`),
 * the answer of each input kept, so typing back shows it at once; null asks nothing. `reset` names what the answers
 * depend on besides the input (the repository, its last sync): a change of it asks again
 */
export function useDebouncedRead<T>(input: string | null, ask: (input: string) => Promise<T>, reset = ""): DebouncedRead<T> {
  const [answers, setAnswers] = useState<Answers<T>>(() => ({ reset, byInput: new Map(), latest: undefined }))
  const [read] = useState(debouncedRead<T>)
  if (answers.reset !== reset) {
    // the answers of what `reset` named before are no answers now; the latest stays shown until the next one
    setAnswers({ reset, byInput: new Map(), latest: answers.latest })
  }
  const answered = input === null || answers.reset !== reset ? undefined : answers.byInput.get(input)
  const askLatest = useEffectEvent((value: string) => ask(value))

  const known = answered !== undefined
  useEffect(() => {
    if (input === null || known) {
      return
    }
    void read(input, reset, value => askLatest(value)).then(done => setAnswers(previous => withAnswer(previous, done.reset, done.input, done.answer)))
  }, [input, reset, known, read])

  const settled = async (): Promise<T | undefined> => {
    if (input === null || answered !== undefined) {
      return answered
    }
    const answer = await ask(input)
    setAnswers(previous => withAnswer(previous, reset, input, answer))
    return answer
  }
  return { answer: answered ?? answers.latest, pending: input !== null && answered === undefined, settled }
}
