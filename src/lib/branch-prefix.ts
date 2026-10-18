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
