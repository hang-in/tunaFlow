$ErrorActionPreference = "Stop"

$RootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$TargetTriple = (& rustc --print host-tuple).Trim()
$DestDir = Join-Path $RootDir "src-tauri\binaries"
$DestPath = Join-Path $DestDir "rawq-$TargetTriple.exe"
$TargetDir = Join-Path $RootDir "src-tauri\target\rawq-sidecar"

$RawqRepoUrl = if ($env:RAWQ_REPO_URL) { $env:RAWQ_REPO_URL } else { "https://github.com/hang-in/rawq" }

$Candidates = @()
if ($env:RAWQ_SRC) {
  $Candidates += $env:RAWQ_SRC
} else {
  $Candidates += (Join-Path $RootDir "vendor\rawq")
  $Candidates += (Join-Path $RootDir "..\tunaDish\vendor\rawq")
  $Candidates += (Join-Path $RootDir "..\_research\_util\rawq")
}

$RawqSrcDir = $null
foreach ($candidate in $Candidates) {
  if (Test-Path (Join-Path $candidate "Cargo.toml")) {
    $RawqSrcDir = (Resolve-Path $candidate).Path
    break
  }
}

# Auto-clone fallback (last resort): if no local rawq source was found and
# RAWQ_SRC env was not explicitly set, clone the upstream repo into vendor\rawq.
# When RAWQ_SRC is set but invalid, do NOT auto-clone — surface the error so
# the user can fix their override path.
if (-not $RawqSrcDir -and -not $env:RAWQ_SRC) {
  $AutoCloneDir = Join-Path $RootDir "vendor\rawq"
  if (Test-Path (Join-Path $AutoCloneDir "Cargo.toml")) {
    Write-Host "[rawq] using existing auto-cloned vendor at $AutoCloneDir"
    $RawqSrcDir = (Resolve-Path $AutoCloneDir).Path
  } else {
    Write-Host "[rawq] source not found locally - auto cloning $RawqRepoUrl -> $AutoCloneDir"
    New-Item -ItemType Directory -Force -Path (Join-Path $RootDir "vendor") | Out-Null
    & git clone --depth 1 $RawqRepoUrl $AutoCloneDir
    if ($LASTEXITCODE -ne 0) {
      Write-Error ("[rawq] auto clone failed. Set RAWQ_SRC=<path> or RAWQ_REPO_URL=<fork-url>.`n  searched candidates:`n  " + ($Candidates -join "`n  "))
    }
    $RawqSrcDir = (Resolve-Path $AutoCloneDir).Path
  }
}

if (-not $RawqSrcDir) {
  Write-Error ("rawq source not found. Set RAWQ_SRC or place rawq at one of:`n  " + ($Candidates -join "`n  "))
}

Write-Host "[rawq] source: $RawqSrcDir"
Write-Host "[rawq] target: $TargetTriple"

New-Item -ItemType Directory -Force -Path $DestDir | Out-Null

cargo build --manifest-path (Join-Path $RawqSrcDir "Cargo.toml") --release --target-dir $TargetDir

Copy-Item (Join-Path $TargetDir "release\rawq.exe") $DestPath -Force

Write-Host "[rawq] installed sidecar: $DestPath"
