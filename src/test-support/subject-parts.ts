import type { SubjectParts } from "@/lib/bindings"

/** A subject as Rust splits it (`subject_parts`): `rest` without markers, prefix, issue key or lead unless `parts` names them */
export function testSubject(rest: string, parts: Partial<SubjectParts> = {}): SubjectParts {
  return { autosquash: "", prefix: null, rest, issueKey: null, lead: null, ...parts }
}
