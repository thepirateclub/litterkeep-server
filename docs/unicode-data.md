# Unicode data profile

> **About this document.** Pinned Unicode data version: NFC, IDNA2008, SASLprep, data sources and the data licence.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Pinned version

The mailbox validator uses the tables of unassigned scalar values from the
Unicode Character Database 17.0.0 `UnicodeData.txt`. This version matches
`char::UNICODE_VERSION` of the installed Rust 1.97.1 and the `rust:1.97`
runtime image. A compile-time assertion in `src/imap/unicode_17.rs` forbids the
build if the built-in Unicode version changes without updating the tables.

Official source:
`https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt`.
SHA-256: `2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C`.
The local acceptance copy is in `./data/g19-unicode17/UnicodeData.txt`.
It is not a runtime file of the server and is not included in the binary.

`tools/generate_unicode17_unassigned.ps1` builds sorted non-overlapping
complement ranges and writes the shards into `src/imap/unicode_17_ranges/`.
The script verifies the SHA-256 of the source and the 300-line limit per shard.
To reproduce it you need PowerShell, the pinned UCD file and rustfmt of the
installed toolchain:

```powershell
.\tools\generate_unicode17_unassigned.ps1
cargo test --locked imap::unicode_17::tests
cargo test --locked imap::mailbox_utf8::tests
```

The runtime uses a binary search over the inclusive ranges. There is no Unicode
crate or production dependency beyond the allowlist. Private-use scalars remain
assigned and are accepted: RFC 5198 recommends avoiding them but does not forbid
them.

## NFC normalization for Net-Unicode

Accepting mailbox names rejects the unassigned code points of the pinned
version, the controls forbidden by RFC 9755 and a leading BOM. The NFC module is
located in the shared Unicode layer, so the protocol boundaries and the Store use
a single version of the algorithm. In UTF8=ACCEPT mode the incoming names are
canonicalized before the storage access, and the IMAP mailbox responses are
serialized with the same algorithm.

For the records of the server versions before NFC the new version does not
rewrite folder or subscription journals. The Store looks up the old name by NFC
equivalence, returns the canonical name in the listings and allows
rename/delete/unsubscribe through an alias. The stable folder ID stays the
same, so the mailbox data key does not change. If two names with the same NFC
form are found in an old directory, the point resolution of the name returns
InvalidInput instead of picking one folder at random. Unit tests check the
replay of an old folder journal through a full Store reopen, the preservation of
the content and the alias operations; an independent IMAP STARTTLS scenario
checks an old modified UTF-7 name through a new UTF8=ACCEPT connection.

The NFC tables are built from the official Unicode 17.0.0 `UnicodeData.txt`,
`DerivedNormalizationProps.txt` and `NormalizationTest.txt`. The generator
`tools/generate_unicode17_nfc.ps1` pins the SHA-256 of every input, excludes the
composition exclusions, generates the canonical decomposition/combining
class/composition tables and splits them into Rust shards of less than 300
lines. The algorithm in `src/imap/unicode_17_nfc.rs` handles Hangul
algorithmically and the remaining sequences through the sorted tables and a
binary search. There are no new runtime dependencies.

The full official corpus passed with a separate command against the pinned
tables:

```powershell
$env:LITTERKEEP_UNICODE_NORMALIZATION_TEST = (Resolve-Path data/g20-unicode17-normalization/NormalizationTest.txt).Path
cargo test --locked unicode::nfc::tests::official_unicode_normalization_vectors -- --ignored --exact
```

All ordinary NFC unit vectors are part of the standard `cargo test`. An
independent STARTTLS black-box check verifies a decomposed CREATE, a composed
LIST response, SELECT by the composed name and DELETE by the decomposed name. A
full Store reopen and a wire test verify the access to the preserved decomposed
folder record through an NFC alias; the journals are not rewritten. RFC 5198
formulates the normalization of the outgoing strings as a SHOULD; the NFC
implementation does not close the remaining Net-Unicode response paths and by
itself does not allow advertising `UTF8=ACCEPT`.

## IDNA2008 derived properties for SMTP

`smtp::idna17` uses the official Unicode 17.0.0 file `idna/Idna2008.txt`,
which forms the derived properties according to RFC 5892. Source:
`https://www.unicode.org/Public/17.0.0/idna/Idna2008.txt`.
SHA-256: `E4A7526A8A37539C0DEFA4DA25F5DBF77D0212A14D4762D455CADEA608A8921C`.
The local copy for the reproducible generation is stored in
`./data/g40-idna17/Idna2008.txt`, is not shipped at runtime and is not included
in the binary.

The PowerShell generator `tools/generate_idna17_ranges.ps1` verifies the
SHA-256, extracts only PVALID/CONTEXTJ/CONTEXTO and splits the sorted ranges
into Rust shards of at most 300 lines. The check of every character is performed
by a binary search over these shards. To reproduce it you need the file from
`./data` and PowerShell:

```powershell
.\tools\generate_idna17_ranges.ps1
cargo test --locked smtp::idna17::tests
```

A separate ignored oracle test reads the same pinned corpus and compares the
Derived Property for every Unicode scalar, including ContextJ, ContextO and
the combined disallowed/unassigned values. Its full reproduction:

```powershell
$env:LITTERKEEP_IDNA2008_FILE = (Resolve-Path data/g40-idna17/Idna2008.txt).Path
cargo test --locked smtp::idna17::conformance_tests::derived_property_matches_official_unicode_17_corpus -- --ignored --exact
Remove-Item Env:LITTERKEEP_IDNA2008_FILE
```

The comparison covers 1,112,064 scalar values. It checks only the RFC 5892
category table, not the full label/domain conformance for NFC, hyphen,
ContextO/ContextJ, bidi, Punycode and the A-label round-trip.

This is only an RFC 5892 Derived Property lookup. PVALID does not mean that the
full label conforms to RFC 5891: NFC, the hyphen restrictions, the contextual
rules, bidi and the A-label round-trip must be enforced by the upper IDNA
layer. `smtp::idna17::to_alabel` applies NFC, hyphen rules, the PVALID lookup,
RFC 3492 encoding and the 63-octet limit of the encoded label. ContextO uses
Unicode 17 Scripts.txt: the Greek, Hebrew, Hiragana, Katakana and Han ranges
are needed for the U+0375/U+05F3/U+05F4/U+30FB checks. The middle dot and the
mutually exclusive Arabic digit sets are checked directly against the RFC
conditions. The generator `tools/generate_idna17_scripts.ps1` pins the SHA-256
of the source and creates a sorted table of 124 ranges of at most 300 lines.
Official source: `https://www.unicode.org/Public/17.0.0/ucd/Scripts.txt`.
SHA-256: `9F5E50D3ABAEE7D6CE09480F325C706F485AE3240912527E651954D2D6B035BF`.
The copy is in `./data/g40-idna17/Scripts.txt` and is needed only for the
repeated generation; the runtime does not depend on it.

ContextJ (ZWJ/ZWNJ) uses `DerivedJoiningType.txt` and the canonical combining
class 9 from `UnicodeData.txt`. ZWNJ accepts a directly preceding virama or
L/D + Transparent* + ZWNJ + Transparent* + R/D; ZWJ requires a directly
preceding virama. The generator `tools/generate_idna17_joining.ps1` pins both
SHA-256 versions, creates joining ranges shards of at most 300 lines and a
compressed virama table.

Unicode 17.0.0 sources:
`https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedJoiningType.txt`
with SHA-256 `F39EBE974825D6736AEE15582250307AA532B2CFAB3CAF3F86BD23FDDC9C5C4D`,
and `https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt` with SHA-256
`2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C`.
The fixture files are in `./data/g40-idna17/`; they are not required while the
server is running and serve only the reproducible generation.

RFC 5893 applies at the domain level: the presence of R, AL or AN activates the
rule for all labels, including the neighbouring ASCII/LTR. The incoming ASCII
`xn--` labels are decoded, checked as U-labels and must match a repeated
Punycode encoding. `to_alabel_domain` also converts the U-label domains into
the ASCII DNS form. The RFC 5891 prohibition of leading combining marks is
provided by the General_Category Mn/Mc/Me table from Unicode 17
`UnicodeData.txt` (the same pinned SHA-256 source as for NFC/virama). The
generator `tools/generate_idna17_marks.ps1` reproduces 327 ranges in two source
shards. The SMTP path parser applies the IDNA2008 domain check: the Unicode
U-labels are converted into ASCII A-labels before the local routing. The
SMTPUTF8 capability is advertised together with 8BITMIME, and a MAIL
reverse-path with Unicode requires the valueless SMTPUTF8 parameter. An
independent Linux STARTTLS client verified the submission of a message to an
address with a U-label, the delivery and the reading of an attachment via
IMAP; this is not a check of the Litterkeep application and not a confirmation
of the remote DNS/MX delivery.

The Punycode codec unit test now checks all 19 sample strings of RFC 3492 §7.1
in both directions. For the Russian example I the spelling after RFC Editor
Erratum 3026 is used. Vector S checks Bootstring for an arbitrary ASCII and is
not a valid IDNA label. These sample vectors do not replace a full IDNA2008
label/domain conformance suite: the full official corpus is still not executed.

Additionally the official Unicode 17 `IdnaTestV2.txt` from UTS #46 is
preserved: `./data/g41-idna-v2/IdnaTestV2.txt`, SHA-256
`BEB5D0BE20E896189B03209A82FDC34F06351502BBD4B8E2523583FC2954D9CF`, URL
`https://www.unicode.org/Public/17.0.0/idna/IdnaTestV2.txt`. To restore the
local ignored fixture:

```powershell
New-Item -ItemType Directory -Force data/g41-idna-v2 | Out-Null
Invoke-WebRequest -Uri 'https://www.unicode.org/Public/17.0.0/idna/IdnaTestV2.txt' -OutFile data/g41-idna-v2/IdnaTestV2.txt
Get-FileHash data/g41-idna-v2/IdnaTestV2.txt -Algorithm SHA256
```

This file is not a pure IDNA2008 oracle: it includes the UTS #46 mappings, the
transitional processing and a wider set of statuses. Therefore a separate
ignored test compares only the intersection of the profiles: NFC input, the
RFC 5892 PVALID labels (for an A-label the decoded U-label is checked), the
empty nontransitional ToUnicode and ToASCII statuses and the same ASCII
results. 172 rows passed on the pinned file; the two records with surrogate
code units are explicitly skipped, the remaining rows that do not belong to the
strict intersection are not counted. This does not confirm the UTS #46
mappings, the full CONTEXTJ/CONTEXTO conformance, the transitional mode or the
full IDNA2008 label/domain profile.

Reproduction of the selective cross-profile oracle:

```powershell
$env:LITTERKEEP_IDNA_TEST_V2_FILE = (Resolve-Path data/g41-idna-v2/IdnaTestV2.txt).Path
cargo test --locked smtp::idna17::idna_test_v2::strict_idna_matches_valid_nfc_pvalid_uts46_rows -- --ignored --exact --nocapture
Remove-Item Env:LITTERKEEP_IDNA_TEST_V2_FILE
```

## SASLprep profile for SCRAM-SHA-1

`src/sasl/saslprep.rs` implements the profile from RFC 4013 over the exact
Unicode 3.2 repertoire selected by that RFC. Mapping B.1 and C.1.2 precedes
NFKC, then the profile applies its complete prohibited-output list and RFC 3454
bidirectional checks. `SaslPrepContext::StoredValue` rejects A.1 unassigned
code points; `Query` permits them as RFC 3454 §7 allows for a comparison query.
The context is an enum so callers must choose the correct stored/query policy.

`ScramSha1Verifier::derive` accepts UTF-8 password bytes, runs SASLprep with
`StoredValue`, and feeds only the normalized bytes to PBKDF2-HMAC-SHA-1.
The API bounds the UTF-8 input at 4096 octets before normalization. Invalid
UTF-8 and profile errors fail before the KDF. This is verifier
derivation only: Argon2 account persistence and legacy credential migration do
not create a SCRAM verifier, the SCRAM protocol exchange is not implemented,
and the server does not advertise SCRAM mechanisms.

The shared Argon2 account verifier also uses this profile. New credentials are
written to the typed `SaslPrepCredentials` `.phc2` file kind after strict
`StoredValue` preparation, and authentication prepares the submitted UTF-8
query before Argon2 verification. Existing `.phc` records remain legacy raw
byte hashes until a successful authentication. That successful check writes a
fresh `.phc2` record from the submitted password using `StoredValue`, flushes
the temporary file, and atomically publishes it with a same-directory hard
link before synchronizing its containing directories and reporting success.
Concurrent migrations verify the already-published `.phc2` rather than reading
a partial file. The old `.phc` remains untouched as a recoverable fallback,
while the new record takes precedence on later authentication. A migration
write or durability failure returns a temporary storage failure and does not
authenticate the session. The legacy raw fallback may continue accepting
values that the new SASLprep profile would reject; this remains an explicit
compatibility gap for accounts that have not migrated.

The runtime tables are generated by `tools/generate_saslprep_tables.py` using
the Python standard library `stringprep` profile tables and
`unicodedata.ucd_3_2_0`; the generator refuses any Unicode version other than
3.2.0. It compares all 14 selected RFC 3454 property tables against their
published ranges for every code point before writing tables. NFKD mappings,
combining classes, and composition pairs are emitted as sorted Rust shards.
Each generated source file is checked against the 300-line limit and formatted
with the installed Rust formatter. No Python or Unicode crate is needed by the
server at runtime.

The local ignored source/corpus directory is `./data/g18-saslprep-20260928/`.
Pinned SHA-256 values:

- RFC 3454: `EB722FA698FB7E8823B835D9FD263E4CDB8F1C7B0D234EDF7F0E3BD2CCBB2C79`
- RFC 4013: `63DAA764693815AE8544728099D17A1C46E96B5818F2A399B3EC475EA153A991`
- Unicode 3.2 `UnicodeData-3.2.0.txt`:
  `5E444028B6E76D96F9DC509609C5E3222BF609056F35E5FCDE7E6FB8A58CD446`
- Unicode 3.2 `NormalizationTest-3.2.0.txt`:
  `C4513869BB7098D19838BE4A1FD5D760843C5804BFE03BD6BBB20623CEB6E57D`

Recreate the local fixtures and generated Rust data with PowerShell and Python:

```powershell
$saslpData = 'data/g18-saslprep-20260928'
New-Item -ItemType Directory -Force $saslpData | Out-Null
Invoke-WebRequest 'https://www.rfc-editor.org/rfc/rfc3454.txt' -OutFile "$saslpData/rfc3454.txt"
Invoke-WebRequest 'https://www.rfc-editor.org/rfc/rfc4013.txt' -OutFile "$saslpData/rfc4013.txt"
Invoke-WebRequest 'https://unicode.org/Public/3.2-Update/UnicodeData-3.2.0.txt' -OutFile "$saslpData/UnicodeData-3.2.0.txt"
Invoke-WebRequest 'https://unicode.org/Public/3.2-Update/NormalizationTest-3.2.0.txt' -OutFile "$saslpData/NormalizationTest-3.2.0.txt"
python tools/generate_saslprep_tables.py
cargo fmt --all -- --check
cargo test --locked sasl::saslprep::tests
$env:LITTERKEEP_UNICODE_32_NORMALIZATION_TEST = (Resolve-Path "$saslpData/NormalizationTest-3.2.0.txt").Path
cargo test --locked sasl::saslprep::tests::official_unicode_32_nfkc_vectors -- --ignored --exact
Remove-Item Env:LITTERKEEP_UNICODE_32_NORMALIZATION_TEST
```

The ignored Rust corpus test checks the NFKC result for each of the five
columns in the complete official Unicode 3.2 normalization corpus. The RFC
4013 examples separately cover B.1 removal, unchanged case, compatibility
normalization, prohibited output, and bidi rejection. These checks establish
the profile primitive, not interoperability of a SCRAM wire exchange.

## Unicode License V3

Copyright © 1991-2026 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY DOWNLOADING,
INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR SOFTWARE, YOU
UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE TERMS AND CONDITIONS
OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT DOWNLOAD, INSTALL, COPY,
DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a copy
of data files and any associated documentation (the "Data Files") or software
and any associated documentation (the "Software") to deal in the Data Files
or Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, and/or sell copies of the Data
Files or Software, and to permit persons to whom the Data Files or Software
are furnished to do so, provided that either (a) this copyright and permission
notice appear with all copies of the Data Files or Software, or (b) this
copyright and permission notice appear in associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF THIRD
PARTY RIGHTS. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN
THIS NOTICE BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL
DAMAGES, OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA FILES OR
SOFTWARE. Except as contained in this notice, the name of a copyright holder
shall not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written authorization
of the copyright holder.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
