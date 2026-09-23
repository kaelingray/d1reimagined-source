param(
    [Parameter(Mandatory = $true)]
    [string]$SourceDirectory,

    [Parameter(Mandatory = $true)]
    [string]$OutputCsv
)

$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $SourceDirectory).Path.TrimEnd('\', '/')
$output = [System.IO.Path]::GetFullPath($OutputCsv)

if (-not (Test-Path -LiteralPath $source -PathType Container)) {
    throw "SourceDirectory must be an existing directory: $source"
}
if ($output.StartsWith($source + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Place the manifest outside the source directory so it cannot hash itself.'
}
if (Test-Path -LiteralPath $output) {
    throw "Output already exists; choose a new filename: $output"
}

$rows = Get-ChildItem -LiteralPath $source -Recurse -File -Force |
    Sort-Object FullName |
    ForEach-Object {
        $relative = $_.FullName.Substring($source.Length + 1)
        [pscustomobject]@{
            RelativePath = $relative
            Bytes = $_.Length
            SHA256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    }

$parent = Split-Path -Parent $output
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$rows | Export-Csv -LiteralPath $output -NoTypeInformation -Encoding UTF8
Write-Output "Hashed $($rows.Count) files into $output"
