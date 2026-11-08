import { describe, expect, it } from "vite-plus/test"
import { describeAgentRules, describeInstall } from "@/hooks/useInstallAgentCli"

const homeDir = "/Users/ann"
const claudeSkill = "/Users/ann/.claude/skills/branch-deck/SKILL.md"
const skills = [claudeSkill, "/Users/ann/.agents/skills/branch-deck/SKILL.md"]
const defaults = { lease: true, subject_branch: true, pull_request: false }

describe("describeInstall", () => {
  it("names the skills, and brd as a command when it is on PATH", () => {
    expect(describeInstall({ bd: "/Users/ann/.local/bin/brd", on_path: true, skills, agents: defaults }, homeDir)).toBe(
      "Coding agents find brd through the skills in ~/.claude/skills/branch-deck/SKILL.md, ~/.agents/skills/branch-deck/SKILL.md. In a terminal, run brd. Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.",
    )
  })

  it("names the full path and the directory to add to PATH when brd is not on it", () => {
    expect(describeInstall({ bd: "C:\\Program Files\\BranchDeck\\brd.exe", on_path: false, skills: [claudeSkill], agents: defaults }, homeDir)).toBe(
      `Coding agents find brd through the skill in ~/.claude/skills/branch-deck/SKILL.md. In a terminal, run C:\\Program Files\\BranchDeck\\brd.exe, or add C:\\Program Files\\BranchDeck to PATH to run brd. Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.`,
    )
  })
})

describe("describeAgentRules", () => {
  it("says how agents work by the rules of every repository", () => {
    expect(describeAgentRules({ lease: false, subject_branch: false, pull_request: false })).toBe("Agents work in your checkout; commit subjects stay as written.")
    expect(describeAgentRules({ lease: true, subject_branch: true, pull_request: false })).toBe(
      "Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.",
    )
    expect(describeAgentRules({ lease: true, subject_branch: true, pull_request: true })).toBe(
      "Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject. They push each branch and open its pull request without asking.",
    )
  })
})
