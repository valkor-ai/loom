param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("codex", "claude-code", "opencode", "all")]
  [string]$agent,
  [string]$version = "0.1.0",
  [string]$baseUrl = "https://github.com/valkor-ai/loom/releases/latest/download"
)

$ErrorActionPreference = "Stop"

$arch = if ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture -eq "Arm64") {
  "arm64"
} else {
  "x64"
}
if ($arch -ne "x64") {
  throw "Windows package is currently published for x64 only."
}

$package = "loom-$version-windows-x64.zip"
$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("loom-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temp | Out-Null

try {
  $archive = Join-Path $temp $package
  Invoke-WebRequest -Uri "$baseUrl/$package" -OutFile $archive
  Expand-Archive -Path $archive -DestinationPath $temp -Force
  $packageRoot = Get-ChildItem -Path $temp -Directory | Where-Object { $_.Name -like "loom-*" } | Select-Object -First 1
  if (-not $packageRoot) {
    throw "Release package did not contain a loom-* directory."
  }
  & (Join-Path $packageRoot.FullName "bin\loom-setup.exe") install --agent $agent --package-root $packageRoot.FullName
} finally {
  Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
}
