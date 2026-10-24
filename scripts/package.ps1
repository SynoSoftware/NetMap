#Requires -Version 7.0
[CmdletBinding()]
param([switch] $Scaffold)

. (Join-Path $PSScriptRoot 'common.ps1')
if (-not $Scaffold) {
    foreach ($asset in $RuntimeAssets) {
        if (-not (Test-Path -LiteralPath (Join-Path $ProjectRoot "assets/$asset") -PathType Leaf)) {
            throw "Missing assets/$asset. Prepare the documented distribution assets, or use -Scaffold for a development-only package."
        }
    }
}

$executable = & (Join-Path $PSScriptRoot 'build.ps1') -Configuration Release
$project = Get-Project
$suffix = if ($Scaffold) { '-scaffold' } else { '' }
$name = "NetMap-$($project.Version)-$BuildTarget$suffix"
$generation = [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
$output = Join-Path $ProjectRoot "artifacts/packages/$generation"
$payload = Join-Path $output $name
New-Item -ItemType Directory -Path $payload | Out-Null
Copy-Item -LiteralPath $executable -Destination (Join-Path $payload 'netmap.exe')
Copy-Item -LiteralPath $executable -Destination (Join-Path $payload 'netmap.scr')
Copy-Item -LiteralPath (Join-Path $ProjectRoot 'README.md') -Destination $payload
Copy-Item -LiteralPath (Join-Path $ProjectRoot 'installer/notices') -Destination $payload -Recurse

Copy-ProjectAssets -Destination (Join-Path $payload 'assets') -AllowMissing:$Scaffold
$archive = Join-Path $output "$name.zip"
Compress-Archive -LiteralPath $payload -DestinationPath $archive
[pscustomobject]@{
    Payload = $payload
    Archive = $archive
    OutputDirectory = $output
    Version = $project.Version
    FileVersion = $project.FileVersion
    Name = $name
}
