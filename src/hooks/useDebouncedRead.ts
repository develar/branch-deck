import { useEffect, useEffectEvent, useState } from "react"
import pDebounce from "p-debounce"
import { isPending, keptAnswer, noAnswers, shownAnswer, withAnswer } from "@/lib/debounced-answers"
import type { Answers } from "@/lib/debounced-answers"

/** How long the typing pauses before Rust is asked */
export const READ_DELAY_MS = 100

export interface DebouncedRead<T> {
  /** The answer for the current input; the latest answer of an earlier input while it is asked, undefined before any */
  readonly answer: T | undefined
  /** The answer is not for the current input yet */
  readonly pending: boolean
  /** The answer for the current input, asked now when it is not there yet: a submit right after typing waits for it */
  readonly settled: () => Promise<T | undefined>
}

/** A read of one pause of the typing: the calls of the pause settle with the answer of its latest input */
function debouncedRead<T>() {
  return pDebounce(async (input: string, reset: string, ask: (input: string) => Promise<T>) => ({ input, reset, answer: await ask(input) }), READ_DELAY_MS)
}

/** Every answer is kept */
const KEEP_ALL = () => true

/**
 * A read of Rust for what the user types (`input`), asked once the typing pauses for `READ_DELAY_MS` (`p-debounce`),
 * the answer of each input kept, so typing back shows it at once; null asks nothing. `reset` names what the answers
 * depend on besides the input (the repository, its last sync): a change of it asks again. An answer `keep` refuses (a
 * failed call of the command) shows, is no pending one, and is asked again (`withAnswer` of src/lib/debounced-answers.ts)
 */
export function useDebouncedRead<T>(input: string | null, ask: (input: string) => Promise<T>, reset = "", keep: (answer: T) => boolean = KEEP_ALL): DebouncedRead<T> {
  const [answers, setAnswers] = useState<Answers<T>>(() => noAnswers<T>(reset, undefined))
  const [read] = useState(debouncedRead<T>)
  if (answers.reset !== reset) {
    // the answers of what `reset` named before are no answers now; the latest stays shown until the next one
    setAnswers(noAnswers(reset, answers.latest))
  }
  const answered = keptAnswer(answers, reset, input)
  const keepAnswer = useEffectEvent((answer: T) => keep(answer))
  const askLatest = useEffectEvent((value: string) => ask(value))

  const known = answered !== undefined
  useEffect(() => {
    if (input === null || known) {
      return
    }
    void read(input, reset, value => askLatest(value)).then(done => {
      // decided here, never in the updater: React runs an updater in its render, where an effect event must not run
      const kept = keepAnswer(done.answer)
      setAnswers(previous => withAnswer(previous, done.reset, done.input, done.answer, kept))
    })
  }, [input, reset, known, read])

  const settled = async (): Promise<T | undefined> => {
    if (input === null || answered !== undefined) {
      return answered
    }
    // asked again also after an answer that was not kept
    const answer = await ask(input)
    const kept = keep(answer)
    setAnswers(previous => withAnswer(previous, reset, input, answer, kept))
    return answer
  }
  return { answer: shownAnswer(answers, reset, input), pending: isPending(answers, reset, input), settled }
}
