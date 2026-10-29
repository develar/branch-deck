import { describe, expect, it } from "vite-plus/test"
import { describeAgentRules, describeInstall } from "@/hooks/useInstallAgentCli"

const homeDir = "/Users/ann"
const claudeSkill = "/Users/ann/.claude/skills/branch-deck/SKILL.md"
const skills = [claudeSkill, "/Users/ann/.agents/skills/branch-deck/SKILL.md"]
const defaults = { lease: true, subject_branch: true }

describe("describeInstall", () => {
  it("names the skills, and bd as a command when it is on PATH", () => {
    expect(describeInstall({ bd: "/Users/ann/.local/bin/bd", on_path: true, skills, agents: defaults }, homeDir)).toBe(
      "Coding agents find bd through the skills in ~/.claude/skills/branch-deck/SKILL.md, ~/.agents/skills/branch-deck/SKILL.md. In a terminal, run bd. Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.",
    )
  })

  it("names the full path and the directory to add to PATH when bd is not on it", () => {
    expect(describeInstall({ bd: "C:\\Program Files\\BranchDeck\\bd.exe", on_path: false, skills: [claudeSkill], agents: defaults }, homeDir)).toBe(
      `Coding agents find bd through the skill in ~/.claude/skills/branch-deck/SKILL.md. In a terminal, run C:\\Program Files\\BranchDeck\\bd.exe, or add C:\\Program Files\\BranchDeck to PATH to run bd. Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.`,
    )
  })
})

describe("describeAgentRules", () => {
  it("says how agents work by the rules of every repository", () => {
    expect(describeAgentRules({ lease: false, subject_branch: false })).toBe("Agents work in your checkout; commit subjects stay as written.")
    expect(describeAgentRules({ lease: true, subject_branch: true })).toBe(
      "Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.",
    )
  })
})
