#Requires -Version 7.0
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$assets = Join-Path (Split-Path -Parent $PSScriptRoot) 'assets'
$country = Get-Content -LiteralPath (Join-Path $assets 'country.json') -Raw | ConvertFrom-Json
$destination = Join-Path $assets 'country.mmdb'
if (Test-Path -LiteralPath $destination) {
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $country.sha256) {
        throw 'Existing country.mmdb differs from country.json. Preserve or remove it before preparing the pinned asset.'
    }
    Write-Output "Verified country.mmdb ($($country.month))."
    return
}

$generation = [guid]::NewGuid().ToString('N')
$archive = Join-Path $assets "country-$generation.gz"
$candidate = Join-Path $assets "country-$generation.tmp"
try {
    Invoke-WebRequest -Uri $country.url -OutFile $archive -MaximumRedirection 0 -ConnectionTimeoutSeconds 30 -OperationTimeoutSeconds 180
    $inputFile = [IO.File]::OpenRead($archive)
    try {
        if ($inputFile.Length -gt 32MB) { throw 'Country archive exceeds 32 MiB.' }
        $gzip = [IO.Compression.GZipStream]::new($inputFile, [IO.Compression.CompressionMode]::Decompress, $true)
        try {
            $outputFile = [IO.File]::Open($candidate, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            try {
                $buffer = [byte[]]::new(65536)
                $total = 0L
                while (($count = $gzip.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $total += $count
                    if ($total -gt $country.bytes) { throw 'Country database exceeds its pinned length.' }
                    $outputFile.Write($buffer, 0, $count)
                }
                if ($total -ne $country.bytes) { throw 'Country database length differs from country.json.' }
                $outputFile.Flush($true)
            }
            finally { $outputFile.Dispose() }
        }
        finally { $gzip.Dispose() }
    }
    finally { $inputFile.Dispose() }
    if ((Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -ne $country.sha256) {
        throw 'Country database checksum differs from country.json.'
    }
    [IO.File]::Move($candidate, $destination)
    Write-Output "Prepared country.mmdb ($($country.month))."
}
finally {
    foreach ($temporary in @($archive, $candidate)) {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
    }
}
