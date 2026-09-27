param(
    [string]$IdnaDataPath = "data/g40-idna17/Idna2008.txt",
    [string]$OutputPath = "src/smtp/idna17/ranges.rs",
    [int]$RangesPerShard = 250
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# Закреплённый digest исключает незаметную подмену Unicode IDNA таблицы.
$expectedSha256 = "E4A7526A8A37539C0DEFA4DA25F5DBF77D0212A14D4762D455CADEA608A8921C"
$actualSha256 = (Get-FileHash -LiteralPath $IdnaDataPath -Algorithm SHA256).Hash
if ($actualSha256 -ne $expectedSha256) {
    throw "Idna2008.txt не совпадает с официальной версией Unicode 17.0.0"
}

$records = [System.Collections.Generic.List[object]]::new()
$pattern = [System.Text.RegularExpressions.Regex]::new(
    '^\s*(?<first>[0-9A-F]{4,6})(?:\.\.(?<last>[0-9A-F]{4,6}))?\s*;\s*(?<property>PVALID|CONTEXTJ|CONTEXTO)\b'
)
foreach ($line in [System.IO.File]::ReadLines($IdnaDataPath)) {
    $match = $pattern.Match($line)
    if (-not $match.Success) { continue }
    $first = [Convert]::ToInt32($match.Groups["first"].Value, 16)
    $lastText = $match.Groups["last"].Value
    $last = switch ([string]::IsNullOrEmpty($lastText)) {
        $true { $first }
        $false { [Convert]::ToInt32($lastText, 16) }
    }
    $property = switch ($match.Groups["property"].Value) {
        "PVALID" { "PValid" }
        "CONTEXTJ" { "ContextJ" }
        "CONTEXTO" { "ContextO" }
        default { throw "Неизвестное derived-property значение" }
    }
    $records.Add([System.ValueTuple]::Create($first, $last, $property))
}

$previousEnd = -1
foreach ($record in $records) {
    if ($record.Item1 -le $previousEnd) { throw "IDNA ranges are not sorted and disjoint" }
    $previousEnd = $record.Item2
}

$directory = Split-Path -Parent $OutputPath
$shardDirectory = Join-Path $directory "ranges"
New-Item -ItemType Directory -Path $shardDirectory -Force | Out-Null
$shardPaths = [System.Collections.Generic.List[string]]::new()
for ($offset = 0; $offset -lt $records.Count; $offset += $RangesPerShard) {
    $count = [Math]::Min($RangesPerShard, $records.Count - $offset)
    $chunk = $records.GetRange($offset, $count)
    $shardName = "ranges_{0:D2}.rs" -f $shardPaths.Count
    $shardPath = Join-Path $shardDirectory $shardName
    $shardPaths.Add($shardPath)
    $shardLines = [System.Collections.Generic.List[string]]::new()
    $shardLines.Add("//! IDNA2008 derived-property ranges, часть Unicode 17.0.0.")
    $shardLines.Add("// Copyright © 2025 Unicode, Inc.; лицензия приведена в docs/unicode-data.md.")
    $shardLines.Add("use super::super::DerivedProperty;")
    $shardLines.Add("")
    $shardLines.Add("pub(super) const RANGES: &[(u32, u32, DerivedProperty)] = &[")
    foreach ($record in $chunk) {
        $shardLines.Add(("    (0x{0:X06}, 0x{1:X06}, DerivedProperty::{2})," -f $record.Item1, $record.Item2, $record.Item3))
    }
    $shardLines.Add("];")
    if ($shardLines.Count -gt 300) { throw "$shardName превысил лимит 300 строк" }
    [System.IO.File]::WriteAllLines($shardPath, $shardLines, [System.Text.UTF8Encoding]::new($false))
    & rustfmt --edition 2024 $shardPath
    if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for $shardPath" }
}

$indexLines = [System.Collections.Generic.List[string]]::new()
$indexLines.Add("//! Индекс небольших таблиц IDNA2008 derived-property ranges.")
foreach ($shardPath in $shardPaths) {
    $shardName = [System.IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("#[path = `"ranges/$shardName.rs`"]")
    $indexLines.Add("mod $shardName;")
}
$indexLines.Add("")
$indexLines.Add("pub(super) const TABLES: &[&[(u32, u32, super::DerivedProperty)]] = &[")
foreach ($shardPath in $shardPaths) {
    $shardName = [System.IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("    $shardName`::RANGES,")
}
$indexLines.Add("];")
if ($indexLines.Count -gt 300) { throw "IDNA table index exceeds the 300-line source limit" }
[System.IO.File]::WriteAllLines($OutputPath, $indexLines, [System.Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 $OutputPath
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for the IDNA table index" }

Write-Output ("Generated {0} ranges in {1} shards from pinned Unicode 17.0.0 source." -f $records.Count, $shardPaths.Count)
