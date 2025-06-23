package backend

import (
  "bufio"
  "fmt"
  "io"
  "os/exec"
  "strings"
  "time"
)

// Variable to allow mocking of exec.Command in tests
var execCommand = exec.Command

type Commit struct {
  Hash    string
  Author  string
  Email   string
  Date    time.Time
  Message string
  Notes   string
}

// GetCommitList retrieves a list of commits that are in HEAD but not in origin/master
// in reverse chronological order (oldest first)
func GetCommitList(dir string, branchName string) ([]Commit, error) {
  cmd := execCommand("git", "log", "--reverse", "--notes", "origin/"+branchName+"..HEAD")
  cmd.Dir = dir

  stdout, err := cmd.StdoutPipe()
  if err != nil {
    return nil, err
  }

  if err := cmd.Start(); err != nil {
    return nil, err
  }

  commits, err := parseGitLogOutput(stdout)
  if err != nil {
    return nil, err
  }

  if err := cmd.Wait(); err != nil {
    return nil, err
  }

  return commits, nil
}

// parseGitLogOutput parses the output of git log command
func parseGitLogOutput(reader io.Reader) ([]Commit, error) {
  var commits []Commit
  scanner := bufio.NewScanner(reader)

  var currentCommit *Commit
  var collectingMessage bool
  var collectingNotes bool

  for scanner.Scan() {
    line := scanner.Text()

    if strings.HasPrefix(line, "commit ") {
      // save the previous commit if there is one
      if currentCommit != nil {
        // trim any trailing whitespace from the message and notes
        currentCommit.Message = strings.TrimSpace(currentCommit.Message)
        currentCommit.Notes = strings.TrimSpace(currentCommit.Notes)
        commits = append(commits, *currentCommit)
      }

      // start a new commit
      hash := strings.TrimPrefix(line, "commit ")
      currentCommit = &Commit{Hash: hash}
      collectingMessage = false
      collectingNotes = false
    } else if strings.HasPrefix(line, "Author: ") {
      if currentCommit == nil {
        return nil, fmt.Errorf("malformed git log: author line before commit hash")
      }

      authorInfo := strings.TrimPrefix(line, "Author: ")
      parts := strings.Split(authorInfo, " <")
      if len(parts) == 2 {
        currentCommit.Author = parts[0]
        currentCommit.Email = strings.TrimSuffix(parts[1], ">")
      }
    } else if strings.HasPrefix(line, "Date: ") {
      if currentCommit == nil {
        return nil, fmt.Errorf("malformed git log: author line before commit hash")
      }

      dateStr := strings.TrimPrefix(line, "Date:   ")
      // Parse the date format from git
      // Example: "Mon Jan 2 15:04:05 2006 -0700"
      date, err := time.Parse("Mon Jan 2 15:04:05 2006 -0700", dateStr)
      if err == nil {
        currentCommit.Date = date
      }
    } else if strings.HasPrefix(line, "Notes:") {
      if currentCommit == nil {
        return nil, fmt.Errorf("malformed git log: notes line before commit hash")
      }
      collectingMessage = false
      collectingNotes = true
    } else if collectingNotes {
      // Append to the commit notes
      if currentCommit.Notes != "" {
        currentCommit.Notes += "\n"
      }
      currentCommit.Notes += line
    } else if collectingMessage {
      // Append to the commit message
      if currentCommit.Message != "" {
        currentCommit.Message += "\n"
      }
      currentCommit.Message += line
    } else if line == "" && currentCommit != nil && currentCommit.Date.Year() > 0 {
      // Empty line after the date means next line will be the start of the commit message
      collectingMessage = true
      collectingNotes = false
    }
  }

  // add the last commit if there is one
  if currentCommit != nil {
    currentCommit.Message = strings.TrimSpace(currentCommit.Message)
    currentCommit.Notes = strings.TrimSpace(currentCommit.Notes)
    commits = append(commits, *currentCommit)
  }

  if err := scanner.Err(); err != nil {
    return nil, fmt.Errorf("error scanning git log output: %w", err)
  }

  return commits, nil
}
