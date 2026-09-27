param(
    [string]$DataPath = "data/g40-idna17/DerivedBidiClass.txt",
    [string]$OutputDirectory = "src/smtp/idna17",
    [int]$RangesPerShard = 250
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function class_abbreviation([string]$class) {
    switch ($class) {
        "Left_To_Right" { "L"; break }
        "Right_To_Left" { "R"; break }
        "Arabic_Letter" { "AL"; break }
        "European_Number" { "EN"; break }
        "European_Separator" { "ES"; break }
        "European_Terminator" { "ET"; break }
        "Arabic_Number" { "AN"; break }
        "Common_Separator" { "CS"; break }
        default {
            if ($class -match '^(L|R|AL|EN|ES|ET|AN|CS|NSM|BN|B|S|WS|ON|LRE|LRO|RLE|RLO|PDF|LRI|RLI|FSI|PDI)$') {
                return $class
            }
            throw "Unknown Bidi_Class '$class'"
        }
    }
}

function rust_variant([string]$class) {
    switch ($class) {
        "L" { "LeftToRight"; break }
        "R" { "RightToLeft"; break }
        "AL" { "ArabicLetter"; break }
        "EN" { "EuropeanNumber"; break }
        "ES" { "EuropeanSeparator"; break }
        "ET" { "EuropeanTerminator"; break }
        "AN" { "ArabicNumber"; break }
        "CS" { "CommonSeparator"; break }
        "NSM" { "NonspacingMark"; break }
        "BN" { "BoundaryNeutral"; break }
        "B" { "ParagraphSeparator"; break }
        "S" { "SegmentSeparator"; break }
        "WS" { "Whitespace"; break }
        "ON" { "OtherNeutral"; break }
        "LRE" { "LeftToRightEmbedding"; break }
        "LRO" { "LeftToRightOverride"; break }
        "RLE" { "RightToLeftEmbedding"; break }
        "RLO" { "RightToLeftOverride"; break }
        "PDF" { "PopDirectionalFormat"; break }
        "LRI" { "LeftToRightIsolate"; break }
        "RLI" { "RightToLeftIsolate"; break }
        "FSI" { "FirstStrongIsolate"; break }
        "PDI" { "PopDirectionalIsolate"; break }
        default { throw "Unknown Bidi_Class '$class'" }
    }
}

function add_range_points($classMap, $record) {
    for ($point = $record.Start; $point -le $record.End; $point++) {
        switch ($record.Class) {
            "L" { $null = $classMap.Remove($point) }
            default { $classMap[$point] = $record.Class }
        }
    }
}

# Digest закрепляет derived property и Unicode 17.0.0 без runtime-зависимости.
$expectedSha256 = "4867B4B7F0731ED1BFCD34CC6251211FF1542541FCE0734B6FBDA139EE80B3A4"
$actualSha256 = (Get-FileHash -LiteralPath $DataPath -Algorithm SHA256).Hash
if ($actualSha256 -ne $expectedSha256) {
    throw "DerivedBidiClass.txt не совпадает с официальной Unicode 17.0.0 редакцией"
}

$classMap = [System.Collections.Generic.Dictionary[int, string]]::new()
$missingPattern = [regex]::new(
    '^\s*# @missing:\s*(?<start>[0-9A-F]{4,6})(?:\.\.(?<end>[0-9A-F]{4,6}))?\s*;\s*(?<class>[A-Za-z_]+)'
)
$missingRecords = [System.Collections.Generic.List[object]]::new()
$explicitPattern = [regex]::new(
    '^\s*(?<start>[0-9A-F]{4,6})(?:\.\.(?<end>[0-9A-F]{4,6}))?\s*;\s*(?<class>[A-Z]+)\b'
)
$explicitRecords = [System.Collections.Generic.List[object]]::new()
foreach ($line in [IO.File]::ReadLines($DataPath)) {
    $missingMatch = $missingPattern.Match($line)
    if ($missingMatch.Success) {
        $start = [Convert]::ToInt32($missingMatch.Groups["start"].Value, 16)
        $end = $start
        $endText = $missingMatch.Groups["end"].Value
        if (-not [string]::IsNullOrEmpty($endText)) { $end = [Convert]::ToInt32($endText, 16) }
        $class = class_abbreviation $missingMatch.Groups["class"].Value
        if ($class -ne "L") {
            $missingRecords.Add([pscustomobject]@{ Start = $start; End = $end; Class = $class; Span = $end - $start })
        }
        continue
    }
    $explicitMatch = $explicitPattern.Match($line)
    if (-not $explicitMatch.Success) { continue }
    $start = [Convert]::ToInt32($explicitMatch.Groups["start"].Value, 16)
    $end = $start
    $endText = $explicitMatch.Groups["end"].Value
    if (-not [string]::IsNullOrEmpty($endText)) { $end = [Convert]::ToInt32($endText, 16) }
    $explicitRecords.Add([pscustomobject]@{
        Start = $start
        End = $end
        Class = class_abbreviation $explicitMatch.Groups["class"].Value
    })
}

# Вложенные @missing интервалы применяются от широкого к более узкому.
foreach ($record in @($missingRecords | Sort-Object -Property Span -Descending)) {
    add_range_points $classMap $record
}
foreach ($record in $explicitRecords) {
    add_range_points $classMap $record
}

$points = [System.Collections.Generic.List[int]]::new()
foreach ($point in $classMap.Keys) { $points.Add($point) }
$points.Sort()
$ranges = [System.Collections.Generic.List[object]]::new()
$rangeStart = $null
$rangeEnd = $null
$rangeClass = $null
foreach ($point in $points) {
    $class = $classMap.Get_Item($point)
    if (($null -ne $rangeStart) -and ($point -eq ($rangeEnd + 1)) -and ($class -eq $rangeClass)) {
        $rangeEnd = $point
        continue
    }
    if ($null -ne $rangeStart) {
        $ranges.Add([pscustomobject]@{ Start = $rangeStart; End = $rangeEnd; Class = $rangeClass })
    }
    $rangeStart = $point
    $rangeEnd = $point
    $rangeClass = $class
}
if ($null -ne $rangeStart) {
    $ranges.Add([pscustomobject]@{ Start = $rangeStart; End = $rangeEnd; Class = $rangeClass })
}

$shardRoot = Join-Path $OutputDirectory "bidi_ranges"
New-Item -ItemType Directory -Path $shardRoot -Force | Out-Null
$shardPaths = [System.Collections.Generic.List[string]]::new()
for ($offset = 0; $offset -lt $ranges.Count; $offset += $RangesPerShard) {
    $chunk = @($ranges | Select-Object -Skip $offset -First $RangesPerShard)
    $shardName = "bidi_{0:D2}.rs" -f $shardPaths.Count
    $shardPath = Join-Path $shardRoot $shardName
    $shardPaths.Add($shardPath)
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("//! Фрагмент non-L Bidi_Class ranges Unicode 17.")
    $lines.Add("use super::super::BidiClass;")
    $lines.Add("")
    $lines.Add("pub(super) const RANGES: &[(u32, u32, BidiClass)] = &[")
    foreach ($record in $chunk) {
        $variant = rust_variant $record.Class
        $lines.Add(("    (0x{0:X06}, 0x{1:X06}, BidiClass::{2})," -f $record.Start, $record.End, $variant))
    }
    $lines.Add("];")
    if ($lines.Count -gt 300) { throw "$shardName exceeds the 300-line source limit" }
    [IO.File]::WriteAllLines($shardPath, $lines, [Text.UTF8Encoding]::new($false))
    & rustfmt --edition 2024 $shardPath
    if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for $shardPath" }
}

$indexLines = [System.Collections.Generic.List[string]]::new()
$indexLines.Add("//! Индекс bidi ranges Unicode 17.")
foreach ($shardPath in $shardPaths) {
    $shardName = [IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("#[path = `"bidi_ranges/$shardName.rs`"]")
    $indexLines.Add("mod $shardName;")
}
$indexLines.Add("")
$indexLines.Add("pub(super) const TABLES: &[&[(u32, u32, super::BidiClass)]] = &[")
foreach ($shardPath in $shardPaths) {
    $shardName = [IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("    $shardName`::RANGES,")
}
$indexLines.Add("];")
$indexPath = Join-Path $OutputDirectory "bidi_ranges.rs"
[IO.File]::WriteAllLines($indexPath, $indexLines, [Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 $indexPath
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for bidi table index" }
Write-Output ("Generated {0} non-L Bidi_Class ranges in {1} shards." -f $ranges.Count, $shardPaths.Count)
