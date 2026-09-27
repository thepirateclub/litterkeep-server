# Unicode data profile

## Закреплённая версия

Mailbox validator использует таблицы unassigned scalar values из Unicode
Character Database 17.0.0 `UnicodeData.txt`. Эта версия совпадает с
`char::UNICODE_VERSION` установленного Rust 1.97.1 и runtime image `rust:1.97`.
Compile-time assertion в `src/imap/unicode_17.rs` запрещает сборку, если
встроенная версия Unicode изменится без обновления таблиц.

Официальный источник:
`https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt`.
SHA-256: `2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C`.
Локальная acceptance-копия находится в `./data/g19-unicode17/UnicodeData.txt`.
Она не является runtime-файлом сервера и не включается в бинарник.

`tools/generate_unicode17_unassigned.ps1` строит отсортированные непересекающиеся
complement ranges и записывает shards в `src/imap/unicode_17_ranges/`. Скрипт
проверяет SHA-256 исходника и лимит 300 строк на shard. Для повтора нужны
PowerShell, закреплённый UCD-файл и rustfmt установленного toolchain:

```powershell
.\tools\generate_unicode17_unassigned.ps1
cargo test --locked imap::unicode_17::tests
cargo test --locked imap::mailbox_utf8::tests
```

Runtime использует binary search по включительным диапазонам. Нет Unicode crate
или production dependency сверх allowlist. Private-use scalars остаются
assigned и принимаются: RFC 5198 рекомендует избегать их, но не запрещает их.

## NFC normalization для Net-Unicode

Приём mailbox names отклоняет unassigned code points закреплённой версии,
запрещённые RFC 9755 controls и начальный BOM. NFC-модуль расположен в общем
Unicode-слое, поэтому протокольные границы и Store используют одну версию
алгоритма. В режиме UTF8=ACCEPT входящие имена канонизируются перед обращением
к хранилищу, а IMAP mailbox responses сериализуются тем же алгоритмом.

Для записей из версий сервера до NFC новая версия не переписывает folder или
subscription journals. Store ищет старое имя по NFC-эквиваленту, возвращает
каноническое имя в списках и позволяет rename/delete/unsubscribe через alias.
Стабильный folder ID остаётся прежним, поэтому mailbox data key не меняется.
Если в старом каталоге обнаружены два имени с одинаковой NFC-формой, точечное
разрешение имени возвращает InvalidInput, а не выбирает одну папку случайно.
Unit-тесты проверяют replay старого folder journal через полный Store reopen,
сохранность содержимого и alias-операции; независимый IMAP STARTTLS сценарий
проверяет старое modified UTF-7 имя через новое UTF8=ACCEPT соединение.

Таблицы NFC построены из официальных Unicode 17.0.0 `UnicodeData.txt`,
`DerivedNormalizationProps.txt` и `NormalizationTest.txt`. Генератор
`tools/generate_unicode17_nfc.ps1` закрепляет SHA-256 каждого входа, исключает
composition exclusions, генерирует canonical decomposition/combining class/
composition таблицы и делит их на Rust shards менее 300 строк. Алгоритм в
`src/imap/unicode_17_nfc.rs` обрабатывает Hangul алгоритмически, остальные
последовательности - через отсортированные таблицы и binary search. Новых
runtime dependencies нет.

Полный официальный corpus прошёл отдельной командой на закреплённых таблицах:

```powershell
$env:LITTERKEEP_UNICODE_NORMALIZATION_TEST = (Resolve-Path data/g20-unicode17-normalization/NormalizationTest.txt).Path
cargo test --locked unicode::nfc::tests::official_unicode_normalization_vectors -- --ignored --exact
```

Все обычные NFC unit vectors входят в стандартный `cargo test`. Независимый
STARTTLS black-box проверяет decomposed CREATE, composed LIST response,
SELECT по composed имени и DELETE по decomposed имени. Полный Store reopen и
wire test проверяют доступ к сохранённой decomposed folder записи по NFC alias;
journals не переписываются. RFC 5198 формулирует
нормализацию исходящих строк как SHOULD; реализация NFC не закрывает остальные
Net-Unicode response paths и сама по себе не разрешает рекламировать
`UTF8=ACCEPT`.

## IDNA2008 derived properties для SMTP

`smtp::idna17` использует официальный Unicode 17.0.0 файл
`idna/Idna2008.txt`, формирующий производные свойства по RFC 5892. Источник:
`https://www.unicode.org/Public/17.0.0/idna/Idna2008.txt`.
SHA-256: `E4A7526A8A37539C0DEFA4DA25F5DBF77D0212A14D4762D455CADEA608A8921C`.
Локальная копия для воспроизводимой генерации хранится в
`./data/g40-idna17/Idna2008.txt`, не поставляется в runtime и не включается
в бинарник.

PowerShell-генератор `tools/generate_idna17_ranges.ps1` проверяет SHA-256,
извлекает только PVALID/CONTEXTJ/CONTEXTO и делит отсортированные диапазоны
на Rust shards размером не более 300 строк. Проверка каждого символа выполняет
binary search по этим shards. Для повтора нужны файл из `./data` и PowerShell:

```powershell
.\tools\generate_idna17_ranges.ps1
cargo test --locked smtp::idna17::tests
```

Отдельный ignored oracle test читает тот же закреплённый corpus и сверяет
Derived Property для каждого Unicode scalar, включая ContextJ, ContextO и
объединённые disallowed/unassigned значения. Его полный повтор:

```powershell
$env:LITTERKEEP_IDNA2008_FILE = (Resolve-Path data/g40-idna17/Idna2008.txt).Path
cargo test --locked smtp::idna17::conformance_tests::derived_property_matches_official_unicode_17_corpus -- --ignored --exact
Remove-Item Env:LITTERKEEP_IDNA2008_FILE
```

Пройденное сравнение охватывает 1,112,064 scalar values. Оно проверяет только
таблицу категорий RFC 5892, не полную label/domain conformance для NFC, hyphen,
ContextO/ContextJ, bidi, Punycode и A-label round-trip.

Это только lookup RFC 5892 Derived Property. PVALID не означает, что полный
label соответствует RFC 5891: NFC, ограничения дефиса, контекстные правила,
bidi и A-label round-trip должны выполняться верхним IDNA-слоем.
`smtp::idna17::to_alabel` применяет NFC, hyphen rules, PVALID lookup, RFC 3492
кодирование и 63-октетный предел кодированного label. ContextO использует
Unicode 17 Scripts.txt: Greek, Hebrew, Hiragana, Katakana и Han ranges нужны
для проверок U+0375/U+05F3/U+05F4/U+30FB. Middle dot и взаимоисключающие
Arabic digit sets проверяются непосредственно по RFC условиям. Генератор
`tools/generate_idna17_scripts.ps1` закрепляет SHA-256 источника и создаёт
отсортированную таблицу из 124 диапазонов не более 300 строк.
Официальный источник: `https://www.unicode.org/Public/17.0.0/ucd/Scripts.txt`.
SHA-256: `9F5E50D3ABAEE7D6CE09480F325C706F485AE3240912527E651954D2D6B035BF`.
Копия лежит в `./data/g40-idna17/Scripts.txt` и нужна только для повторной
генерации; runtime от неё не зависит.

ContextJ (ZWJ/ZWNJ) использует `DerivedJoiningType.txt` и canonical combining
class 9 из `UnicodeData.txt`. ZWNJ принимает непосредственно предшествующий
virama либо L/D + Transparent* + ZWNJ + Transparent* + R/D; ZWJ требует
непосредственно предшествующий virama. Генератор `tools/generate_idna17_joining.ps1`
закрепляет обе версии SHA-256, создаёт joining ranges shards не более 300 строк
и сжатую virama таблицу.

Источники Unicode 17.0.0:
`https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedJoiningType.txt`
с SHA-256 `F39EBE974825D6736AEE15582250307AA532B2CFAB3CAF3F86BD23FDDC9C5C4D`,
и `https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt` с SHA-256
`2E1EFC1DCB59C575EEDF5CCAE60F95229F706EE6D031835247D843C11D96470C`.
Fixture файлы лежат в `./data/g40-idna17/`; они не требуются во время работы
сервера и служат только воспроизводимой генерации.

RFC 5893 применяется на уровне домена: наличие R, AL или AN активирует правило
для всех labels, включая соседние ASCII/LTR. Входящие ASCII `xn--` labels
декодируются, проверяются как U-label и обязаны совпасть с повторным
Punycode-кодированием. `to_alabel_domain` также преобразует U-label домены в
ASCII DNS form. RFC 5891 запрет ведущих combining marks обеспечен таблицей
General_Category Mn/Mc/Me из Unicode 17 `UnicodeData.txt` (тот же закреплённый
SHA-256 источника NFC/virama). Генератор
`tools/generate_idna17_marks.ps1` воспроизводит 327 ranges в двух source shards.
SMTP path parser применяет проверку домена IDNA2008: Unicode U-label
преобразуются в ASCII A-label до локальной маршрутизации. SMTPUTF8 capability
объявляется вместе с 8BITMIME, а MAIL reverse-path с Unicode требует valueless
параметр SMTPUTF8. Независимый Linux STARTTLS-клиент проверил передачу письма
на адрес с U-label, доставку и чтение вложения через IMAP; это не проверка
приложения Litterkeep и не подтверждение удалённой DNS/MX-доставки.

Unit test Punycode codec теперь проверяет все 19 sample strings RFC 3492 §7.1
в обоих направлениях. Для русского примера I используется написание после
RFC Editor Erratum 3026. Вектор S проверяет Bootstring для произвольного ASCII
и не является допустимой IDNA-меткой. Эти sample vectors не заменяют полную
IDNA2008 label/domain conformance suite: полный официальный corpus всё ещё не
выполнен.

Дополнительно сохранён официальный Unicode 17 `IdnaTestV2.txt` из UTS #46:
`./data/g41-idna-v2/IdnaTestV2.txt`, SHA-256
`BEB5D0BE20E896189B03209A82FDC34F06351502BBD4B8E2523583FC2954D9CF`, URL
`https://www.unicode.org/Public/17.0.0/idna/IdnaTestV2.txt`. Для восстановления
локального ignored fixture:

```powershell
New-Item -ItemType Directory -Force data/g41-idna-v2 | Out-Null
Invoke-WebRequest -Uri 'https://www.unicode.org/Public/17.0.0/idna/IdnaTestV2.txt' -OutFile data/g41-idna-v2/IdnaTestV2.txt
Get-FileHash data/g41-idna-v2/IdnaTestV2.txt -Algorithm SHA256
```

Этот
файл не является чистым IDNA2008 oracle: он включает UTS #46 mappings,
transitional processing и более широкий набор статусов. Поэтому отдельный
ignored test сравнивает только пересечение профилей: NFC input, RFC 5892
PVALID labels (для A-label проверяется decoded U-label), пустые статусы
nontransitional ToUnicode и ToASCII и те же ASCII результаты. На закреплённом файле прошли
172 строки; две записи с surrogate code units явно пропущены, остальные
непринадлежащие strict-пересечению строки не засчитываются. Это не подтверждает
UTS #46 mappings, CONTEXTJ/CONTEXTO полную conformance, transitional режим или
полный IDNA2008 label/domain профиль.

Повтор выборочного cross-profile oracle:

```powershell
$env:LITTERKEEP_IDNA_TEST_V2_FILE = (Resolve-Path data/g41-idna-v2/IdnaTestV2.txt).Path
cargo test --locked smtp::idna17::idna_test_v2::strict_idna_matches_valid_nfc_pvalid_uts46_rows -- --ignored --exact --nocapture
Remove-Item Env:LITTERKEEP_IDNA_TEST_V2_FILE
```

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
