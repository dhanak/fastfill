# CI-only: package the release binary, then test the extracted standalone app.
$ErrorActionPreference = "Stop"
$rustRoot = Split-Path $PSScriptRoot -Parent
$repoRoot = Split-Path $rustRoot -Parent
$source = Join-Path $env:CARGO_TARGET_DIR `
    "x86_64-pc-windows-msvc/release/fastfill-rust.exe"
$packageDir = Join-Path $env:RUNNER_TEMP "FastFill-Rust"
$archive = Join-Path $env:RUNNER_TEMP "FastFill-Rust-windows-x64.zip"
New-Item -ItemType Directory -Path $packageDir | Out-Null
Copy-Item -LiteralPath $source `
    -Destination (Join-Path $packageDir "FastFill-Rust.exe")
Copy-Item -LiteralPath (Join-Path $rustRoot "PORTABLE.txt") `
    -Destination (Join-Path $packageDir "README.txt")
Copy-Item -LiteralPath (Join-Path $repoRoot "LICENSE") `
    -Destination (Join-Path $packageDir "LICENSE.txt")
Copy-Item -LiteralPath (Join-Path $repoRoot `
    "docs/samples/fastfill-camera-sample-form.pdf") `
    -Destination (Join-Path $packageDir "sample-form.pdf")
@(
    "Commit: $env:GITHUB_SHA"
    "Build: https://github.com/$env:GITHUB_REPOSITORY/actions/runs/" +
        $env:GITHUB_RUN_ID
) | Set-Content -LiteralPath (Join-Path $packageDir "BUILD.txt")
Compress-Archive -LiteralPath $packageDir -DestinationPath $archive

# Extract elsewhere: never rely on files beside the Cargo build output.
$smokeDir = Join-Path $env:RUNNER_TEMP "FastFill-Rust-smoke"
Expand-Archive -LiteralPath $archive -DestinationPath $smokeDir
$extracted = Join-Path $smokeDir "FastFill-Rust"
foreach ($file in @("FastFill-Rust.exe", "README.txt", "LICENSE.txt",
    "sample-form.pdf", "BUILD.txt")) {
    $path = Join-Path $extracted $file
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Portable ZIP is missing $file."
    }
}
$executable = Join-Path $extracted "FastFill-Rust.exe"

# Static CRT is intentional: no separate Visual C++ runtime installation.
$vswhere = Join-Path ${env:ProgramFiles(x86)} `
    "Microsoft Visual Studio/Installer/vswhere.exe"
$dumpbin = & $vswhere -latest -products '*' `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' |
    Select-Object -Last 1
if (-not $dumpbin) { throw "MSVC dumpbin was not found." }
$imports = & $dumpbin /nologo /dependents $executable
if ($LASTEXITCODE -ne 0) { throw "Could not inspect portable dependencies." }
$imports | Write-Output
if ($imports -match '(?i)\b(vcruntime|msvcp|concrt|vccorlib)\d.*\.dll\b') {
    throw "Portable executable still requires a Visual C++ runtime DLL."
}

# A real executable launch, but invalid arguments prevent any camera access.
$startInfo = [Diagnostics.ProcessStartInfo]::new($executable)
$startInfo.WorkingDirectory = $extracted
$startInfo.UseShellExecute = $false
$startInfo.RedirectStandardError = $true
$startInfo.ArgumentList.Add("--fastfill-camera-query")
$startInfo.ArgumentList.Add("invalid")
$process = [Diagnostics.Process]::Start($startInfo)
try {
    if (-not $process.WaitForExit(10000)) {
        $process.Kill($true)
        throw "Extracted portable executable did not exit."
    }
    $stderr = $process.StandardError.ReadToEnd()
    if ($process.ExitCode -ne 1 -or
        $stderr -notmatch 'Camera worker: Expected list or modes INDEX') {
        throw "Portable launch failed ($($process.ExitCode)): $stderr"
    }
}
finally {
    $process.Dispose()
}
Write-Output "Portable extraction, dependency and launch checks passed."
