// The problems of a frame or a file of a corpus against a JSON Schema: a token `${name}` of a line stands for any value,
// so an error at one, and the errors of a branch that did not apply (`oneOf`, `if`, `anyOf`), are none. The contract
// tests of the IDE (src/platform/air/contract.test.ts) and of the backend (src/platform/backend/contract.test.ts)
import type { ErrorObject, ValidateFunction } from "ajv/dist/2020.js"

/** The value at the JSON pointer `pointer` of `data` */
function at(data: unknown, pointer: string): unknown {
  return pointer
    .split("/")
    .slice(1)
    .reduce<unknown>(
      (value, key) => (value === null || typeof value !== "object" ? undefined : (value as Record<string, unknown>)[key.replaceAll("~1", "/").replaceAll("~0", "~")]),
      data,
    )
}

/** The errors of `validate` on `data` that are no token, one line each */
export function schemaProblems(validate: ValidateFunction, data: unknown): string[] {
  if (validate(data)) {
    return []
  }
  const token = (error: ErrorObject) => {
    const value = at(data, error.instancePath)
    return typeof value === "string" && /^\$\{[^}]+\}$/.test(value)
  }
  return (validate.errors ?? [])
    .filter(error => !token(error) && !["oneOf", "if", "anyOf"].includes(error.keyword))
    .map(error => `${error.instancePath} ${error.message} ${JSON.stringify(error.params)}`)
}
