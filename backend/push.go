package backend

import (
  "fmt"
  "os/exec"
  "strings"
)

// PushBranchToRemote pushes a specific branch to the remote repository
func PushBranchToRemote(repositoryPath string, branchPrefix string, branchName string) ActionResult {
  finalBranchName := toFinalBranchName(branchPrefix, branchName)

  cmd := exec.Command("git", "push", "origin", finalBranchName)
  cmd.Dir = repositoryPath

  output, err := cmd.CombinedOutput()
  outputStr := strings.TrimSpace(string(output))
  if err != nil {
    return ActionResult{
      Success: false,
      Message: fmt.Sprintf("failed to push branch '%s': %s", branchName, outputStr),
    }
  }

  return ActionResult{
    Success: true,
    Message: outputStr,
  }
}
