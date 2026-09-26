param(
    [string]$DataDirectory = "data/g20-unicode17-normalization",
    [string]$OutputDirectory = "src/imap/unicode_17_nfc"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$pinnedFiles = @{
    "UnicodeData.txt" = "2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C"
    "DerivedNormalizationProps.txt" = "71FD6A206A2C0CDD41FEB6B7F656AA31091DB45E9CEDC926985D718397F9E488"
    "NormalizationTest.txt" = "5019FFD530751A741900C849C0E010332F142A3612234639BD200B82138A87DB"
}
foreach ($fileName in $pinnedFiles.Keys) {
    $path = Join-Path $DataDirectory $fileName
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    if ($actual -ne $pinnedFiles.Get_Item($fileName)) { throw "$fileName hash differs from pinned UCD 17.0.0" }
}

$decompositions = [System.Collections.Generic.List[object]]::new()
$combiningClasses = [System.Collections.Generic.List[object]]::new()
foreach ($line in [System.IO.File]::ReadLines((Join-Path $DataDirectory "UnicodeData.txt"))) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $pointText, $name, $category, $classText, $bidi, $decompositionText, `
        $decimal, $digit, $numeric, $mirrored, $oldName, $isoComment, `
        $uppercase, $lowercase, $titlecase = $line.Split(';')
    $point = [Convert]::ToInt32($pointText, 16)
    $class = [Convert]::ToInt32($classText, 10)
    if ($class -gt 0) { $combiningClasses.Add([pscustomobject]@{ Point = $point; Class = $class }) }
    if ([string]::IsNullOrWhiteSpace($decompositionText) -or $decompositionText.StartsWith("<")) { continue }
    $mapping = [System.Collections.Generic.List[int]]::new()
    foreach ($part in $decompositionText.Split(' ', [StringSplitOptions]::RemoveEmptyEntries)) {
        $mapping.Add([Convert]::ToInt32($part, 16))
    }
    $decompositions.Add([pscustomobject]@{ Point = $point; Mapping = $mapping.ToArray() })
}

$exclusions = [System.Collections.Generic.HashSet[int]]::new()
foreach ($line in [System.IO.File]::ReadLines((Join-Path $DataDirectory "DerivedNormalizationProps.txt"))) {
    $content, $comment = $line.Split('#', 2)
    $content = $content.Trim()
    if ([string]::IsNullOrEmpty($content)) { continue }
    $rangeText, $property = $content.Split(';', 2)
    if ($property.Trim() -ne "Full_Composition_Exclusion") { continue }
    $startText, $endText = $rangeText.Trim().Split("..", [StringSplitOptions]::None)
    $startPoint = [Convert]::ToInt32($startText, 16)
    $endPoint = $startPoint
    if (-not [string]::IsNullOrEmpty($endText)) { $endPoint = [Convert]::ToInt32($endText, 16) }
    for ($point = $startPoint; $point -le $endPoint; $point++) { $null = $exclusions.Add($point) }
}

$classByPoint = [System.Collections.Generic.Dictionary[int, int]]::new()
foreach ($entry in $combiningClasses) { $classByPoint.Add($entry.Point, $entry.Class) }
$compositions = [System.Collections.Generic.List[object]]::new()
foreach ($entry in $decompositions) {
    if ($entry.Mapping.Count -ne 2 -or $exclusions.Contains($entry.Point)) { continue }
    $starter = $entry.Mapping | Select-Object -First 1
    $starterClass = 0
    if ($classByPoint.TryGetValue($starter, [ref]$starterClass)) { continue }
    $combining = $entry.Mapping | Select-Object -Skip 1 -First 1
    $compositions.Add([pscustomobject]@{
        Starter = $starter
        Combining = $combining
        Composite = $entry.Point
    })
}
$compositions = [System.Collections.Generic.List[object]]::new(@($compositions | Sort-Object Starter, Combining))

$classRanges = [System.Collections.Generic.List[object]]::new()
$classIterator = $combiningClasses.GetEnumerator()
if ($classIterator.MoveNext()) {
    $rangeStart = $classIterator.Current.Point
    $rangeEnd = $rangeStart
    $rangeClass = $classIterator.Current.Class
    while ($classIterator.MoveNext()) {
        $entry = $classIterator.Current
        if ($entry.Point -eq ($rangeEnd + 1) -and $entry.Class -eq $rangeClass) {
            $rangeEnd = $entry.Point
            continue
        }
        $classRanges.Add([pscustomobject]@{ Start = $rangeStart; End = $rangeEnd; Class = $rangeClass })
        $rangeStart = $entry.Point
        $rangeEnd = $entry.Point
        $rangeClass = $entry.Class
    }
    $classRanges.Add([pscustomobject]@{ Start = $rangeStart; End = $rangeEnd; Class = $rangeClass })
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$index = [System.Collections.Generic.List[string]]::new()
$index.Add("//! Индекс generated Unicode 17.0.0 normalization shards.")
$shardSize = 150
foreach ($table in @(
    @{ Name = "decomposition"; Entries = $decompositions },
    @{ Name = "combining_class"; Entries = $classRanges },
    @{ Name = "composition"; Entries = $compositions }
)) {
    $tableModules = [System.Collections.Generic.List[string]]::new()
    $offset = 0
    while ($offset -lt $table.Entries.Count) {
        $count = [Math]::Min($shardSize, $table.Entries.Count - $offset)
        $moduleName = "{0}_{1:D3}" -f $table.Name, $tableModules.Count
        $tableModules.Add($moduleName)
        $rows = $table.Entries.GetRange($offset, $count)
        $lines = [System.Collections.Generic.List[string]]::new()
        $lines.Add("//! Данные Unicode 17.0.0; copyright Unicode, Inc., см. docs/unicode-data.md.")
        switch ($table.Name) {
            "decomposition" {
                $indexType = "(u32, &[u32])"
                $lines.Add("pub(super) const ENTRIES: &[(u32, &[u32])] = &[")
                foreach ($row in $rows) {
                    $values = ($row.Mapping | ForEach-Object { "0x{0:X06}" -f $_ }) -join ", "
                    $lines.Add(("    (0x{0:X06}, &[{1}])," -f $row.Point, $values))
                }
            }
            "combining_class" {
                $indexType = "(u32, u32, u8)"
                $lines.Add("pub(super) const ENTRIES: &[(u32, u32, u8)] = &[")
                foreach ($row in $rows) {
                    $lines.Add(("    (0x{0:X06}, 0x{1:X06}, {2})," -f $row.Start, $row.End, $row.Class))
                }
            }
            "composition" {
                $indexType = "(u32, u32, u32)"
                $lines.Add("pub(super) const ENTRIES: &[(u32, u32, u32)] = &[")
                foreach ($row in $rows) {
                    $lines.Add(("    (0x{0:X06}, 0x{1:X06}, 0x{2:X06})," -f $row.Starter, $row.Combining, $row.Composite))
                }
            }
        }
        $lines.Add("];")
        $target = Join-Path $OutputDirectory "$moduleName.rs"
        [System.IO.File]::WriteAllLines($target, $lines, [System.Text.UTF8Encoding]::new($false))
        & rustfmt --edition 2024 $target
        if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for $target" }
        if ((Get-Content $target).Count -gt 300) { throw "$moduleName exceeds 300 lines" }
        $offset += $count
    }
    foreach ($moduleName in $tableModules) {
        $index.Add("#[path = `"$moduleName.rs`"]")
        $index.Add("mod $moduleName;")
    }
    $references = ($tableModules | ForEach-Object { "$_`::ENTRIES" }) -join ", "
    $index.Add("pub(super) const $($table.Name.ToUpperInvariant()): &[&[$indexType]] = &[$references];")
}
$indexPath = Join-Path $OutputDirectory "data.rs"
[System.IO.File]::WriteAllLines($indexPath, $index, [System.Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 $indexPath
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for $indexPath" }
if ((Get-Content $indexPath).Count -gt 300) { throw "data.rs exceeds 300 lines" }
"Generated Unicode 17 NFC tables: decomposition={0}, ccc-ranges={1}, composition={2}." -f `
    $decompositions.Count, $classRanges.Count, $compositions.Count
