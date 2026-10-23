#Requires -Version 7.0
[CmdletBinding()]
param()

. (Join-Path $PSScriptRoot 'common.ps1')
Invoke-Cargo -Arguments @('fmt', '--all', '--check')
Invoke-Cargo -Arguments @('clippy', '--locked', '--all-targets', '--target', $BuildTarget, '--', '-D', 'warnings')
Invoke-Cargo -Arguments @('test', '--locked', '--all-targets', '--target', $BuildTarget)
