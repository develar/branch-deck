/** As the backend builds `{prefix}/virtual/{name}` (`qualified_branch_name`): `jane/` and ` jane ` are `jane` */
export function normalizeBranchPrefix(prefix: string | undefined): string {
  return prefix?.trim().replace(/\/+$/, "").trim() ?? ""
}

/** What git refuses in a ref name, besides the slash that separates the parts of the branch name */
const INVALID_PREFIX = /[\s~^:?*[\\]|\.\.|@\{/

/** What `isValidBranchPrefix` refuses, under every field of a prefix (General, the welcome card, Settings → Repository) */
export const INVALID_PREFIX_TEXT = 'A prefix cannot contain spaces, "..", "@{" or any of ~ ^ : ? * [ \\'

export function isValidBranchPrefix(prefix: string): boolean {
  return prefix !== "" && !INVALID_PREFIX.test(prefix)
}

/**
 * `globalUserBranchPrefix` of a value of the `appSettings` key as the writer has it (`onWriterChange`): a reader of the
 * prefix Rust resolves asks again once it differs
 */
export function appPrefixOf(value: unknown): unknown {
  return typeof value === "object" && value !== null && "globalUserBranchPrefix" in value ? value.globalUserBranchPrefix : undefined
}
