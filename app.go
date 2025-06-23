package main

import (
  "context"
  "fmt"
  "virtual-branches/backend"

  "github.com/go-git/go-git/v5"
  "github.com/wailsapp/wails/v2/pkg/runtime"
)

// App struct
type App struct {
  ctx context.Context
}

// NewApp creates a new App application struct
func NewApp() *App {
  return &App{}
}

// OnStartup is called when the app starts
func (a *App) OnStartup(ctx context.Context) {
  a.ctx = ctx
}

//goland:noinspection GrazieInspection
func (a *App) OpenDirectoryDialog() string {
  dialog, _ := runtime.OpenDirectoryDialog(a.ctx, runtime.OpenDialogOptions{})
  return dialog
}

// CreateVirtualBranches processes the repository and creates virtual branches
func (a *App) CreateVirtualBranches(repositoryPath, branchPrefix string) backend.ProcessResult {
  if repositoryPath == "" {
    return backend.ProcessResult{
      Success: false,
      Error:   "Repository path is required",
    }
  }

  if branchPrefix == "" {
    return backend.ProcessResult{
      Success: false,
      Error:   "Branch prefix is required",
    }
  }

  branches, err := backend.CreateBranches(repositoryPath, branchPrefix)
  if err != nil {
    return backend.ProcessResult{
      Success: false,
      Error:   fmt.Sprintf("Failed to create branches: %v", err),
    }
  }

  return backend.ProcessResult{
    Success:  true,
    Message:  fmt.Sprintf("Successfully processed %d branches", len(branches)),
    Branches: branches,
  }
}

// GetRepositoryInfo gets basic info about the repository
func (a *App) GetRepositoryInfo(repositoryPath string) backend.RepositoryInfo {
  repo, err := git.PlainOpen(repositoryPath)
  if err != nil {
    return backend.RepositoryInfo{
      Error: fmt.Sprintf("Failed to open repository: %v", err),
    }
  }

  // Get current branch
  head, err := repo.Head()
  if err != nil {
    return backend.RepositoryInfo{
      Error: fmt.Sprintf("Failed to get HEAD: %v", err),
    }
  }

  // Get remotes
  remotes, err := repo.Remotes()
  if err != nil {
    return backend.RepositoryInfo{
      Error: fmt.Sprintf("Failed to get remotes: %v", err),
    }
  }

  remoteNames := make([]string, len(remotes))
  for i, remote := range remotes {
    remoteNames[i] = remote.Config().Name
  }

  return backend.RepositoryInfo{
    Path:          repositoryPath,
    CurrentBranch: head.Name().Short(),
    Remotes:       remoteNames,
  }
}
