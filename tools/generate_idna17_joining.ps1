param(
    [string]$DataDirectory = "data/g40-idna17",
    [string]$OutputDirectory = "src/smtp/idna17",
    [int]$RangesPerShard = 250
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function assert_disjoint($records, [string]$name) {
    $previousEnd = -1
    foreach ($record in $records) {
        if ($record.Start -le $previousEnd) { throw "$name ranges overlap or are unsorted" }
        $previousEnd = $record.End
    }
}

function compress_points($points) {
    $ordered = @($points | Sort-Object -Unique)
    $ranges = [System.Collections.Generic.List[object]]::new()
    $start = $null
    $end = $null
    foreach ($point in $ordered) {
        if ($null -eq $start) { $start = $point; $end = $point; continue }
        if ($point -eq ($end + 1)) { $end = $point; continue }
        $ranges.Add([pscustomobject]@{ Start = $start; End = $end })
        $start = $point
        $end = $point
    }
    if ($null -ne $start) { $ranges.Add([pscustomobject]@{ Start = $start; End = $end }) }
    return $ranges
}

# Digest-ы фиксируют Unicode 17 joining data и canonical combining classes.
$pinnedHashes = @{
    "DerivedJoiningType.txt" = "F39EBE974825D6736AEE15582250307AA532B2CFAB3CAF3F86BD23FDDC9C5C4D"
    "UnicodeData.txt" = "2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C"
}
foreach ($fileName in $pinnedHashes.Keys) {
    $path = Join-Path $DataDirectory $fileName
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if ($actual -ne $pinnedHashes.Get_Item($fileName)) { throw "$fileName hash differs from pinned Unicode 17.0.0" }
}

$joiningPattern = [System.Text.RegularExpressions.Regex]::new(
    '^\s*(?<start>[0-9A-F]{4,6})(?:\.\.(?<end>[0-9A-F]{4,6}))?\s*;\s*(?<type>[CDLRT])\b'
)
$joiningRecords = [System.Collections.Generic.List[object]]::new()
foreach ($line in [System.IO.File]::ReadLines((Join-Path $DataDirectory "DerivedJoiningType.txt"))) {
    $match = $joiningPattern.Match($line)
    if (-not $match.Success) { continue }
    $start = [Convert]::ToInt32($match.Groups["start"].Value, 16)
    $end = $start
    $endText = $match.Groups["end"].Value
    if (-not [string]::IsNullOrEmpty($endText)) { $end = [Convert]::ToInt32($endText, 16) }
    $joiningRecords.Add([pscustomobject]@{ Start = $start; End = $end; Type = $match.Groups["type"].Value })
}
$orderedJoining = @($joiningRecords | Sort-Object -Property Start)
assert_disjoint $orderedJoining "Joining type"

$joiningRoot = Join-Path $OutputDirectory "joining_ranges"
New-Item -ItemType Directory -Path $joiningRoot -Force | Out-Null
$shardPaths = [System.Collections.Generic.List[string]]::new()
for ($offset = 0; $offset -lt $orderedJoining.Count; $offset += $RangesPerShard) {
    $count = [Math]::Min($RangesPerShard, $orderedJoining.Count - $offset)
    $chunk = @($orderedJoining | Select-Object -Skip $offset -First $count)
    $shardName = "joining_{0:D2}.rs" -f $shardPaths.Count
    $shardPath = Join-Path $joiningRoot $shardName
    $shardPaths.Add($shardPath)
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("//! Фрагмент таблицы Joining_Type Unicode 17.")
    $lines.Add("use super::super::JoiningType;")
    $lines.Add("")
    $lines.Add("pub(super) const RANGES: &[(u32, u32, JoiningType)] = &[")
    foreach ($record in $chunk) {
        $type = switch ($record.Type) {
            "L" { "Left" }
            "R" { "Right" }
            "D" { "Dual" }
            "C" { "JoinCausing" }
            "T" { "Transparent" }
            default { throw "Unknown Joining_Type" }
        }
        $lines.Add(("    (0x{0:X06}, 0x{1:X06}, JoiningType::{2})," -f $record.Start, $record.End, $type))
    }
    $lines.Add("];")
    if ($lines.Count -gt 300) { throw "$shardName exceeds the 300-line source limit" }
    [IO.File]::WriteAllLines($shardPath, $lines, [Text.UTF8Encoding]::new($false))
    & rustfmt --edition 2024 $shardPath
    if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for $shardPath" }
}

$indexLines = [System.Collections.Generic.List[string]]::new()
$indexLines.Add("//! Индекс таблиц joining ranges Unicode 17.")
foreach ($shardPath in $shardPaths) {
    $shardName = [IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("#[path = `"joining_ranges/$shardName.rs`"]")
    $indexLines.Add("mod $shardName;")
}
$indexLines.Add("")
$indexLines.Add("pub(super) const TABLES: &[&[(u32, u32, super::JoiningType)]] = &[")
foreach ($shardPath in $shardPaths) {
    $shardName = [IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("    $shardName`::RANGES,")
}
$indexLines.Add("];")
[IO.File]::WriteAllLines((Join-Path $OutputDirectory "joining_ranges.rs"), $indexLines, [Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 (Join-Path $OutputDirectory "joining_ranges.rs")
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for joining table index" }

$viramaPoints = [System.Collections.Generic.List[int]]::new()
$unicodePattern = [System.Text.RegularExpressions.Regex]::new(
    '^(?<point>[0-9A-F]{4,6});[^;]*;[^;]*;(?<class>[0-9]+);'
)
foreach ($line in [System.IO.File]::ReadLines((Join-Path $DataDirectory "UnicodeData.txt"))) {
    $match = $unicodePattern.Match($line)
    if ($match.Success -and $match.Groups["class"].Value -eq "9") {
        $viramaPoints.Add([Convert]::ToInt32($match.Groups["point"].Value, 16))
    }
}
$viramaRanges = compress_points $viramaPoints
$viramaLines = [System.Collections.Generic.List[string]]::new()
$viramaLines.Add("//! Unicode 17 Canonical_Combining_Class=Virama ranges.")
$viramaLines.Add("pub(super) const RANGES: &[(u32, u32)] = &[")
foreach ($range in $viramaRanges) {
    $viramaLines.Add(("    (0x{0:X06}, 0x{1:X06})," -f $range.Start, $range.End))
}
$viramaLines.Add("];")
if ($viramaLines.Count -gt 300) { throw "Virama table exceeds the 300-line source limit" }
[IO.File]::WriteAllLines((Join-Path $OutputDirectory "virama_ranges.rs"), $viramaLines, [Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 (Join-Path $OutputDirectory "virama_ranges.rs")
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for Virama table" }
Write-Output ("Generated {0} joining ranges in {1} shards and {2} Virama ranges." -f $orderedJoining.Count, $shardPaths.Count, $viramaRanges.Count)
