param(
    [string]$UnicodeDataPath = "data/g19-unicode17/UnicodeData.txt",
    [string]$OutputDirectory = "src/imap/unicode_17_ranges"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# Закреплённый SHA-256 не позволяет случайно смешать таблицы разных редакций UCD.
$expectedSha256 = "2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C"
$actualSha256 = (Get-FileHash -LiteralPath $UnicodeDataPath -Algorithm SHA256).Hash
if ($actualSha256 -ne $expectedSha256) {
    throw "UnicodeData.txt не совпадает с зафиксированной Unicode 17.0.0 редакцией"
}

$assignedRanges = [System.Collections.Generic.List[object]]::new()
$pendingStart = $null
foreach ($line in [System.IO.File]::ReadLines($UnicodeDataPath)) {
    if ([string]::IsNullOrEmpty($line)) { continue }
    $codePointText, $name, $category = $line -split ";", 3
    if ($category -eq "Cn") { continue }
    $codePoint = [Convert]::ToInt32($codePointText, 16)
    switch -Regex ($name) {
        ", First>$" {
            if ($null -ne $pendingStart) { throw "Повторная Unicode First-запись" }
            $pendingStart = $codePoint
            break
        }
        ", Last>$" {
            if ($null -eq $pendingStart) { throw "Unicode Last-запись без First" }
            $assignedRanges.Add([System.ValueTuple]::Create($pendingStart, $codePoint))
            $pendingStart = $null
            break
        }
        default {
            $assignedRanges.Add([System.ValueTuple]::Create($codePoint, $codePoint))
            break
        }
    }
}
if ($null -ne $pendingStart) { throw "Unicode First-запись без Last" }

$blocks = @(
    @{ Name = "plane_00_a"; Start = 0x00000; End = 0x07FFF },
    @{ Name = "plane_00_b"; Start = 0x08000; End = 0x0FFFF },
    @{ Name = "plane_01_a"; Start = 0x10000; End = 0x17FFF },
    @{ Name = "plane_01_b"; Start = 0x18000; End = 0x1FFFF }
)
foreach ($plane in 2..16) {
    $start = $plane * 0x10000
    $blocks += @{ Name = ("plane_{0:X2}" -f $plane); Start = $start; End = ($start + 0xFFFF) }
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$allUnassignedRanges = [System.Collections.Generic.List[object]]::new()
foreach ($block in $blocks) {
    $cursor = $block.Start
    $blockUnassignedRanges = [System.Collections.Generic.List[object]]::new()
    $tableLines = [System.Collections.Generic.List[string]]::new()
    $tableLines.Add("//! Неприсвоенные code points Unicode 17.0.0 для части кодового пространства.")
    $tableLines.Add("// Copyright © 1991-2025 Unicode, Inc.; лицензия включена в docs/unicode-data.md.")
    $tableLines.Add("pub(super) const RANGES: &[(u32, u32)] = &[")
    foreach ($range in $assignedRanges) {
        $assignedStart = [Math]::Max($range.Item1, $block.Start)
        $assignedEnd = [Math]::Min($range.Item2, $block.End)
        if ($assignedStart -gt $assignedEnd) { continue }
        if ($assignedEnd -lt $cursor) { continue }
        if ($assignedStart -gt $cursor) {
            $gapEnd = $assignedStart - 1
            if ($gapEnd -gt $block.End) { throw "$($block.Name) interval exceeded its block" }
            $gap = [System.ValueTuple]::Create($cursor, $gapEnd)
            $blockUnassignedRanges.Add($gap)
            $allUnassignedRanges.Add($gap)
            $tableLines.Add(("    (0x{0:X06}, 0x{1:X06})," -f $cursor, $gapEnd))
        }
        if ($assignedEnd -ge $cursor) { $cursor = $assignedEnd + 1 }
    }
    if ($cursor -le $block.End) {
        $gap = [System.ValueTuple]::Create($cursor, $block.End)
        $blockUnassignedRanges.Add($gap)
        $allUnassignedRanges.Add($gap)
        $tableLines.Add(("    (0x{0:X06}, 0x{1:X06})," -f $cursor, $block.End))
    }
    $tableLines.Add("];")
    if ($tableLines.Count -gt 300) { throw "$($block.Name) превысил лимит 300 строк" }
    $target = Join-Path $OutputDirectory ("{0}.rs" -f $block.Name)
    [System.IO.File]::WriteAllLines($target, $tableLines, [System.Text.UTF8Encoding]::new($false))
    & rustfmt --edition 2024 $target
    if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for $target" }

    $actualRanges = [System.Collections.Generic.List[object]]::new()
    $pattern = [System.Text.RegularExpressions.Regex]::new("\(0x(?<start>[0-9A-F]{6}), 0x(?<end>[0-9A-F]{6})\)")
    foreach ($match in $pattern.Matches([System.IO.File]::ReadAllText($target))) {
        $start = [Convert]::ToInt32($match.Groups.Item("start").Value, 16)
        $end = [Convert]::ToInt32($match.Groups.Item("end").Value, 16)
        $actualRanges.Add([System.ValueTuple]::Create($start, $end))
    }
    if ($actualRanges.Count -ne $blockUnassignedRanges.Count) {
        throw "$($block.Name) generated interval count does not match UnicodeData"
    }
    $expectedIterator = $blockUnassignedRanges.GetEnumerator()
    $actualIterator = $actualRanges.GetEnumerator()
    while ($expectedIterator.MoveNext()) {
        if (-not $actualIterator.MoveNext()) { throw "$($block.Name) table ended early" }
        $expectedRange = $expectedIterator.Current
        $actualRange = $actualIterator.Current
        if ($expectedRange.Item1 -ne $actualRange.Item1) { throw "$($block.Name) start mismatch" }
        if ($expectedRange.Item2 -ne $actualRange.Item2) { throw "$($block.Name) end mismatch" }
    }
}

# Полный scalar-space проход ловит потерянные и лишние интервалы в сериализации.
$assignedIterator = $assignedRanges.GetEnumerator()
$unassignedIterator = $allUnassignedRanges.GetEnumerator()
$hasAssigned = $assignedIterator.MoveNext()
$hasUnassigned = $unassignedIterator.MoveNext()
$assignedRange = $assignedIterator.Current
$unassignedRange = $unassignedIterator.Current
for ($codePoint = 0; $codePoint -le 0x10FFFF; $codePoint++) {
    while ($hasAssigned) {
        if ($assignedRange.Item2 -ge $codePoint) { break }
        $hasAssigned = $assignedIterator.MoveNext()
        if ($hasAssigned) { $assignedRange = $assignedIterator.Current }
    }
    $expectedAssigned = $false
    if ($hasAssigned) {
        if ($assignedRange.Item1 -le $codePoint) {
            $expectedAssigned = $assignedRange.Item2 -ge $codePoint
        }
    }
    while ($hasUnassigned) {
        if ($unassignedRange.Item2 -ge $codePoint) { break }
        $hasUnassigned = $unassignedIterator.MoveNext()
        if ($hasUnassigned) { $unassignedRange = $unassignedIterator.Current }
    }
    $actualUnassigned = $false
    if ($hasUnassigned) {
        if ($unassignedRange.Item1 -le $codePoint) {
            $actualUnassigned = $unassignedRange.Item2 -ge $codePoint
        }
    }
    if ($expectedAssigned -eq $actualUnassigned) {
        throw ("assignment mismatch at U+{0:X6}" -f $codePoint)
    }
}

"Generated {0} Unicode 17.0.0 table shards." -f $blocks.Count
