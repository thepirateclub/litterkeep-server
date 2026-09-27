param(
    [string]$DataPath = "data/g40-idna17/UnicodeData.txt",
    [string]$OutputDirectory = "src/smtp/idna17",
    [int]$RangesPerShard = 250
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# Закрепляем ровно ту редакцию UnicodeData, на которой построена NFC-таблица.
$expectedSha256 = "2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C"
$actualSha256 = (Get-FileHash -LiteralPath $DataPath -Algorithm SHA256).Hash
if ($actualSha256 -ne $expectedSha256) {
    throw "UnicodeData.txt не совпадает с официальной Unicode 17.0.0 редакцией"
}

$points = [System.Collections.Generic.List[int]]::new()
foreach ($line in [IO.File]::ReadLines($DataPath)) {
    $fields = $line.Split(';')
    $category = $fields.GetValue(2)
    if ($category -notmatch '^M[nce]$') { continue }
    $name = $fields.GetValue(1)
    if ($name -match ', (First|Last)>') {
        throw "Неожиданный диапазон General_Category Mark: $line"
    }
    $points.Add([Convert]::ToInt32($fields.GetValue(0), 16))
}
$points.Sort()

$ranges = [System.Collections.Generic.List[object]]::new()
$start = $null
$end = $null
foreach ($point in $points) {
    if ($null -eq $start) {
        $start = $point
        $end = $point
        continue
    }
    if ($point -eq ($end + 1)) {
        $end = $point
        continue
    }
    $ranges.Add([pscustomobject]@{ Start = $start; End = $end })
    $start = $point
    $end = $point
}
if ($null -ne $start) { $ranges.Add([pscustomobject]@{ Start = $start; End = $end }) }

$shardRoot = Join-Path $OutputDirectory "mark_ranges"
New-Item -ItemType Directory -Path $shardRoot -Force | Out-Null
$shardPaths = [System.Collections.Generic.List[string]]::new()
for ($offset = 0; $offset -lt $ranges.Count; $offset += $RangesPerShard) {
    $chunk = @($ranges | Select-Object -Skip $offset -First $RangesPerShard)
    $shardName = "marks_{0:D2}.rs" -f $shardPaths.Count
    $shardPath = Join-Path $shardRoot $shardName
    $shardPaths.Add($shardPath)
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("//! Диапазоны General_Category Mark Unicode 17.")
    $lines.Add("pub(super) const RANGES: &[(u32, u32)] = &[")
    foreach ($range in $chunk) {
        $lines.Add(("    (0x{0:X06}, 0x{1:X06})," -f $range.Start, $range.End))
    }
    $lines.Add("];")
    if ($lines.Count -gt 300) { throw "$shardName превышает предел 300 строк" }
    [IO.File]::WriteAllLines($shardPath, $lines, [Text.UTF8Encoding]::new($false))
    & rustfmt --edition 2024 $shardPath
    if ($LASTEXITCODE -ne 0) { throw "rustfmt завершился ошибкой: $shardPath" }
}

$indexLines = [System.Collections.Generic.List[string]]::new()
$indexLines.Add("//! Индекс Unicode 17 General_Category Mark ranges.")
foreach ($shardPath in $shardPaths) {
    $shardName = [IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("#[path = `"mark_ranges/$shardName.rs`"]")
    $indexLines.Add("mod $shardName;")
}
$indexLines.Add("")
$indexLines.Add("pub(super) const TABLES: &[&[(u32, u32)]] = &[")
foreach ($shardPath in $shardPaths) {
    $shardName = [IO.Path]::GetFileNameWithoutExtension($shardPath)
    $indexLines.Add("    $shardName`::RANGES,")
}
$indexLines.Add("];")
$indexPath = Join-Path $OutputDirectory "mark_ranges.rs"
[IO.File]::WriteAllLines($indexPath, $indexLines, [Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 $indexPath
if ($LASTEXITCODE -ne 0) { throw "rustfmt завершился ошибкой для индекса Mark" }
Write-Output ("Generated {0} Unicode Mark ranges in {1} shards." -f $ranges.Count, $shardPaths.Count)
