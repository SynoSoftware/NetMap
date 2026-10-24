#Requires -Version 7.0
[CmdletBinding()]
param([ValidateSet('Debug', 'Release')][string] $Configuration = 'Debug')

. (Join-Path $PSScriptRoot 'common.ps1')
$arguments = @('build', '--locked', '--target', $BuildTarget)
if ($Configuration -eq 'Release') { $arguments += '--release' }
Invoke-Cargo -Arguments $arguments
$project = Get-Project
$directory = Join-Path $project.TargetDirectory "$BuildTarget/$($Configuration.ToLowerInvariant())"
Copy-ProjectAssets -Destination (Join-Path $directory 'assets') -AllowMissing
Join-Path $directory "$($project.Name).exe"
