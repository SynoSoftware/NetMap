#Requires -Version 7.0
[CmdletBinding()]
param(
    [switch] $Scaffold,
    [string] $ISCCPath
)

. (Join-Path $PSScriptRoot 'common.ps1')
if (-not $ISCCPath) {
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compiler) { $ISCCPath = $compiler.Source }
    else {
        foreach ($directory in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
            if (-not $directory) { continue }
            $candidate = Join-Path $directory 'Inno Setup 7/ISCC.exe'
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                $ISCCPath = $candidate
                break
            }
        }
    }
}
if (-not $ISCCPath -or -not (Test-Path -LiteralPath $ISCCPath -PathType Leaf)) {
    throw 'Install Inno Setup 7, put ISCC.exe on PATH, or pass -ISCCPath.'
}
$ISCCPath = (Resolve-Path -LiteralPath $ISCCPath).Path
$compilerVersion = & $ISCCPath --version
if ($LASTEXITCODE -ne 0 -or ($compilerVersion -join ' ') -notmatch '\b(\d+)\.\d+') {
    throw 'Could not read the Inno Setup compiler engine version.'
}
if ([int]$Matches[1] -lt 7) {
    throw 'The installer requires Inno Setup 7 or newer.'
}

$package = & (Join-Path $PSScriptRoot 'package.ps1') -Scaffold:$Scaffold
$arguments = @(
    '--quiet-progress',
    "--define=AppVersion=$($package.Version)",
    "--define=AppFileVersion=$($package.FileVersion)",
    "--define=PayloadDir=$($package.Payload)",
    "--output-dir=$($package.OutputDirectory)",
    "--output-filename=$($package.Name)-setup",
    (Join-Path $ProjectRoot 'installer/netmap.iss')
)
& $ISCCPath @arguments | Out-Host
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed (exit $LASTEXITCODE)." }
Join-Path $package.OutputDirectory "$($package.Name)-setup.exe"
