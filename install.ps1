[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [Alias("agent")]
  [ValidateSet("codex", "claude-code", "opencode", "all")]
  [string]$Agent,

  [Alias("version")]
  [string]$Version = "0.2.8",

  [Alias("baseUrl")]
  [string]$BaseUrl = "https://github.com/valkor-ai/loom/releases/latest/download",

  [Alias("localBuild")]
  [switch]$LocalBuild,

  [Alias("repoRoot")]
  [string]$RepoRoot = (Get-Location).Path,

  [Alias("printPlan")]
  [switch]$PrintPlan
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
  $testOs = $env:LOOM_INSTALL_TEST_OS
  $testArch = $env:LOOM_INSTALL_TEST_ARCH
  if ($testOs) {
    if ($testOs.ToLowerInvariant() -notin @("windows", "win32", "win")) {
      throw "install.ps1 supports Windows release packages only. Use install.sh on macOS or Linux."
    }
    $archName = $testArch
  } else {
    if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
      throw "install.ps1 supports Windows release packages only. Use install.sh on macOS or Linux."
    }
    $archName = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
  }

  switch ($archName.ToLowerInvariant()) {
    { $_ -in @("x64", "amd64", "x86_64") } { return "windows-x64" }
    "arm64" { throw "Windows ARM64 package is not published yet." }
    default { throw "Unsupported Windows architecture: $archName" }
  }
}

function Test-ArchiveChecksum {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Archive,
    [Parameter(Mandatory = $true)]
    [string]$ChecksumFile
  )
  $content = Get-Content -Raw -Path $ChecksumFile
  $match = [regex]::Match($content, "(?i)[a-f0-9]{64}")
  if (-not $match.Success) {
    throw "Checksum file did not contain a SHA-256 digest: $ChecksumFile"
  }
  $expected = $match.Value.ToLowerInvariant()
  $actual = (Get-FileHash -Algorithm SHA256 -Path $Archive).Hash.ToLowerInvariant()
  if ($expected -ne $actual) {
    throw "Archive checksum mismatch for $(Split-Path -Leaf $Archive): expected $expected, got $actual"
  }
}

$platform = Get-LoomPlatform
$package = "loom-$Version-$platform.zip"
if (($PSBoundParameters.ContainsKey("Version") -or $env:LOOM_INSTALL_USE_VERSIONED_URL -eq "1") -and -not $PSBoundParameters.ContainsKey("BaseUrl")) {
  $BaseUrl = "https://github.com/valkor-ai/loom/releases/download/v$Version"
}
$packageUrl = "$BaseUrl/$package"
$checksumUrl = "$packageUrl.sha256"

if ($PrintPlan) {
  [pscustomobject]@{
    agent = $Agent
    version = $Version
    platform = $platform
    localBuild = [bool]$LocalBuild
    package = $package
    packageUrl = $packageUrl
    checksumUrl = $checksumUrl
    archiveChecksumRequired = $true
  } | ConvertTo-Json -Compress
  exit 0
}

$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("loom-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temp | Out-Null

try {
  if ($LocalBuild) {
    $manifest = Join-Path $RepoRoot "src\rust\Cargo.toml"
    if (-not (Test-Path $manifest)) {
      throw "-LocalBuild requires a Loom repository root with src\rust\Cargo.toml: $RepoRoot"
    }
    Get-Command cargo | Out-Null

    Invoke-CheckedCommand cargo build --release -p mcp-server -p setup --manifest-path $manifest
    $setup = Join-Path $RepoRoot "src\rust\target\release\loom-setup.exe"
    $packageOutput = Join-Path $temp "packages"
    New-Item -ItemType Directory -Path $packageOutput | Out-Null
    Invoke-CheckedCommand $setup package-layout --output-dir $packageOutput --platform $platform
    $packageRoot = Get-ChildItem -Path $packageOutput -Directory | Where-Object { $_.Name -like "loom-*" } | Sort-Object Name | Select-Object -First 1
  } else {
    $archive = Join-Path $temp $package
    $checksumFile = Join-Path $temp "$package.sha256"
    Invoke-WebRequest -Uri $packageUrl -OutFile $archive
    Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumFile
    Test-ArchiveChecksum -Archive $archive -ChecksumFile $checksumFile
    Expand-Archive -Path $archive -DestinationPath $temp -Force
    $packageRoot = Get-ChildItem -Path $temp -Directory | Where-Object { $_.Name -like "loom-*" } | Sort-Object Name | Select-Object -First 1
  }

  if (-not $packageRoot) {
    throw "Package did not contain a loom-* directory."
  }
  $setup = Join-Path $packageRoot.FullName "bin\loom-setup.exe"
  Invoke-CheckedCommand $setup install --agent $Agent --package-root $packageRoot.FullName
  Invoke-CheckedCommand $setup doctor --agent $Agent --package-root $packageRoot.FullName
} finally {
  Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
}
