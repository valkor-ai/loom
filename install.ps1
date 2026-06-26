param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("codex", "claude-code", "opencode", "all")]
  [string]$agent,
  [string]$version = "0.1.0",
  [string]$baseUrl = "https://github.com/valkor-ai/loom/releases/latest/download",
  [switch]$localBuild,
  [string]$repoRoot = (Get-Location).Path
)

$ErrorActionPreference = "Stop"

function Invoke-CheckedCommand {
  param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
  )
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE"
  }
}

function Get-LoomPlatform {
  $arch = if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") {
    "arm64"
  } else {
    "x64"
  }
  if ($arch -ne "x64") {
    throw "Windows package is currently published for x64 only."
  }
  "windows-x64"
}

$platform = Get-LoomPlatform
$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("loom-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temp | Out-Null

try {
  if ($localBuild) {
    $manifest = Join-Path $repoRoot "src\rust\Cargo.toml"
    if (-not (Test-Path $manifest)) {
      throw "-localBuild requires a Loom repository root with src\rust\Cargo.toml: $repoRoot"
    }
    Get-Command cargo | Out-Null

    Invoke-CheckedCommand cargo build --release -p mcp-server -p setup --manifest-path $manifest
    $setup = Join-Path $repoRoot "src\rust\target\release\loom-setup.exe"
    $packageOutput = Join-Path $temp "packages"
    New-Item -ItemType Directory -Path $packageOutput | Out-Null
    Invoke-CheckedCommand $setup package-layout --output-dir $packageOutput --platform $platform
    $packageRoot = Get-ChildItem -Path $packageOutput -Directory | Where-Object { $_.Name -like "loom-*" } | Sort-Object Name | Select-Object -First 1
  } else {
    $package = "loom-$version-$platform.zip"
    $archive = Join-Path $temp $package
    Invoke-WebRequest -Uri "$baseUrl/$package" -OutFile $archive
    Expand-Archive -Path $archive -DestinationPath $temp -Force
    $packageRoot = Get-ChildItem -Path $temp -Directory | Where-Object { $_.Name -like "loom-*" } | Sort-Object Name | Select-Object -First 1
  }

  if (-not $packageRoot) {
    throw "Package did not contain a loom-* directory."
  }
  Invoke-CheckedCommand (Join-Path $packageRoot.FullName "bin\loom-setup.exe") install --agent $agent --package-root $packageRoot.FullName
} finally {
  Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
}
