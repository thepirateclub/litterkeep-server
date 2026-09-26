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

## Ограничение normalization

Приём mailbox names отклоняет unassigned code points закреплённой версии,
запрещённые RFC 9755 controls и начальный BOM. Исходная строка сохраняется.
NFC normalization для исходящих mailbox names не реализована; RFC 5198
формулирует NFC как SHOULD. Поэтому эта таблица закрывает обязательную
assigned-version проверку, но не весь Unicode/Net-Unicode profile и сама по
себе не разрешает рекламировать `UTF8=ACCEPT`.

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
