#Requires -Version 7.0
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$BuildTarget = 'x86_64-pc-windows-msvc'
$RuntimeAssets = @('world.bin', 'country.mmdb', 'Attribution.md')

function Copy-ProjectAssets {
    param([Parameter(Mandatory)][string] $Destination, [switch] $AllowMissing)

    foreach ($asset in $RuntimeAssets) {
        $source = Join-Path $ProjectRoot "assets/$asset"
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            if ($AllowMissing) { continue }
            throw "Missing assets/$asset. Prepare the documented distribution assets."
        }
        New-Item -ItemType Directory -Path $Destination -Force | Out-Null
        Copy-Item -LiteralPath $source -Destination $Destination
    }
}

function Invoke-Cargo {
    param([Parameter(Mandatory)][string[]] $Arguments)

    Push-Location -LiteralPath $ProjectRoot
    try {
        & cargo @Arguments | Out-Host
        if ($LASTEXITCODE -ne 0) {
            throw "cargo $($Arguments -join ' ') failed (exit $LASTEXITCODE)."
        }
    }
    finally {
        Pop-Location
    }
}

function Get-Project {
    Push-Location -LiteralPath $ProjectRoot
    try {
        $json = & cargo metadata --format-version 1 --no-deps --locked
        if ($LASTEXITCODE -ne 0) { throw 'Could not read Cargo project metadata.' }
        $metadata = ($json -join "`n") | ConvertFrom-Json
        $package = $metadata.packages | Where-Object { $_.manifest_path -eq (Join-Path $ProjectRoot 'Cargo.toml') }
        if (-not $package) { throw 'Cargo metadata did not identify the NetMap package.' }
        [pscustomobject]@{
            Name = $package.name
            Version = $package.version
            FileVersion = (($package.version -split '[-+]')[0] + '.0')
            TargetDirectory = $metadata.target_directory
        }
    }
    finally {
        Pop-Location
    }
}
