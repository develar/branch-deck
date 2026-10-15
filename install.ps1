# Installs or upgrades Branch Deck on Windows from the GitHub release:
#   irm https://raw.githubusercontent.com/develar/branch-deck/master/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/develar/branch-deck/master/install.ps1))) -WithCli
#
# A download by PowerShell carries no Mark of the Web, so SmartScreen doesn't stop the unsigned installer, as for the
# app's own updates. The installer URL comes from the release's latest.json, the file the updater reads. The NSIS
# installer runs silently and per user (no administrator rights).
#
# -Version <x.y.z>  install this release instead of the latest one
# -WithCli          also install the Agent CLI: bd.exe on the user PATH and the agent skills (`bd install`)
# -Manifest <url>   the URL of latest.json instead of the release's (tests)

param(
  [string]$Version = "",
  [switch]$WithCli,
  [string]$Manifest = ""
)

[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = "develar/branch-deck"
# the Agent CLI (bd.exe next to the app) is in the release since this version
$CliSince = [version]"0.12.0"

function Fail([string]$Message) {
  # throw, not exit: `irm | iex` runs in the user's own session
  throw "Cannot install Branch Deck: $Message"
}

function Install-BranchDeck {
  # function scope: `irm | iex` runs in the user's session, whose preferences stay as they are
  $ErrorActionPreference = "Stop"
  # the progress bar makes Invoke-WebRequest many times slower in Windows PowerShell
  $ProgressPreference = "SilentlyContinue"

  if ($env:OS -ne "Windows_NT") {
    Fail "use install.sh on macOS and Linux"
  }
  # Windows on ARM runs the x64 build emulated
  if (-not [Environment]::Is64BitOperatingSystem) {
    Fail "Branch Deck for Windows is built for 64-bit Windows only"
  }

  $ManifestUrl = if ($Manifest) {
    $Manifest
  } elseif ($Version) {
    "https://github.com/$Repo/releases/download/v$($Version.TrimStart('v'))/latest.json"
  } else {
    "https://github.com/$Repo/releases/latest/download/latest.json"
  }
  $Temp = Join-Path ([IO.Path]::GetTempPath()) "branch-deck-install-$([Guid]::NewGuid().ToString('N'))"
  New-Item -ItemType Directory -Path $Temp | Out-Null
  try {
    Install-Release $ManifestUrl $Temp
  } finally {
    Remove-Item -Recurse -Force $Temp -ErrorAction SilentlyContinue
  }
}

function Install-Release([string]$ManifestUrl, [string]$Temp) {
  # GitHub serves latest.json as application/octet-stream, which Invoke-RestMethod doesn't parse
  $ManifestFile = Join-Path $Temp "latest.json"
  try {
    Invoke-WebRequest -Uri $ManifestUrl -OutFile $ManifestFile -UseBasicParsing
  } catch {
    Fail "no release manifest at ${ManifestUrl}: $($_.Exception.Message)"
  }
  $Release = Get-Content -Raw -Encoding UTF8 $ManifestFile | ConvertFrom-Json
  $Platform = $Release.platforms."windows-x86_64-nsis"
  if (-not $Platform) {
    Fail "release $($Release.version) has no windows-x86_64-nsis build"
  }

  $Setup = Join-Path $Temp ($Platform.url -split "/")[-1]
  Write-Host "Downloading $(Split-Path $Setup -Leaf)"
  try {
    Invoke-WebRequest -Uri $Platform.url -OutFile $Setup -UseBasicParsing
  } catch {
    Fail "download of $($Platform.url) failed: $($_.Exception.Message)"
  }
  # PowerShell writes no Zone.Identifier, but a proxy or a management tool can
  Unblock-File -Path $Setup

  # an upgrade stops the running app and starts it again afterwards; the installer would ask to close it
  $Running = @(Get-Process -Name "branch-deck" -ErrorAction SilentlyContinue)
  foreach ($Process in $Running) {
    $Process.CloseMainWindow() | Out-Null
    if (-not $Process.WaitForExit(10000)) {
      Stop-Process -Id $Process.Id -Force
    }
  }

  $Installer = Start-Process -FilePath $Setup -ArgumentList "/S" -Wait -PassThru
  if ($Installer.ExitCode -ne 0) {
    Fail "the installer exited with code $($Installer.ExitCode)"
  }

  $Uninstall = Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\BranchDeck" -ErrorAction SilentlyContinue
  $Dir = if ($Uninstall -and $Uninstall.InstallLocation) { $Uninstall.InstallLocation.Trim('"') } else { Join-Path $env:LOCALAPPDATA "BranchDeck" }
  $App = Join-Path $Dir "branch-deck.exe"
  if (-not (Test-Path $App)) {
    Fail "the installer put no branch-deck.exe into $Dir"
  }
  $HasCli = [version]$Release.version -ge $CliSince
  $Bd = Join-Path $Dir "bd.exe"
  if ($HasCli) {
    # also the proof that the installed binary starts
    & $Bd version | Out-Null
    if ($LASTEXITCODE -ne 0) {
      Fail "the installed $Bd does not start"
    }
  }
  Write-Host "Installed Branch Deck $($Release.version) to $Dir"
  if ($Running.Count -gt 0) {
    Start-Process -FilePath $App
  }

  $Git = Get-Command git -ErrorAction SilentlyContinue
  $GitVersion = if ($Git) { (& git --version) -replace '^git version (\d+\.\d+\.\d+).*', '$1' }
  if (-not $GitVersion) {
    Write-Host "Git is not installed: Branch Deck requires Git 2.54 or newer. Install Git for Windows:"
    Write-Host "  winget install --id Git.Git -e"
  } elseif ([version]$GitVersion -lt [version]"2.54.0") {
    Write-Host "Git $GitVersion is too old: Branch Deck requires Git 2.54 or newer. Upgrade it with"
    Write-Host "  winget upgrade --id Git.Git -e"
  }

  if (-not $HasCli) {
    if ($WithCli) {
      Write-Host "Branch Deck $($Release.version) has no Agent CLI yet: it comes with $CliSince"
    }
  } elseif ($WithCli) {
    $Result = (& $Bd install | Out-String) | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) {
      Fail "the Agent CLI could not be installed"
    }
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $Entries = @($UserPath -split ";" | Where-Object { $_ })
    if ($Entries -notcontains $Dir) {
      [Environment]::SetEnvironmentVariable("Path", (($Entries + $Dir) -join ";"), "User")
      $env:Path = "$env:Path;$Dir"
      Write-Host "Added $Dir to your PATH: open a new terminal to run bd"
    }
    Write-Host "Installed the Agent CLI as $($Result.bd), and the branch-deck skill for coding agents"
  } else {
    Write-Host "Agent CLI for coding agents: run the installer with -WithCli, or Help > Install Agent CLI... in the app"
  }
}

Install-BranchDeck
