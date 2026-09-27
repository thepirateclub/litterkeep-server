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

ContextJ (ZWJ/ZWNJ) пока возвращает `ContextRuleRequired`; это закрытая часть
IDNA-профиля, а не полный валидатор. Проверка bidi и входящих A-label не
сделана. SMTP parser эту функцию пока не вызывает и SMTPUTF8 не рекламирует.

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
