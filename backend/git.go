package backend

import (
  "fmt"
  "os/exec"
  "strings"
)

type Git struct {
  RepositoryPath string
  gitPath        string
}

func NewGit(repositoryPath string, gitPath string) *Git {
  return &Git{RepositoryPath: repositoryPath, gitPath: gitPath}
}

func (t *Git) execCommand(args ...string) (string, error) {
  cmd := exec.Command(t.gitPath, args...)
  cmd.Dir = t.RepositoryPath
  output, err := cmd.CombinedOutput()
  if err != nil {
    return "", fmt.Errorf("git command failed: %v: %s", err, output)
  }
  return strings.TrimSpace(string(output)), nil
}

func (t *Git) execCommandNoOutput(args ...string) error {
  cmd := exec.Command(t.gitPath, args...)
  cmd.Dir = t.RepositoryPath
  err := cmd.Run()
  if err != nil {
    return fmt.Errorf("git command failed: %v", err)
  }
  return nil
}

func (t *Git) AddNote(message string, commitHash string) error {
  return t.execCommandNoOutput("notes", "add", "-f", "-m", message, commitHash)
}

//func (g *Git) GetNote(commitHash string) (string, error) {
//  return g.execCommand("notes", "show", commitHash)
//}

//func (g *Git) HasRemote(name string) (bool, error) {
//  output, err := g.execCommand("remote")
//  if err != nil {
//    return false, err
//  }
//  remotes := strings.Split(output, "\n")
//  for _, remote := range remotes {
//    if remote == name {
//      return true, nil
//    }
//  }
//  return false, nil
//}
