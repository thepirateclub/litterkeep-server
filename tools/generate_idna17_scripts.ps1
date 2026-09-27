param(
    [string]$ScriptsPath = "data/g40-idna17/Scripts.txt",
    [string]$OutputPath = "src/smtp/idna17/script_ranges.rs"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# SHA закрепляет точную редакцию официального Unicode 17.0.0 Scripts.txt.
$expectedSha256 = "9F5E50D3ABAEE7D6CE09480F325C706F485AE3240912527E651954D2D6B035BF"
$actualSha256 = (Get-FileHash -LiteralPath $ScriptsPath -Algorithm SHA256).Hash
if ($actualSha256 -ne $expectedSha256) {
    throw "Scripts.txt не совпадает с официальной версией Unicode 17.0.0"
}

$records = [System.Collections.Generic.List[object]]::new()
$pattern = [System.Text.RegularExpressions.Regex]::new(
    '^\s*(?<start>[0-9A-F]{4,6})(?:\.\.(?<end>[0-9A-F]{4,6}))?\s*;\s*(?<script>Greek|Hebrew|Hiragana|Katakana|Han)\b'
)
foreach ($line in [System.IO.File]::ReadLines($ScriptsPath)) {
    $match = $pattern.Match($line)
    if (-not $match.Success) { continue }
    $start = [Convert]::ToInt32($match.Groups["start"].Value, 16)
    $end = $start
    $endText = $match.Groups["end"].Value
    if (-not [string]::IsNullOrEmpty($endText)) { $end = [Convert]::ToInt32($endText, 16) }
    $records.Add([pscustomobject]@{ Start = $start; End = $end; Script = $match.Groups["script"].Value })
}

$orderedRecords = @($records | Sort-Object -Property Start)
$previousEnd = -1
foreach ($record in $orderedRecords) {
    if ($record.Start -le $previousEnd) { throw "Script ranges are not sorted and disjoint" }
    $previousEnd = $record.End
}

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("//! Unicode 17 Script ranges, нужные CONTEXTO правилам IDNA2008.")
$lines.Add("use super::Script;")
$lines.Add("")
$lines.Add("pub(super) const RANGES: &[(u32, u32, Script)] = &[")
foreach ($record in $orderedRecords) {
    $lines.Add(("    (0x{0:X06}, 0x{1:X06}, Script::{2})," -f $record.Start, $record.End, $record.Script))
}
$lines.Add("];")
if ($lines.Count -gt 300) { throw "Generated Script table exceeds the 300-line source limit" }
[System.IO.File]::WriteAllLines($OutputPath, $lines, [System.Text.UTF8Encoding]::new($false))
& rustfmt --edition 2024 $OutputPath
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed for Unicode Script table" }
Write-Output ("Generated {0} Script ranges from pinned Unicode 17.0.0 data." -f $orderedRecords.Count)
