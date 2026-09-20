# Goal ledger

Root Goal ID: `01a0ba65-3922-7d21-ac75-0963ef79819a`  
Root status: ACTIVE  
Current subgoal: G06

The actually available Goal interface was inspected. It supports creating and
updating one active root goal (`create_goal`, `get_goal`, `update_goal`), but
has no subgoal/dependency operation. The G00-G12 rows below are therefore the
authoritative local subgoal ledger, rather than invented Goal API calls.

| Goal | Depends on | Acceptance criterion | Status |
|---|---|---|---|
| G00 | none | build, help, lock, dependency report, saved goal | PASS |
| G01 | G00 | lifecycle/lease/wake tests and Linux poll integration | PASS |
| G02 | G01 | durable store, journal, blob, recovery | PASS |
| G03 | G02 | TLS, AUTH, bounded admission | PASS |
| G04 | G03 | durable SMTP submission and local delivery | PASS |
| G05 | G04 | MIME + IMAP, independently checked attachment | PASS |
| G06 | G05 | IMAP folders, flags, UID, IDLE | RUNNING |
| G07 | G02 | POP3 backed by the shared store | PASS |
| G08 | G02 | Sieve and ManageSieve delivery effect | RUNNING |
| G09 | G04 | outbound SMTP, pool, retry/DSN | RUNNING |
| G10 | G06,G07,G08,G09 | failure matrix, replay, resource plateaus | NOT RUN |
| G11 | G10 | offline check/backup and repeatable seed | NOT RUN |
| G12 | G11 | litterkeep and final acceptance report | NOT RUN |

## Checkpoints

### 2026-09-20: G06 SEARCH date and boolean checkpoint

- SEARCH AST поддерживает `BEFORE`, `ON`, `SINCE`, `NOT`, бинарный `OR` и
  вложенные parenthesized groups. Последовательность criteria внутри и вне
  группы остаётся неявным AND по IMAP4rev1, незакрытая или лишняя скобка
  отвергается parser-ом.
- Date parser вынесен в отдельный typed `SearchDate`: wire-строка проверяется
  существующей Gregorian/UTC арифметикой `MailDate`, после чего executor
  сравнивает только номер календарного дня с сохранённым INTERNALDATE.
- Unit tests покрывают parser tree, невалидную дату, границы ON/BEFORE/SINCE и
  short-circuit OR/NOT. Неподдержанные SENT-date и прочие keys всё ещё получают
  BAD, поэтому профиль не выдаёт неполную семантику за пустой успех.
- PASS: Windows 171 tests, strict Clippy и format. Docker Rust 1.97 strict
  Linux Clippy и 197 tests прошли в финальном image
  `litterkeep-server:g06-search-boolean-date-final`.
- G06 остаётся RUNNING: расширение остальных базовых flag/date criteria и
  независимая сетевая SEARCH/concurrent/reconnect matrix ещё не завершены.

### 2026-09-20: G06 typed content SEARCH checkpoint

- SEARCH больше не ограничен пятью flag predicates. Typed AST поддерживает
  `FROM`, `TO`, `CC`, `BCC`, `SUBJECT`, произвольный `HEADER`, `BODY`, `TEXT`,
  `LARGER`, `SMALLER` и неявный AND нескольких последовательных criteria для
  обычного и UID SEARCH. Неизвестный criterion и отсутствующий аргумент
  по-прежнему дают BAD, а не ложный пустой SEARCH.
- Сопоставление работает над immutable bytes согласованного mailbox snapshot.
  ASCII-поиск регистронезависим; BODY исключает header block, TEXT включает всё
  сообщение. MIME header block разбирается не более одного раза на сообщение и
  только для запроса, которому нужны header/body boundaries.
- PASS: Windows 169 tests, strict Clippy и format. Docker Rust 1.97 strict
  Linux Clippy и 194 tests прошли в image
  `litterkeep-server:g06-search-profile`.
- G06 остаётся RUNNING: date criteria, отрицание/OR и независимый сетевой
  SEARCH/concurrent/reconnect matrix ещё не завершены.

### 2026-09-20: G06 IDLE fan-out and Recent ownership checkpoint

- RFC 3501 оставляет неопределённым, какая из одновременно selected sessions
  увидит новое письмо с `\Recent`. Профиль продукта фиксирует более простую
  проверяемую политику: все уже selected sessions получают session-local
  marker вместе с durable EXISTS; durable кандидат остаётся до первого
  будущего SELECT, который атомарно claim-ит его. Следующий SELECT marker уже
  не получает, поэтому `\Recent` не становится persistent message flag.
- Интеграционный Store/reactor test подтверждает live notification двум
  владельцам, однократный durable claim будущим SELECT и пустой следующий
  SELECT. Отдельный профиль создаёт 32 настоящих socket lease в одном
  Orchestrator, доставляет каждому независимый bounded EXISTS buffer и
  проверяет ровно 32 task/resource без session threads.
- PASS: Windows 167 tests, strict Clippy и format. Docker Rust 1.97 strict
  Linux Clippy и 191 tests прошли в image
  `litterkeep-server:g06-idle-matrix`.
- G06 остаётся RUNNING: расширенный SEARCH профиль и независимая сетевая
  concurrent/reconnect matrix ещё не завершены; Docker bind mount `./data`
  на этой машине остаётся непригодным для честного black-box запуска.

### 2026-09-20: G06 isolated IMAP IDLE overflow checkpoint

- Unsolicited IMAP notifications больше не возвращают общую reactor error при
  переполнении bounded session output. Отстающий подписчик получает typed
  `SessionTermination::NotificationOverflow`; его неотправленные буферы
  очищаются, а lease закрывается единым путём lifecycle cleanup после dispatch.
- Остальные selected/IDLE-сессии продолжают получать durable EXISTS, FLAGS,
  EXPUNGE и COPY/MOVE projection. Отдельный Linux test заполняет очередь одного
  подписчика до `MAX_OUTPUT`, проверяет корректный EXISTS у второго и доказывает
  удаление только одного task/resource owner без утечки metadata.
- PASS: Windows `cargo test` (167 tests), strict Clippy и format. Docker Rust
  1.97 strict Linux Clippy и 189 tests прошли в image
  `litterkeep-server:g06-idle-overflow-final`.
- Docker Desktop file sharing перепроверен фактическим `alpine:3.22` bind mount
  `H:/github/mail-server/data:/data`. Обращение к `/data` снова зависло, а
  daemon не смог завершить контейнер в штатный срок. Поэтому сетевой black-box
  сценарий не объявлен выполненным, G06 остаётся RUNNING.

### 2026-09-20: G08 redirect route and loop policy checkpoint

- Общий `redirect_policy` разбирает mailbox в typed `Domain::PeerTest`,
  проверяет bounded dot-atom и канонизирует domain. Первый локальный профиль
  разрешает только `peer.test`; `litterkeep.test`, неизвестный domain и
  malformed address не могут попасть в outbound queue.
- Store применяет policy после bounded Sieve execution. Запрещённый route или
  восемь уже существующих `Received` fields отбрасывают весь частичный план,
  выполняют implicit keep и атомарно публикуют versioned Sieve diagnostic.
- Sieve projection теперь читает ordered MIME header fields напрямую, поэтому
  повторные `Received` не схлопываются `BTreeMap` и loop budget проверяет
  фактическую цепочку trace headers.
- Outbound worker повторяет ту же policy до получения connection из pool.
  Повреждённый/старый queue job не открывает socket; typed `InvalidInput`
  становится terminal `PermanentFailure` с DSN `5.7.1 RoutePolicy`, а не
  расходует transient retry budget.
- PASS: Windows `cargo test --all-targets --locked` (167 tests), strict Clippy,
  format. Docker Rust 1.97 strict Linux Clippy и 188 tests прошли в image
  `litterkeep-server:g08-redirect-policy-gate`.
- G08 остаётся RUNNING только до независимого сетевого runtime-error/loop
  matrix. Его запуск всё ещё невозможен на `./data` из-за зависшего runtime
  bind mount Docker Desktop; unit/integration доказательство не выдано за
  black-box PASS.

### 2026-09-20: G08 transactional Sieve runtime-error checkpoint

- Bounded executor возвращает typed `ExecutionError`, а не строку. При runtime
  error частично построенный DeliveryPlan отбрасывается полностью и применяется
  RFC-safe implicit keep, поэтому уже принятое письмо не теряется и redirect из
  недовыполненного плана не публикуется.
- Диагностика содержит account, точную version активного script, transaction ID
  и typed error code. Она записывается до общей Store commit-точки и при replay
  фильтруется по committed transaction IDs вместе с mailbox/queue state.
  Оборванная transaction поэтому не показывает ложную ошибку пользователю.
- Добавлена offline-команда `litterkeep-server sieve-errors <data-dir>
  <account>` со стабильным TSV-выводом. Она фактически запущена на локальном
  `./data/g08-sieve-diagnostics`; пустой новый Store вернул только корректный
  header, не создавая фиктивных записей.
- Store tests вызывают реальный runtime redirect limit: исходное письмо попадает
  в INBOX, outbound queue остаётся пустой, версия и причина переживают reopen.
  Failpoint после mailbox frame доказывает, что до global commit не видны ни
  письмо, ни предварительно записанная Sieve diagnostic.
- Повторный RCPT одного account дедуплицируется через `HashSet` до исполнения
  правила, поэтому не создаёт несколько одинаковых redirect jobs или ошибок.
- Recovery обрезает только оборванный tail diagnostic journal до выдачи writer,
  но отклоняет завершённый повреждённый frame. После ошибки записи Store
  становится poisoned и не дописывает данные за неопределённым tail.
- Diagnostic journal имеет явную квоту 1 MiB. Её исчерпание останавливает
  transaction до global commit и SMTP success, сохраняя проверяемую семантику.
- PASS: Windows `cargo test --all-targets --locked` (161 tests), strict Clippy и
  format. Docker Rust 1.97 strict Linux Clippy и 182 tests прошли в image
  `litterkeep-server:g08-runtime-error-gate`.
- G08 остаётся RUNNING: независимый сетевой runtime-error test не запускался,
  поскольку Docker Desktop bind mount по-прежнему зависает на `init /data`.
  Осталась отдельная проверка redirect route/loop policy и полный replay matrix.

### 2026-09-20: G08 Sieve implicit keep semantics checkpoint

- Исполнитель Sieve теперь хранит typed-состояние `Implicit`, `Cancelled` или
  `Explicit`. Действия `redirect`, `fileinto` и `discard` отменяют только
  implicit keep; ранее выполненный explicit `keep` сохраняется независимо от
  порядка последующих delivery actions.
- Исправлена ошибочная Store-проверка, которая считала INBOX обязательным для
  одиночного `redirect`. Теперь redirect-only transaction создаёт durable
  outbound job без mailbox link, сохраняет исходный envelope sender и после
  terminal transition не восстанавливает job при reopen.
- Отдельный Store test подтверждает составной план `keep; redirect`: INBOX link
  и outbound job фиксируются вместе и оба восстанавливаются после повторного
  открытия Store. Unit test языка дополнительно проверяет сочетание explicit
  keep, redirect и fileinto.
- Тесты Store вынесены в отдельный предметный файл, поэтому production-файл
  снова укладывается в лимит 300 строк без `mod.rs` и без смешивания тестовой и
  рабочей ответственности.
- PASS: Windows `cargo test --all-targets --locked` (156 tests), strict Clippy,
  format и `git diff --check`. Docker Rust 1.97 strict Linux Clippy и 177 tests
  прошли в image `litterkeep-server:g08-implicit-keep-gate`.
- G08 остаётся RUNNING: ещё не завершены полный runtime-error/replay matrix и
  независимый сетевой redirect-only сценарий без дополнительного `discard`.
  Его нельзя объявлять PASS по одним unit/integration тестам; runtime bind mount
  Docker Desktop ранее зависал даже после включения file sharing.

### 2026-09-20: G06 IMAP AUTHENTICATE PLAIN checkpoint

- Устранена ложная capability: `AUTHENTICATE PLAIN` теперь имеет typed
  mechanism, optional SASL initial response, continuation state, явную отмену
  `*`, общий authzid policy и асинхронную проверку тем же Argon2 Store worker,
  что LOGIN/SMTP/POP3/ManageSieve. Tagged completion различает LOGIN и
  AUTHENTICATE через enum, а не строковое восстановление команды.
- LOGIN и AUTHENTICATE запрещены до реального TLS и после уже успешной
  аутентификации. Plaintext listener больше не рекламирует `AUTH=PLAIN`.
  `SASL-IR` объявляется в TLS capability, поскольку initial response реально
  принимается и проверяется.
- Общий входной dispatcher отделён от IMAP command dispatcher; SASL
  continuation обрабатывается до обычной command grammar и остаётся
  собственностью одной session. Это сохраняет file-level ответственность и
  лимит 300 строк.
- Добавлена фактическая матрица rev1 в `docs/imap-profile.md`: реализованные
  команды, ограниченный SEARCH профиль и явно не заявленные расширения.
- PASS: Windows `cargo test --all-targets --locked` (154 tests), strict Clippy,
  format. Docker Rust 1.97 strict Linux Clippy и 175 tests прошли в image
  `litterkeep-server:g06-imap-auth-gate`. Linux integration использует реальный
  TCP socket, event wake через `poll`, account journal и Argon2 worker; отдельно
  проверены успешный continuation, cancel, plaintext denial и повторный вход.
- Независимый `tests/blackbox/imap_authenticate.py` проверяет обе wire-формы и
  capability до/после STARTTLS; `py_compile` PASS, сетевой запуск NOT RUN из-за
  зафиксированного зависания runtime bind mount Docker Desktop.
- G06 остаётся RUNNING до завершения всей concurrent-change/error matrix и
  независимой сетевой проверки AUTHENTICATE после восстановления bind-mount
  runtime Docker Desktop.

### 2026-09-20: G06 COPY/MOVE multi-session synchronization checkpoint

- После успешного durable COPY/MOVE reactor обновляет destination projection
  каждой уже selected сессии: увеличивает EXISTS и добавляет destination UID в
  session-local `\\Recent`. IDLE-получатель немедленно получает агрегированный
  `EXISTS`, обычная selected-сессия сохраняет согласованное внутреннее состояние.
- MOVE дополнительно уменьшает EXISTS source-сессий, удаляет перенесённые UID
  из их `\\Recent` и публикует рассчитанные source sequence numbers как
  `EXPUNGE` ожидающим IDLE-сессиям. Инициатор исключён только из source
  broadcast, поскольку его completion уже содержит эти EXPUNGE; destination
  projection инициатора при совпадении mailbox не теряется.
- Поиск source sequence numbers переведён с повторного линейного `Vec::contains`
  на `BTreeSet`, поэтому построение проекции не становится квадратичным.
- PASS: Windows `cargo test --all-targets --locked` (153 tests), strict Clippy,
  format, `git diff --check`, отсутствие `mod.rs` и Rust-файлов длиннее 300
  строк. Docker Rust 1.97 strict Linux Clippy и 172 tests прошли в image
  `litterkeep-server:g06-transfer-sync-gate`; новый тест использует три
  настоящих TCP socket и три эксклюзивных lifecycle lease.
- Независимый сетевой сценарий `tests/blackbox/imap_idle_move.py` подготовлен и
  проходит `py_compile`, но NOT RUN по протоколу. После разрешения file sharing
  финальный image собрался, однако новый `docker run --rm -v
  H:\\github\\mail-server\\data\\g06-idle-move:/data ... init /data bob
  bob-password` завис до вывода результата, как и прежний runtime-контейнер.
  Поэтому unit/integration доказательство не выдано за сетевой black-box PASS.
- G06 остаётся RUNNING: требуется завершить оставшиеся классы concurrent-change
  matrix и реально выполнить подготовленный COPY/MOVE watcher после
  восстановления runtime bind-mount I/O Docker Desktop.

### 2026-09-20: G09 scheduler TLS success and SAN failure checkpoint

- Linux integration test запускает настоящий постоянный scheduler, durable
  Store worker, bounded TLS transport worker и отдельный rustls SMTP peer.
  Успешный trusted route проходит queue `Pending -> InFlight -> Delivered`,
  после reopen Store больше не содержит pending job.
- Второй тест использует доверенный CA, но неверный SNI. rustls отклоняет SAN,
  неопределённое соединение уничтожается, а scheduler фиксирует durable
  `Deferred`, `attempts=1` и ненулевой restart-safe retry deadline.
- Оба сценария завершаются cooperative cancellation scheduler task, закрытием
  job channel, `join` transport thread и повторным открытием Store.
- PASS: Windows `cargo test --all-targets --locked` (153 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 171 test в image
  `litterkeep-server:g09-scheduler-tls-gate`. Новые Linux tests используют
  реальные TCP/rustls sockets, а не mock результата transport-а.
- G09 остаётся RUNNING: нужен внешний обмен двух одновременно запущенных
  service instances на отдельных `./data` и доказательство pool reuse в этом
  black-box процессе. Текущий Docker Desktop shared mount всё ещё занят старым
  зависшим контейнером, поэтому этот отдельный сценарий не объявлен PASS.

### 2026-09-20: G09 typed TLS route pool checkpoint

- Outbound worker принимает `Route::Plain` либо `Route::ImplicitTls`; variant
  выбирается до запуска thread. Enum-owned `Pool` физически не позволяет
  вернуть rustls stream в plaintext pool или смешать разные transport types.
- Implicit-TLS connection теперь остаётся non-cloneable владельцем rustls
  stream между доставками. Route key закрепляет числовой address, SNI и CA;
  channel возвращается в pool только после successful DATA либо accepted RSET.
- Постоянный reactor scheduler принимает typed route. Новый CLI
  `serve-all-mail-outbound-tls-starttls` загружает outbound CA отдельно от
  входящего server certificate и запускает тот же lifecycle/queue state machine.
- Unit test провёл две SMTP transaction через один TLS connection и подтвердил
  один idle channel. Linux scheduler test продолжает проверять plaintext variant.
- PASS: Windows `cargo test --all-targets --locked` (153 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 169 tests в image
  `litterkeep-server:g09-tls-route-gate`. Format и structural gate выполнены
  перед commit.
- G09 остаётся RUNNING до внешнего обмена двух service instances и негативной
  проверки wrong CA/SAN именно через постоянный scheduler.

### 2026-09-20: G09 bounded outbound network deadlines checkpoint

- Plaintext и implicit-TLS transport используют один connector contract:
  route разбирается в typed `SocketAddr` до worker thread, TCP connect получает
  deadline 5 секунд, read/write получают deadline 10 секунд.
- Hostname route отклоняется как `InvalidInput`, поэтому внутри scheduler-а нет
  скрытого blocking DNS. `ConnectionPool` хранит уже проверенный `SocketAddr` и
  не разбирает строку заново для каждой доставки.
- Детерминированный peer-test принимает TCP, не отправляет greeting и ждёт EOF.
  Клиент получает системный timeout через 100 ms test profile, уничтожает
  неопределённый channel, после чего peer наблюдает реальное закрытие без sleep.
- PASS: Windows `cargo test --all-targets --locked` (152 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 168 tests в image
  `litterkeep-server:g09-deadline-gate`. Format, `git diff --check` и structural
  gate выполнены перед commit.
- G09 остаётся RUNNING: постоянный scheduler ещё использует plaintext route;
  нужны typed TLS route key и двухсерверная проверка service mode.

### 2026-09-20: G09 cooperative cancellation checkpoint

- Local control protocol и CLI получили typed команду
  `runtime cancel <task-id>`. `request_cancel` снимает registrations ожидания,
  но не выдаёт занятый ресурс повторно и не удаляет task с живым lease.
- Reactor после обработки готовой poll-пачки закрывает отменённую клиентскую
  сессию через общий `session_close`: socket, POP3 maildrop lease, resource и
  task metadata освобождаются одним владельцем. Отложенный Store completion
  всё ещё коррелируется по generation и безопасно отбрасывается.
- Outbound scheduler в idle завершается сразу. При активной операции отмена
  дренирует текущую доставку до durable terminal transition, затем закрывает
  bounded worker channel, присоединяет thread и только после этого удаляет task.
- PASS: Windows `cargo test --all-targets --locked` (150 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 166 tests, включая реальное закрытие
  отменённого socket lease, в `litterkeep-server:cancel-bind-gate-final`.
  Format, `git diff --check` и structural gate также пройдены.
- BLOCKED: новое разрешение Docker file sharing проверено успешным read-only
  запуском с `-v H:/github/mail-server/data:/data`, но write/init остаётся в
  `Created`. Старый контейнер всё ещё завис внутри прежнего shared mount, поэтому
  bind-mount black-box отмены не объявлен PASS до перезапуска Docker Desktop.
- G09 остаётся RUNNING: нет deadline для отмены зависшего transport I/O, TLS
  route для постоянного scheduler-а и полного межсерверного service matrix.

### 2026-09-20: G09 scheduler lifecycle ownership checkpoint

- Постоянный outbound scheduler теперь создаёт реальную task запись в общем
  `Orchestrator<Session>`, а не только пишет логи или держит локальный enum.
- Каждая Store и SMTP transport операция паркует task на собственном
  `Event::Completion(request_id)`. Completion сначала выполняет versioned
  `signal`, проверяет переход task в `Ready`, затем следующая фаза снова
  регистрирует ожидание. Пустая очередь использует отдельный typed
  `Event::Timer(TaskId)` и пробуждается общим poll deadline.
- Linux integration test после запуска и после полной доставки проверяет
  `waiting_tasks=1`, `wait_registrations=1` и `ready_queue_depth=0`. Тот же тест
  подтверждает реальную SMTP-транзакцию и отсутствие pending job после reopen.
- Listener binding/регистрация вынесены в отдельный файл одной ответственности;
  все Rust/Python файлы остаются не длиннее 300 строк, `mod.rs` отсутствует.
- PASS: Windows 149 tests, strict Clippy и format; Docker Rust 1.97 strict Linux
  Clippy и 163 tests в image `litterkeep-server:lifecycle-gate-2`.
- G09 остаётся RUNNING: lifecycle task пока нельзя отменить через control API,
  а завершение process всё ещё не присоединяет transport worker управляемо.

### 2026-09-20: G09 long-lived reactor scheduler checkpoint

- `serve-all-mail-outbound-starttls` запускает один постоянный single-flight
  scheduler в общем Linux `poll` loop. Scheduler использует тот же Store
  worker, отдельный bounded SMTP transport owner и отдельный wake fd.
- Store completions больше не теряются: session dispatcher возвращает
  не принадлежащие session ответы, а scheduler принимает их только при точном
  совпадении верхнего диапазона request ID и текущей typed-фазы. Transport
  completion также проверяет job ID перед durable transition.
- Пустая очередь переводит scheduler в timed idle. Общий poll deadline выбирает
  минимум TLS handshake и следующего queue scan; отдельный timer-thread не
  создаётся. После terminal transition или DSN scheduler немедленно запрашивает
  следующую due delivery.
- Linux integration test `scheduler_drains_without_explicit_queue_command`
  создал durable job, провёл его через настоящий TCP SMTP peer, wakeup Store и
  transport workers и подтвердил после повторного открытия Store отсутствие
  pending job без вызова `queue-drain`.
- PASS: Windows 149 tests, strict Clippy и format; Docker Rust 1.97 gate теперь
  обязательно устанавливает Clippy и выполнил strict Linux Clippy плюс 163
  теста. Все Rust/Python файлы не превышают 300 строк, `mod.rs` отсутствует.
- Попытка дополнительного bind-mount black-box выявила зависание Docker Desktop
  внутри Unix `bind(2)` на shared volume. Control socket переведён на безопасный
  `/tmp` socket с owner-only locator в `./data`. Уже зависший контейнер Docker
  Desktop не смог завершить даже через `docker rm -f`, поэтому этот внешний
  сценарий не объявлен пройденным; доказательством checkpoint является Linux
  integration test внутри image build.
- G09 остаётся RUNNING: ещё нужны scheduler cancellation/clean shutdown,
  TLS route policy для постоянного сервиса и обмен двух одновременно работающих
  экземпляров именно через новый service mode.

### 2026-09-20: G10 local control socket foundation

- Linux reactor теперь владеет bounded Unix-domain control listener-ом и
  короткими nonblocking control-сессиями в том же `poll` loop, что SMTP, IMAP,
  POP3, ManageSieve и Store wakeup. Для диагностики не создаётся отдельный
  поток и не открывается второй Store writer.
- CLI добавил `runtime stats`, `runtime tasks`, `runtime resources` и
  `runtime inspect <task-id>` с обязательным `--data-dir`. Ответы строятся из
  реальных records `Orchestrator`: task state, owner и held resource IDs;
  значения ресурсов, тела почты, пароли и ключи не сериализуются.
- Control admission ограничен 16 одновременными session slots; лишний local
  peer закрывается без роста таблицы и без вытеснения уже обслуживаемого запроса.
- Docker Desktop shared bind mount не поддерживает Unix socket inode. Это
  выявлено фактическим Linux запуском, а не скрыто: только при таком bind error
  server создаёт owner-only locator `runtime.control` к process-local socket в
  `/tmp`; CLI проверяет, что locator не выходит из временного каталога.
- PASS: Linux Docker image `litterkeep-server:control-runtime` запущен с
  `./data/g10-control`; независимый `docker exec` выполнил `runtime stats`,
  `tasks`, `resources` и `inspect 2:1` при удерживаемой SMTP session. Ответ
  показал `TASK id=2:1 state=Ready held=2:1` и соответствующий leased resource;
  locator имел права `0600`. До final gate выполнены Windows `cargo test
  --all-targets --locked` (149 tests), strict Clippy, format и structural audit.
- G10 остаётся NOT RUN: зависимости G06/G08/G09 ещё не завершены; отсутствуют
  stop/cancel/restart counters, bounded event ring, полный failure/replay matrix
  и resource plateau acceptance.

### 2026-09-20: G08 ManageSieve literal and shared-reactor checkpoint

- ManageSieve получил собственный ограниченный RFC 5804 literal receiver для
  `PUTSCRIPT` и `CHECKSCRIPT`. Он принимает `{n+}` и синхронизирующий `{n}`
  до 65 536 байтов, не выделяет память по неограниченной длине и сохраняет
  pipeline-байты следующей команды. Это не копия IMAP APPEND: completion
  наступает ровно после n байтов без вымышленного завершающего CRLF.
- Добавлен `serve-all-mail-starttls`: SMTP, IMAP, POP3 и ManageSieve работают
  в одном Linux poll-reactor с одним Store owner. Отдельные процессы не
  конкурируют за файловый lock и не дают ложной проверки общей семантики.
- PASS для checkpoint: Windows `cargo test --all-targets --locked` (117
  tests), strict Clippy, format и structural audit без `mod.rs` и Rust-файлов
  больше 300 строк; Docker Linux test target прошёл 124 tests. Независимый Python socket
  сценарий `tests/blackbox/managesieve_literal_delivery.py` проверил STARTTLS,
  SASL PLAIN, `CHECKSCRIPT {n}` continuation, fragmented `PUTSCRIPT {n+}` и
  `SETACTIVE`, затем standard-library SMTP отправил Alice -> Bob, а IMAP
  прочитал письмо из `Invoices`. Использован только bind mount
  `./data/g08-managesieve`, CA `./data/pki/ca.pem`.
- G08 остаётся RUNNING: базовый RFC Sieve ещё не покрывает все обязательные
  tests/comparators/actions, redirect не связан с outbound queue, а полный
  внешний ManageSieve matrix всех команд и response codes ещё не завершён.

### 2026-09-20: G08 Sieve elsif checkpoint

- Parser представляет `elsif` вложенным `Command::If` в единственной
  else-ветви. Поэтому executor использует прежний bounded алгоритм и единый
  лимит операций, а следующие ветви не исполняются после первого совпадения.
- PASS: Windows `cargo test --all-targets --locked` (118 tests), strict
  Clippy, format и `git diff --check`. G08 остаётся RUNNING: ещё требуются
  address/size, match types/comparators, redirect через outbound queue и
  полный внешний ManageSieve matrix.

### 2026-09-20: G08 Sieve matcher and size checkpoint

- AST теперь явно хранит enum match type (`:is`, `:contains`, `:matches`) и
  comparator (`i;ascii-casemap`, `i;octet`), а не восстанавливает семантику
  из строк во время доставки. Matcher вынесен в отдельный bounded компонент:
  glob не использует regex и не делает экспоненциальный backtracking.
- `header` принимает списки имён и ключей, `size :over`/`:under` сравнивает
  именно число октетов исходного письма, переданное Store до journal commit.
  Неподдержанный comparator, переполнение и неверный quantifier отклоняются
  до публикации script version.
- PASS: Windows `cargo test --all-targets --locked` (121 tests), strict
  Clippy, format и Docker Linux test target (128 tests). G08 остаётся
  RUNNING: отсутствуют `address`-условия, redirect ещё не идёт через outbound
  queue, нужен полный внешний ManageSieve command/error matrix.

### 2026-09-20: G08 Sieve address checkpoint

- `address` test получил отдельный bounded extractor для `:all`, `:localpart`,
  `:domain`, `:user` и `:detail`; только затем выбранные значения передаются
  тому же typed matcher-у. Quoted display name и angle-bracket mailbox не
  ломают delimiter comma, некорректный адрес не становится совпадением.
- PASS: Windows `cargo test --all-targets --locked` (123 tests), strict
  Clippy, format и Docker Linux test target (130 tests). G08 остаётся
  RUNNING: redirect ещё не проходит outbound queue, не проверены все команды
  и response codes ManageSieve независимым сетевым matrix.

### 2026-09-20: G08 ManageSieve positive command matrix

- Независимый socket-клиент теперь исполняет по TCP/TLS CAPABILITY, STARTTLS,
  AUTHENTICATE, HAVESPACE, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, SETACTIVE,
  GETSCRIPT, RENAMESCRIPT, DELETESCRIPT, NOOP и LOGOUT. Он читает
  multi-line LIST и literal GETSCRIPT как wire-протокол, а не через наш parser.
- PASS: Linux Docker image с единым `serve-all-mail-starttls` reactor-ом;
  положительный matrix завершился SMTP Alice -> Bob и IMAP-чтением `Invoices`.
  Данные: bind mount `./data/g08-managesieve-matrix`, CA `./data/pki/ca.pem`.
  После проверки контейнер остановлен. G08 остаётся RUNNING: ещё нет
  отрицательного matrix response codes и redirect не подключён к durable
  outbound queue.

### 2026-09-20: G08 ManageSieve negative command matrix

- Тот же независимый TCP-клиент подтвердил отказ AUTHENTICATE до STARTTLS,
  HAVESPACE сверх лимита, невалидного PUTSCRIPT и DELETESCRIPT действующей
  версии. После неуспешной загрузки LISTSCRIPTS/GETSCRIPT всё ещё возвращают
  исходную active `invoices`, затем SMTP применяет её к доставке.
- PASS: Linux Docker `serve-all-mail-starttls`, отдельный bind mount
  `./data/g08-managesieve-negative`, CA `./data/pki/ca.pem`; тестовый
  контейнер остановлен. G08 остаётся RUNNING только из-за неинтегрированного
  redirect/outbound queue и полного RFC Sieve failure/replay coverage.

### 2026-09-20: G08 advanced Sieve network fixture

- Независимый uploaded script теперь реально использует `allof`, `header
  :contains`, `address :detail :is` и `size :over`. SMTP envelope recipient
  остаётся `bob@litterkeep.test`, а header `To` содержит plus-detail, поэтому
  проверка не подменяет Sieve routing разрешением другого RCPT account.
- PASS: Linux Docker SMTP -> Store -> IMAP прочитал письмо в `Invoices` из
  bind mount `./data/g08-sieve-advanced-network-3`; client serializes RFC 5322
  с CRLF через standard-library `email.policy.SMTP`. Windows полный suite:
  124 tests, strict Clippy и format. G08 остаётся RUNNING из-за redirect queue
  и replay/transaction integration.

### 2026-09-20: G08 envelope sender handoff checkpoint

- `AppendMany` теперь переносит SMTP `MAIL FROM` как отдельный
  `envelope_sender` от reactor через bounded storage worker к Store. Значение
  взято до `smtp_trace::prepend`, не выводится из MIME `From` и сохраняет
  допустимый null reverse-path. Это готовит корректный sender для Sieve
  redirect и исключает подмену envelope данными письма.
- PASS: Windows `cargo test --all-targets --locked` (124 tests), strict Clippy,
  format и `git diff --check`. G08 остаётся RUNNING: durable queue frame должен
  войти в тот же Store transaction commit, затем требуются replay и SMTP peer
  проверки redirect.

### 2026-09-20: G09 transaction-aware queue format checkpoint

- Durable queue теперь имеет typed `OwnedDelivery`: transaction ID immutable
  blob, envelope sender, recipient и enum state. `pending_owned` возвращает
  job только когда его ID присутствует в Store global committed set; тест
  подтверждает невидимость физически записанного, но незафиксированного job.
- PASS: Windows `cargo test --all-targets --locked` (125 tests), strict Clippy,
  format и `git diff --check`; `src/queue.rs` 263 lines. G08/G09 остаются
  RUNNING: Store ещё должен записывать owned jobs до общего commit, recovery
  должен использовать committed IDs, затем redirect и SMTP transport worker.

### 2026-09-20: G08 redirect and Store queue transaction checkpoint

- Store владеет queue и записывает Sieve redirect job после durable blob, но до
  единственного global commit. Новый unit test подтверждает: локальный INBOX
  link и `OwnedDelivery` становятся видимыми вместе, job хранит SMTP envelope
  sender, а не MIME `From`. `src/store/append.rs` отделяет эту ответственность
  от Store model, все файлы остаются не длиннее 300 строк.
- PASS: Windows `cargo test --all-targets --locked` (126 tests), strict Clippy,
  format и `git diff --check`. G08/G09 остаются RUNNING: recovery должен
  загрузить committed jobs для worker-а, затем нужны delivery state transitions,
  retry и независимый SMTP peer.

### 2026-09-20: G09 committed queue recovery checkpoint

- Store сохраняет recovered global committed transaction IDs и обновляет набор
  только после успешной global commit publication. `pending_outbound` отдаёт
  queue jobs через этот набор; restart test проверяет, что committed Sieve
  redirect снова доступен после `Store::open`.
- PASS: Windows `cargo test --all-targets --locked` (126 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: jobs ещё не получают
  durable state transitions и не передаются исходящему SMTP worker/peer.

### 2026-09-20: G09 durable owned queue transitions checkpoint

- `OwnedDelivery` получил append-only `transition_owned`; при recovery reader
  выбирает последнее состояние transaction/recipient и не выдаёт повторно
  `Delivered` job. Тест подтверждает это через reopen. Тесты вынесены из
  production queue journal: `queue.rs` 226 lines, `queue_tests.rs` 91 lines.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: Store должен предоставить
  worker-у transition command и blob read, затем нужен реальный SMTP peer.

### 2026-09-20: G09 Store-owned transport handoff checkpoint

- Store получил typed API `pending_outbound`, `outbound_blob` и
  `transition_outbound`. Они принимают только committed `OwnedDelivery`, не
  раскрывают worker-у путь blob и сохраняют queue state через владельца Store.
  Unit test прошёл pending -> immutable payload -> Delivered -> restart без
  повторной выдачи terminal job.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: storage worker protocol
  и SMTP transport worker ещё не используют этот handoff against real peer.

### 2026-09-20: G09 bounded worker outbound protocol checkpoint

- Bounded `StorageWorker` получил typed requests/responses для pending jobs,
  immutable blob и state transition. Единственный Store owner исполняет их,
  а reactor correlation ID покрывает все новые variants, поэтому completion не
  теряется и transport не получает Store/file access напрямую.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`; `message.rs` 258 lines, `execution.rs` 284.
  G09 остаётся RUNNING: отдельный bounded SMTP transport worker ещё не делает
  TCP peer transaction и не переводит результаты в эти completion requests.

### 2026-09-20: G09 SMTP outcome classification checkpoint

- Собственный outbound SMTP client возвращает typed `DeliveryOutcome`:
  positive completion, `Deferred` для SMTP 4xx и `PermanentFailure` для 5xx.
  Системные I/O ошибки остаются `io::Error`; CLI явно показывает оба вида
  отказа. Это даёт очереди корректное решение о durable следующем state.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: outcome ещё не вызывает
  Store worker transition в bounded queue-drain loop и не проверен peer-ом.

### 2026-09-20: G09 bounded queue drain and crash recovery checkpoint

- Добавлен `outbound_queue::drain`: он берёт не больше указанного числа
  committed jobs, фиксирует `InFlight`, запускает собственный SMTP transport и
  записывает `Delivered`, `Deferred` либо `PermanentFailure`. I/O ошибка peer
  становится `Deferred`, не потерей письма. Recovery mapping переводит stale
  `InFlight` в `Deferred`; reopen test это подтверждает.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format и `git diff --check`; queue files не длиннее 300 lines. G09 остаётся
  RUNNING: drain ещё не имеет CLI/service entrypoint и не проверен против
  независимого Linux SMTP peer.

### 2026-09-20: G09 queue drain CLI checkpoint

- CLI получил `queue-drain <data-dir> <peer-address> [limit]`: открывает
  единственный Store writer, запускает bounded drain и печатает durable
  delivered/deferred/permanent counters. Нулевой или некорректный limit
  отвергается до доступа к хранилищу; default limit равен 8.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format и `git diff --check`; `main.rs` 292 lines. G09 остаётся RUNNING до
  независимой Linux SMTP peer проверки queue-drain и service integration.

### 2026-09-20: G09 Linux outbound completion mapping checkpoint

- Linux Docker release build обнаружил неполный cfg-gated `Response` mapping
  reactor-а для новых outbound completions. `completion_id` теперь exhaustively
  покрывает все variants без wildcard; образ `litterkeep-server:g09-queue-runtime`
  собран из текущего исходного кода.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format, `git diff --check` и Linux Docker release build. G09 остаётся RUNNING:
  независимый SMTP peer queue-drain ещё не выполнен.

### 2026-09-20: G09 Linux queue drain peer acceptance

- Независимый `queue_redirect_seed.py` по ManageSieve STARTTLS загрузил
  `redirect "remote@peer.test"` и отправил SMTP письмо source instance. После
  остановки source writer `queue-drain /data peer:2526 1` в отдельном Linux
  container выдал `delivered=1 deferred=0 permanent_failures=0`. Независимый
  Python `imaplib` container прочёл `queue redirect peer` из peer `remote`
  mailbox. Route policy теперь принимает documented peer domain `peer.test`.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format, Linux Docker release build и два listener/container peer scenario.
  Все state data только в bind mounts `./data/g09-peer-source` и
  `./data/g09-peer-dest`; тестовые контейнеры остановлены. G09 остаётся
  RUNNING: отсутствуют automatic service drain, retry schedule/TTL, TLS peer
  transport и connection pool reuse/cancellation acceptance.

### 2026-09-20: G09 outbound 4xx/5xx wire peer checkpoint

- Отдельный standard-library `TcpListener` peer получил реальный SMTP greeting,
  EHLO, MAIL и RCPT от нашего client-а, затем вернул 450 или 550. Regression
  tests подтверждают `DeliveryOutcome::Deferred` для 4xx и
  `PermanentFailure` для 5xx, без DATA после RCPT rejection. Tests вынесены в
  `outbound_tests.rs`, production transport остаётся 125 lines.
- PASS: Windows `cargo test --all-targets --locked` (129 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: отсутствует persistent
  retry schedule/TTL и automatic service drain/pool.

### 2026-09-20: G09 durable retry budget checkpoint

- Owned queue frames теперь несут `attempts`; reader сохраняет backward
  compatibility с прежним форматом без счётчика. Drain увеличивает attempts
  перед durable `InFlight`, поэтому crash не обнуляет budget. Третья 4xx/I/O
  deferred попытка становится `PermanentFailure`, что подтверждено unit test.
- PASS: Windows `cargo test --all-targets --locked` (130 tests), strict Clippy,
  format, `git diff --check`; queue/transport files не длиннее 300 lines. G09
  остаётся RUNNING: есть bounded count, но ещё нет persisted next-at/TTL delay,
  automatic service drain и TLS/pool lifecycle peer acceptance.

### 2026-09-20: G08 redirect-only durable delivery checkpoint

- Sieve `redirect; discard;` теперь публикует durable outbound job без
  фиктивной INBOX link. Store разрешает пустой список mailbox links только при
  непустом outbound plan; обычный пустой APPEND по-прежнему получает
  `InvalidInput`. Queue job, blob и global commit остаются одной transaction.
- PASS: Windows `cargo test --all-targets --locked` (131 tests), strict Clippy,
  format и `git diff --check`. G08/G09 остаются RUNNING: redirect-only peer
  scenario ещё не проверен в Linux; автоматический retry timing, TLS и pool
  остаются неполными.

### 2026-09-20: G08/G09 redirect-only Linux recovery checkpoint

- Recovery отличает redirect-only transaction от повреждённого commit: ID без
  mailbox link допустим только при owned SMTP job; blob такой transaction
  проверяется при открытии Store. Это устраняет ошибку после restart перед
  `queue-drain`, не ослабляя проверку повреждённых committed mail transactions.
- PASS: Linux image `litterkeep-server:g09-redirect-discard`, source и peer
  используют bind-mounted `./data/g09-redirect-*`. ManageSieve загрузил
  `redirect "remote@peer.test"; discard;`, SMTP принял письмо, source Store
  после restart drained его как `delivered=1`, а независимый Python IMAP
  клиент прочитал его у Linux peer. В source mailbox links нет, queue journal
  содержит terminal `delivered`. Windows: 131 tests, strict Clippy, format и
  `git diff --check` PASS. G08/G09 остаются RUNNING: нет automatic delayed
  retry, outbound TLS, pool и cancellation acceptance.

### 2026-09-20: G09 owned SMTP pool checkpoint

- `ConnectionPool` владеет настоящими TCP channels одного маршрута и имеет
  capacity 2 для bounded `queue-drain`. После positive final DATA channel
  возвращается в idle и применяется к следующему job; SMTP reject, EOF и I/O
  уничтожают channel и освобождают slot, поэтому грязное соединение не
  выдаётся новому recipient. Разовый CLI `deliver` остаётся отдельным
  standalone transport.
- PASS: детерминированные peer tests подтвердили два сообщения на одном TCP
  connection и новую TCP accept после RCPT 450. Windows: 133 tests, strict
  Clippy, format и `git diff --check`; Linux Docker test stage: 140 tests.
  G09 остаётся RUNNING: pool пока живёт только в одном bounded drain, нет
  runtime service ownership, persisted next-at/backoff, TLS route keys, DSN
  и cancellation lifecycle acceptance.

### 2026-09-20: G09 durable retry deadline checkpoint

- Owned queue frames теперь сохраняют `next_attempt_epoch` как restart-safe
  Unix epoch. После deferred delivery используется bounded exponential backoff
  60/120/240... seconds до 3600 seconds; terminal state не получает deadline.
  `queue-drain` не выдаёт job раньше deadline. Reader сохраняет совместимость
  с прежними owned frames, присваивая им immediate retry, а crash в InFlight
  возвращает job в Deferred без ложной задержки.
- PASS: restart test подтверждает due-gate на границе epoch, отдельный test
  читает прежний attempts frame. Windows: 136 tests, strict Clippy, format и
  `git diff --check`; Linux Docker test stage: 143 tests. G09 остаётся
  RUNNING: не реализованы service-owned automatic drain, TLS route keys,
  DSN и cancellation lifecycle acceptance.
- `Store::due_outbound` закрепляет due-отбор за единственным владельцем queue:
  transport больше не читает и не фильтрует durable schedule самостоятельно.

### 2026-09-20: G09 outbound TLS transport checkpoint

- Добавлен отдельный implicit-TLS SMTP transport с явными `server-name` и
  DER CA параметрами. Он строит свой rustls client config, проверяет SNI/CA и
  только после TLS handshake выполняет SMTP greeting, EHLO, MAIL, RCPT и DATA.
  Plaintext pool с ним не смешивается, поскольку TLS identity является другим
  route key. CLI `deliver-tls` делает этот путь доступным без runtime Python.
- PASS: собственный rustls TLS peer с выданным на лету CA принял полный SMTP
  transaction; malformed DER CA отвергается до TCP-подключения. Windows: 138
  tests, strict Clippy, format и `git diff --check`; Linux Docker test stage:
  145 tests. G09 остаётся RUNNING: TLS ещё не
  подключён к durable pool/service route, также отсутствуют automatic drain,
  DSN и cancellation lifecycle acceptance.

### 2026-09-20: Linux POP3 ownership correction

- Linux release build обнаружил cfg-specific перенос POP3 `Command` при
  проверке STLS до передачи команды state machine. Проверка теперь сопоставляет
  заимствованную команду, поэтому ownership остаётся у единственного `apply`.
- PASS: Windows `cargo test --all-targets --locked` (138 tests), strict Clippy,
  format и `git diff --check`; Docker Linux test stage: 145 tests. Контекст
  Docker равен 83 KiB и не содержит `./data`; runtime данные остаются только в
  явно подключаемых bind mounts из `./data`.

### 2026-09-20: G09 SMTP pool reset checkpoint

- После известного SMTP 4xx/5xx pool больше не уничтожает channel только по
  классу ответа. Он читает полный ответ, отправляет `RSET` и возвращает TCP
  connection в idle исключительно после положительного полного RSET reply.
  EOF, I/O error и отказ RSET уничтожают channel до следующего recipient.
- PASS: отдельный TCP peer подтвердил reuse одного соединения после RCPT 450
  и `RSET 250`; отдельный peer подтвердил новый accept после `RSET 500`.
  Windows `cargo test --all-targets --locked`: 139 tests; strict Clippy,
  format и `git diff --check` PASS. Docker Linux test stage: 146 tests.
- G09 остаётся RUNNING: TLS route keys/service-owned drain, DSN, cancellation
  lifecycle и независимый сторонний MTA ещё не проверены.

### 2026-09-20: G09 local DSN checkpoint

- `queue-drain` после durable `PermanentFailure` строит multipart/report DSN
  с `message/delivery-status` и исходным RFC822 body. Пустой reverse-path и
  внешний либо отсутствующий local account не создают bounce. 5xx и исчерпанный
  retry budget выбирают разные typed причины; публикацию выполняет только
  Store owner после account/local-route проверки.
- PASS: Linux Docker scenario с bind mount `./data/g09-dsn-source`: независимый
  ManageSieve/SMTP client создал redirect job от Alice, независимый socket peer
  вернул RCPT `550`, `queue-drain` выдал `permanent_failures=1`, а независимый
  Python IMAP STARTTLS client прочитал у Alice `report-type=delivery-status`,
  `Action: failed` и исходный subject. Тестовые Docker container/network
  остановлены и удалены; данные и peer log сохранены в stage directory.
- PASS: Windows `cargo test --all-targets --locked` (146 tests), strict Clippy,
  format и `git diff --check`; Docker Linux test stage: 153 tests. Structural
  audit: нет `mod.rs`, Rust files не длиннее 300 lines, запрещённых составных
  `if` не найдено. G09 остаётся RUNNING: DSN пока выполняется через explicit
  `queue-drain`, TLS route keys/service lifecycle, cancellation и сторонний MTA
  ещё не приняты.

### 2026-09-20: G09 bounded outbound worker checkpoint

- `OutboundWorker` владеет единственным `ConnectionPool` в выделенном thread и
  принимает только immutable SMTP jobs через bounded `sync_channel`. Worker не
  получает `Store`, не создаёт thread на job и возвращает completion без
  блокирования reactor. Явный `shutdown` закрывает jobs channel и присоединяет
  владельца transport ресурса.
- PASS: независимый TCP peer принял ровно одно TCP-соединение и подтвердил два
  последовательных SMTP jobs на этом connection. Windows
  `cargo test --all-targets --locked`: 147 tests; strict Clippy, format,
  `git diff --check` и structural audit PASS. Docker Linux test stage: 154
  tests.
- G09 остаётся RUNNING: worker ещё не подключён к reactor/durable queue;
  отсутствуют service wakeup, cancellation, TLS route keys и acceptance со
  сторонним MTA.

### 2026-09-20: G09 outbound completion wakeup checkpoint

- Linux `WakingOutboundWorker` создаёт Unix wake socket одновременно с
  bounded transport worker-ом. После каждого completion поток пытается
  записать только coalesced hint; authoritative result остаётся в channel.
  Поэтому заполненный wake socket не теряет outcome, а reactor не опрашивает
  transport в холостом цикле. Shutdown сначала присоединяет владельца pool.
- PASS: Linux unit test через собственный `poll(2)` подтвердил readable wake
  socket, вычитал hint и получил `Delivered` completion. Windows
  `cargo test --all-targets --locked`: 147 tests, strict Clippy, format,
  `git diff --check` и structural audit PASS. Docker Linux test stage: 155
  tests.
- G09 остаётся RUNNING: wakeup ещё не включён в server loop и не получает
  durable jobs; сохраняются обязательные service scheduler, cancellation,
  TLS route keys и независимый MTA acceptance.

### 2026-09-20: G09 bounded scheduler contract checkpoint

- Retry state, restart-safe deadline и DSN reason вынесены в единую typed
  `outbound_policy`; `queue-drain` больше не содержит отдельную копию этих
  решений. Store worker получил `OutboundDue`, который возвращает не более
  явно запрошенного лимита committed due jobs. Следующий service scheduler
  будет запрашивать только свою свободную capacity у Store owner.
- PASS: новый Store test создал две durable redirect-only jobs и подтвердил
  выдачу ровно одной при `limit=1`. Windows `cargo test --all-targets --locked`:
  148 tests; strict Clippy, format и `git diff --check` PASS. Docker Linux
  test stage: 156 tests.
- G09 остаётся RUNNING: контракт ещё не означает запуск scheduler-а в server
  loop. Требуются durable transition before submit, processing completion,
  service wakeup/timer, cancellation, TLS route keys и independent MTA test.

### 2026-09-20: G09 event-driven service drain checkpoint

- Linux `queue-drain` теперь использует `outbound_service`, а не открывает
  Store рядом с transport loop. Service создаёт bounded Store worker и
  bounded outbound pool worker, ожидает только их Unix wake sockets, durable
  фиксирует `InFlight` до SMTP submit и фиксирует final state до DSN. Один
  job является единственным mutable phase владельцем, поэтому completion не
  может быть применён к другому recipient.
- PASS: Linux integration test создаёт committed outbound job, независимый
  TCP peer принимает SMTP transaction, service получает wake completions и
  после final transition reopened Store не видит pending job. Docker Linux
  test stage: 157 tests. Windows: 148 tests, strict Clippy, format,
  `git diff --check` and current structural audit PASS.
- G09 остаётся RUNNING: это bounded explicit drain, не long-lived scheduler
  в `server_loop`; future retry timer, cancellation policy, TLS route keys и
  independent two-server acceptance ещё не завершены.

### 2026-09-20: G09 independent service drain DSN acceptance

- Под Linux Docker создана отдельная stage directory
  `./data/g09-service-blackbox`. Независимые Python ManageSieve и SMTP clients
  загрузили `redirect "remote@peer.test"; discard;` и передали письмо source
  server-у. После остановки source writer контейнерный `queue-drain /data
  host.docker.internal:3555 1` использовал event-driven service owners.
- PASS: независимый `smtp_550_peer.py` получил обычные EHLO, MAIL, RCPT 550 и
  RSET, а CLI сообщил `delivered=0 deferred=0 permanent_failures=1`. После
  restart source server независимый `queue_dsn_read.py` через IMAP STARTTLS
  прочитал multipart/report с `Action: failed` и исходным subject. Test
  container остановлен и удалён; данные и peer logs остаются в указанной
  `./data` directory.
- Этот PASS доказывает explicit service drain и DSN, но не long-lived runtime
  scheduler: G09 остаётся RUNNING до server-loop lifecycle, cancellation,
  TLS route keys и independent two-server acceptance.

### 2026-09-20: G10 lifecycle snapshot foundation

- `Orchestrator::stats` строит read-only snapshot только из актуальных task,
  resource, ready queue и wait registrations. Snapshot не удерживает lease,
  не пишет секреты и не создаёт историю terminal records. Unit test проверяет
  leased/quarantined resource и parked task на настоящих lifecycle transitions.
- PASS: Windows `cargo test --all-targets --locked` 149 tests, strict Clippy,
  format и `git diff --check`. G10 остаётся NOT RUN: нет local control socket,
  runtime CLI, plateau/failure matrix и Linux acceptance данного интерфейса.

### 2026-09-20: lifecycle parking correction

- Snapshot обнаружил, что `park_on` оставлял Waiting task в ready queue.
  Переход теперь удаляет task из ready queue только после атомарной проверки
  event version и регистрации wait. `signal` остаётся единственным путём,
  который вернёт parked task в ready state. Это восстанавливает требование
  освобождать execution slot во время ожидания.
- PASS: Windows 149 tests, strict Clippy, format и `git diff --check`.

### 2026-09-20: correction of scheduler contract structure

- Первый commit scheduler contract временно увеличил `src/store/tests.rs` до
  321 physical lines. Нарушение найдено тем же обязательным audit до передачи
  результата. Тест вынесен в `store/outbound_tests.rs`; история не
  переписывалась, чтобы факт и исправление были проверяемы отдельным commit.
- Current structural audit обязан показать не более 300 lines для каждого Rust
  file перед следующим checkpoint.

### 2026-09-20: structural condition-rule compliance checkpoint

- Устранены все найденные в `src` условия `if` с `&&` или `||`: ветвления
  выражены через typed tuples, `match` и guards. Для reactor read/write guards
  сохранено ленивое выполнение I/O, поэтому отсутствие readiness не вызывает
  лишнего read/flush. Проверка не нашла `mod.rs`, файлов длиннее 300 lines или
  прямых array-index expressions.
- PASS: Windows 137 tests, strict Clippy, format и `git diff --check`.

### 2026-09-20: Sieve capability enum checkpoint

- `require` преобразуется parser-ом в typed `Capability`; validation проверяет
  `Capability::Unsupported`, а не сравнивает предметное состояние со строкой.
- PASS: Windows 137 tests, strict Clippy, format и `git diff --check`.

### 2026-09-20: G07 POP3 core foundation

- Заглушка POP3 заменена typed grammar и FSM AUTHORIZATION/TRANSACTION/UPDATE.
  Реализованы USER/PASS, STAT, LIST, RETR, DELE, NOOP, RSET, QUIT, UIDL, TOP,
  CAPA и STLS на session-local `Maildrop` snapshot. UIDL строится из durable
  mailbox UID, RETR/TOP делают dot-stuffing, а QUIT возвращает delete plan и
  не объявляет UPDATE до отдельного completion `commit_deletes`.
- PASS только для core: Windows `cargo test --all-targets --locked` (108
  tests), strict Clippy, format, structural audit и Docker Linux test target.
  G07 не отмечен PASS:
  Linux POP3 listener, auth/store worker, maildrop lease, durable QUIT и
  независимый poplib test ещё не реализованы.

### 2026-09-20: G07 reactor loop extraction checkpoint

- Poll-loop SMTP/IMAP вынесен из фасада reactor в отдельный
  `reactor/server_loop.rs`. Владелец socket leases, wakeup, deadline и
  cancellation остаётся тем же: это структурная подготовка к третьему POP3
  listener без создания потоков на соединение и без изменения wire-семантики.
- PASS: Windows `cargo test --all-targets --locked` (108 tests), strict
  Clippy, format, `git diff --check`, audit Rust-файлов (не более 300 строк),
  Linux release build и Docker Linux test target (112 tests).
- G07 остаётся NOT RUN: сетевой POP3 listener, auth/store worker, maildrop
  lease, durable QUIT и независимый poplib test ещё не реализованы.

### 2026-09-20: G07 SMTP to POP3/STLS checkpoint

- Общий Linux poll reactor теперь может владеть только выбранными listener-ами,
  поэтому `serve-pop3-starttls` использует те же session leases, wakeup
  storage-worker и TLS deadline без потока на POP3-соединение. POP3 PASS
  создаёт worker request, успешная аутентификация открывает atomарный snapshot
  каталогового INBOX, а DELE становится durable EXPUNGE только после QUIT.
- Исправлена общая delivery граница: SMTP local recipient теперь разрешается в
  его durable INBOX key до `append_many`. Поэтому SMTP, IMAP и POP3 читают одну
  mailbox projection, а не разные строковые ключи.
- PASS: Windows `cargo test --all-targets --locked` (109 tests), strict
  Clippy, format, `git diff --check`, audit Rust-файлов (не более 300 строк),
  Linux release image и Docker test target (113 tests). Независимый standard
  library `tests/blackbox/pop3_starttls_delivery.py` выполнил SMTP STARTTLS
  Alice -> Bob, POP3 `CAPA`/STLS/USER/PASS/UIDL/RETR/DELE/QUIT и сравнил байты
  MIME attachment; отдельный POP3 restart подтвердил durable удаление. Данные:
  bind mount `./data/g07-pop3`, CA `./data/pki/ca.pem`.
- G07 остаётся NOT RUN: maildrop lease для конкурентных POP3 sessions,
  implicit-TLS POP3 и расширенный interoperability/failure matrix ещё не
  реализованы. Этот checkpoint не объявляет их проверенными.

### 2026-09-20: G07 maildrop lease and implicit-TLS checkpoint

- POP3 account получает отдельную bounded maildrop lease только после worker
  authentication. Она принадлежит `(slot, generation)`, не удерживает Store и
  освобождается после durable QUIT, TCP EOF, poll error или TLS deadline.
  Поэтому второй POP3 client не получает тот же snapshot, но SMTP/IMAP и
  storage-worker продолжают работать независимо.
- Добавлен `serve-pop3-tls`: implicit TLS не выдаёт plaintext greeting до
  handshake, переиспользуя существующие poll, deadline и socket lease.
- PASS: Windows `cargo test --all-targets --locked` (109 tests), strict
  Clippy, format, `git diff --check`, structural audit. Docker Linux test
  target прошёл 114 tests. Независимый `poplib` подтвердил конкурентный отказ
  второй POP3 session и получение maildrop после EOF первой, а `POP3_SSL`
  подтвердил implicit TLS с доверенным CA. Данные: `./data/g07-pop3`.
- G07 остаётся NOT RUN: политика конфликтующего IMAP EXPUNGE во время POP3
  snapshot и полный destructive/interoperability matrix ещё не завершены.

### 2026-09-20: G07 POP3 snapshot versus IMAP EXPUNGE checkpoint

- Политика зафиксирована и проверена: POP3 maildrop содержит immutable snapshot
  bytes, поэтому уже открытый RETR остаётся доступен после IMAP EXPUNGE. Однако
  Store не допускает повторный tombstone отсутствующего UID: POP3 QUIT получает
  `-ERR delete commit failed` и не делает ложного обещания. IMAP удаление
  остаётся durable и побеждает конфликт.
- Добавлен `serve-all-starttls`, объединяющий SMTP, IMAP и POP3 listener-ы в
  один poll-reactor и один Store owner для межпротокольных проверок.
- PASS: независимый `tests/blackbox/pop3_imap_conflict.py` на Linux Docker
  открыл POP3 snapshot `pop`, удалил то же письмо через IMAP STORE/EXPUNGE,
  прочитал snapshot через RETR, получил отказ QUIT и подтвердил пустой INBOX.
  Данные: `./data/g07-conflict`, CA: `./data/pki/ca.pem`.
- G07 остаётся NOT RUN: destructive matrix RSET/disconnect до QUIT и полный
  interoperability profile ещё не завершены.

### 2026-09-20: G07 empty-QUIT lease completion checkpoint

- Даже пустой POP3 delete plan теперь передаётся storage worker-у как пустой
  durable EXPUNGE completion. Только после него session переходит в UPDATE и
  освобождает maildrop lease, поэтому QUIT без DELE не оставляет account
  искусственно занятым до TCP EOF.
- PASS: Windows `cargo test --all-targets --locked` (110 tests), strict
  Clippy, format, structural audit и `git diff --check`. Linux regression
  target для этого уточнения ещё не запускался.
- G07 остаётся NOT RUN: требуется Linux destructive matrix RSET и disconnect
  до QUIT, а также полный interoperability profile.

### 2026-09-20: G07 Linux POP3 delete matrix checkpoint

- Независимый POP3 client подтвердил `DELE + RSET` и `DELE + TCP disconnect`:
  оба пути сохраняют сообщение. Затем тот же SMTP-delivered MIME message прошёл
  UIDL/RETR/DELE/QUIT, а restart подтвердил durable удаление. Пустой QUIT также
  прошёл worker completion как часть повторного входа после RSET/disconnect.
- PASS: Linux Docker image с одним `serve-all-starttls` poll-reactor; данные
  только в bind mount `./data/g07-pop3`, CA `./data/pki/ca.pem`.
- G07 остаётся NOT RUN: destructive fixture ещё использует `bob`, тогда как
  цель требует отдельный `pop@litterkeep.test`, и полный external
  interoperability profile не завершён.

### 2026-09-20: G07 PASS

- Отдельный fixture `pop@litterkeep.test` создан как account `pop` и проверен
  независимыми `smtplib`/`poplib` клиентами в Linux Docker. SMTP STARTTLS
  доставил сообщение в общий INBOX; POP3 STLS подтвердил UIDL и RETR, `RSET`
  и TCP disconnect не опубликовали DELE, а QUIT выполнил durable удаление.
- Отдельная maildrop lease проверена на конкурентный отказ и повторную выдачу
  после EOF. Политика IMAP EXPUNGE против POP3 snapshot документирована и
  проверена отдельным client-сценарием без второго Store writer-а.
- PASS: критерий G07 `UIDL/RETR, правильное удаление и освобождение maildrop
  lease` выполнен. Неподдержанные внешние клиенты и расширенная failure matrix
  остаются задачами G10, а не ложным claim о всеобщей interoperability.

### 2026-09-20: G08 Sieve language foundation

- Заглушка `fileinto` заменена собственными отдельными слоями lexer, parser,
  AST, static validation и bounded execution. Поддерживаются comments,
  `require`, `if`/`else`, `true`/`false`, `header :contains`, `exists`,
  `not`, `allof`, `anyof`, `keep`, `discard`, `fileinto`, `redirect` и `stop`.
  Результат исполнения является `DeliveryPlan`, а не записью напрямую в Store.
- Unit-тест доказал обязательное правило `Invoices`: заголовок `[invoice]`
  выдаёт только folder-plan `Invoices`; отдельно проверены implicit keep,
  comments и отказ неподдержанного `require`/unsafe mailbox.
- Проверено локально: `cargo test --all-targets --locked` (112 passed) и
  `cargo clippy --all-targets --locked -- -D warnings`. G08 ещё не завершён:
  требуется versioned script storage, транзакционное применение delivery plan,
  ManageSieve listener и независимый сетевой SMTP -> IMAP сценарий.

### 2026-09-20: G08 versioned script storage checkpoint

- Единственный Store owner теперь хранит scripts в `sieve/<account>.state`.
  PUT сначала parser-ом проверяет source, строит candidate snapshot, fsync-ит
  temporary file и только затем atomically заменяет state. Account, script,
  версия, active flag и quotas ограничены, поэтому ошибочная загрузка не
  заменяет действующий script.
- Unit-тест создал и активировал `invoices`, отверг невалидное обновление,
  перепроверил прежний source и active metadata после restart Store.
- Проверено локально: `cargo test --all-targets --locked` (113 passed),
  `cargo clippy --all-targets --locked -- -D warnings`, формат и лимит файлов.
  G08 остаётся RUNNING: storage должен быть подключён к worker/SMTP delivery,
  после чего нужен самостоятельный ManageSieve listener и внешний сценарий.

### 2026-09-20: G08 transactional delivery-plan checkpoint

- `AppendMany` storage worker теперь перед публикацией message links получает
  active script каждого local recipient у того же Store owner. Sieve plan
  вычисляется до первой записи journal: `fileinto "Invoices"` заменяет
  implicit INBOX, обычное письмо остаётся в INBOX. Redirect пока возвращает
  явную delivery error, не теряя внешний адрес молча до подключения G09 queue.
- Unit-тест активировал invoice-rule, доставил invoice и контрольное письмо,
  затем подтвердил по folder snapshots ровно одно сообщение в `Invoices` и
  одно в `INBOX`. Это Store-level proof, сетевой ManageSieve/SMTP сценарий
  ещё не заявляется выполненным.

### 2026-09-20: G08 ManageSieve FSM foundation

- Добавлено собственное ManageSieve ядро с typed commands, quoted-string
  framing boundary и состояниями plaintext/authenticated/closed. CAPABILITY,
  STARTTLS, AUTHENTICATE PLAIN, LOGOUT, HAVESPACE, PUTSCRIPT, LISTSCRIPTS,
  SETACTIVE, GETSCRIPT, DELETESCRIPT, RENAMESCRIPT, CHECKSCRIPT и NOOP
  представлены отдельными typed действиями; password запрещён до TLS.
- Unit-тесты подтверждают TLS-before-AUTH, смену authenticated state, передачу
  PUTSCRIPT source и строгий отказ лишней arity. Это не является claim о
  listener: TCP literals, storage worker completion и Linux acceptance ещё
  требуются до G08 PASS.

### 2026-09-19: G00 PASS, G01 running

- Added a single Cargo package with no production dependencies. This preserves
  the allowlist while G00/G01 need neither TLS nor password hashing.
- Added the first real lifecycle core, not a telemetry registry: exclusive
  values leave the registry in a non-cloneable `ResourceLease`; release checks
  the current owner; cancellation does not return a live lease; parking uses
  event versions to prevent a lost wakeup.
- PASS: `cargo generate-lockfile`; `cargo fmt --check`; `cargo test --locked`
  (3 passed); `cargo build --locked --release --bin litterkeep-server`;
  `cargo clippy --locked --all-targets -- -D warnings`; `cargo run --locked -- help`;
  `cargo tree --locked -e normal` (only `litterkeep-server`).
- G01 is deliberately still RUNNING. The portable ownership and wake tests pass,
  but Linux `poll(2)` and an actual socket wakeup test are not written yet.
- G02 started: `store::Store` writes each blob to a new temporary path, calls
  `sync_all`, atomically publishes it, then appends and syncs mailbox metadata.
  The restart read test is PASS; writer locking, recovery and queue work remain.
- Repository convention: `mod.rs` is prohibited. The runtime module uses
  `src/runtime.rs` with an explicit path for its implementation file. The
  package now requires the installed Rust 1.97 toolchain and Edition 2024.
- Independent SMTP smoke: PASS on `127.0.0.1:2529` with standard Python
  `smtplib`. It stored `bob-1.eml` (37 bytes) and `bob.journal` (12 bytes).
  The test first exposed case-sensitive envelope prefixes and rejection of the
  standard `SIZE` parameter; both were fixed in our parser before the PASS.
- G01 now contains one Linux-only `serve` loop: it binds SMTP and IMAP in the
  same process, uses the owned `poll(2)` boundary, limits accepting and reading
  work per wake, and asks for writable events only while output is buffered.
  Session slots have generations. Storage request completions are routed by the
  pair, so completion after disconnect or slot reuse is ignored safely.
- Removed the former `serve-smtp` and `serve-imap` blocking commands and their
  independent `Store::open` calls. The binary now exposes one service command,
  so a Windows invocation fails explicitly instead of accidentally bypassing
  lifecycle ownership.
- The reactor does not call `Store`: a single bounded `StorageWorker` owns it.
  Requests are retried only after a completion wake, not by sleeps or a busy
  loop. A full wake socket is harmless because the response channel is the
  completion authority.
- PASS on the installed Windows toolchain: `cargo fmt --check`; `cargo test
  --locked` (15 passed); `cargo clippy --locked --all-targets -- -D warnings`;
  `cargo build --locked --release --bin litterkeep-server`.
- PASS: `cargo check --locked --target x86_64-unknown-linux-gnu` after adding
  the matching standard library target. This caught and fixed two Linux-only
  compile errors that were hidden by the Windows `cfg` path.
- NOT RUN: the Linux reactor itself and all G01 network acceptance cases. This
  host runs Windows, so cross-checking proves type checking only, not runtime
  socket behavior. `serve` reports that limitation on Windows instead of
  selecting the old blocking listeners.

### 2026-09-19: first Linux SMTP to IMAP path PASS

- A Linux/amd64 Docker execution image is available in
  `docker/Dockerfile.linux-test`. It is a verification environment, not a
  production dependency or part of the server topology.
- PASS in the running `litterkeep-g01` container, published as SMTP 2525 and
  IMAP 2143: `python tests/blackbox/smtp_imap_attachment.py 127.0.0.1 2525
  2143`. The independent standard-library `smtplib` client submitted a MIME
  message; the single store worker wrote `bob-1.eml` (749 bytes); independent
  `imaplib` fetched it; Python MIME decoding compared attachment bytes.
  Actual SHA-256: `3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- The client exposed two real IMAP interoperability omissions, `CAPABILITY`
  and read-only `EXAMINE`; both were implemented and retested. This proves the
  first local plaintext path only. It does not prove TLS, authentication
  security, account isolation, full IMAP, POP3, Sieve, outgoing SMTP, or the
  remaining G01 concurrency and cancellation matrix.

### 2026-09-19: G02 recovery frame checkpoint

- Store internals are now separated by responsibility: `model`, `journal`,
  `blobs`, `recovery`, and the owner-only facade. No `mod.rs` was introduced.
- New journal frames contain a version, UID, canonical blob name, and checksum.
  Recovery deletes only temporary blobs and trims only a non-terminated final
  journal tail. A complete corrupt frame remains on disk and produces an
  `InvalidData` error; an orphan blob is not delivered; a referenced missing
  blob is an error. Unit tests cover each case.
- PASS: `cargo fmt --check`; `cargo test --locked` (15 passed); `cargo clippy
  --locked --all-targets -- -D warnings`.

### 2026-09-19: source-boundary enforcement

- Enforced the repository rule that a Rust source file has one responsibility
  and stays within 300 lines. `reactor.rs` now owns the poll loop and session
  resources; `reactor/protocol.rs` owns SMTP/IMAP command state transitions.
  The lease type and orchestrator tests are likewise separate files.
- PASS after this move: format, 15 unit tests, Clippy, Linux target check, and
  the Docker SMTP-to-IMAP attachment checksum test.

### 2026-09-19: G03 crypto dependency and credential checkpoint

- Added only the allowlisted direct dependencies: `rustls 0.23.45` with
  `default-features = false` and the single `ring` provider, plus `argon2
  0.6.0` for Argon2id PHC password records and OS CSPRNG salts. `cargo tree`
  was inspected and the complete graph is locked.
- `auth::Credential` creates and verifies Argon2id PHC records. Its unit test
  proves the correct password is accepted and a different password rejected.
  It is now connected to bounded worker authentication and IMAP session
  completions, but TLS and full admission controls remain incomplete.

### 2026-09-19: IMAP password verification PASS

- `init /runtime` creates the documented fixture account `bob` with password
  `test-password`. Linux Docker `litterkeep-g03` was initialized on the named
  test volume and used for the independent Python test.
- PASS: `imaplib` LOGIN for Bob succeeded during the SMTP-to-IMAP attachment
  SHA-256 test. A second independent `imaplib` call with password `wrong`
  received `NO authentication failed`.
- This checkpoint does not prove TLS, SMTP AUTH, account isolation or bounded
  admission, so G03 remains RUNNING.

### 2026-09-19: Linux writer-lock crash recovery PASS

- Linux storage now keeps a real advisory exclusive `flock(2)` on
  `writer.lock` for the lifetime of the sole `Store` owner. The file itself is
  deliberately retained: the kernel releases the lock when the process exits,
  including an ungraceful termination, while a second live writer is refused.
- PASS in Docker on the named `litterkeep-lock-test` volume: a running server
  was terminated with `docker kill`; `writer.lock` was confirmed to remain;
  another server using the same volume became healthy. The independent Python
  SMTP to IMAP attachment test then passed with SHA-256
  `3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- Windows retains the portable create-new sentinel fallback. It has not been
  validated as crash-safe and is not claimed to have the Linux guarantee.
- G02 remains RUNNING: durable directory metadata syncing, cross-platform
  exclusive locking, quota and queue recovery acceptance are still missing.

### 2026-09-19: IMAP authorization boundary PASS

- `SELECT`, `EXAMINE` and `FETCH` now obtain the mailbox only from the account
  recorded after a successful worker completion. Before `LOGIN` the reactor
  returns tagged `NO authenticate first` and does not enqueue a storage read.
- PASS in the current Linux Docker server with
  `tests/blackbox/imap_requires_login.py`. This is a raw standard-library TCP
  client, not `imaplib`: the latter refuses to send SELECT while it is in its
  own NONAUTH state and therefore cannot be evidence of the server response.
- The independent SMTP to IMAP MIME attachment checksum regression also passed.
  G03 remains RUNNING because the service is still plaintext and does not yet
  have SMTP AUTH, TLS or admission controls.

### 2026-09-19: TLS material boundary PASS

- `tls::ServerMaterial` читает только DER leaf certificate и PKCS#8 private
  key, затем передаёт их непосредственно разрешённому `rustls 0.23.45` для
  валидации пары. Нет самописной криптографии и нет дополнительного PEM crate.
- PASS: unit-тесты подтверждают, что отсутствующие файлы не маскируются, а
  пустой DER не создаёт `ServerConfig`; полный `cargo test --locked` дал 19
  успешных тестов, Clippy с `-D warnings` прошёл.
- Это не TLS-сетевой результат. `ServerConfig` еще не подключён к
  неблокирующему reactor, поэтому TLS handshake, частичные records, таймауты и
  проверка независимым клиентом остаются NOT RUN.

### 2026-09-19: TLS record boundary PASS

- Добавлена `tls_transport::ServerTls`, владеющая только `rustls`
  `ServerConnection`, а не TCP. Она ограничивает encrypted input 64 KiB,
  накапливает неполный TLS record и вызывает `process_new_packets` только для
  полного record. `WouldBlock` при отсутствии plaintext во время handshake не
  закрывает живую сессию.
- PASS: `partial_client_hello_is_buffered` передаёт ClientHello двумя частями,
  подтверждая, что первая часть не попадает в обработку как повреждённый record.
  Полный набор `cargo test --locked` содержит 20 успешных тестов.
- Reactor пока использует plaintext `TcpStream`; соединение `ServerTls` с его
  lifecycle, poll interest, handshake timeout и внешним TLS-клиентом остается
  NOT RUN.

### 2026-09-19: outbound SMTP dialogue checkpoint

- CLI `deliver` читает RFC822-файл и вызывает собственный исходящий SMTP
  transport. PASS: Windows-клиент `cargo run --locked -- deliver` передал
  `tests/fixtures/outbound.eml` работающему Linux Docker серверу на порту 2525;
  после этого независимый SMTP to IMAP MIME-тест снова прошел.
- Неудачная попытка запустить клиент вторым Docker-контейнером через Docker
  Desktop network namespace зависла и не считается приемкой Linux-клиента.
  TLS, durable queue integration, retry и pool для G09 остаются NOT RUN.
- PASS дополнительно: `tests/blackbox/imap_outbound_fixture.py` через
  независимый Python `imaplib` нашёл Subject fixture, переданного `deliver`, и
  сравнил полученные RFC822 bytes с `tests/fixtures/outbound.eml`.

### 2026-09-19: G04 local-recipient routing checkpoint

- SMTP больше не записывает каждое принятое сообщение в жёстко заданный
  mailbox `bob`. После `RCPT` reactor передаёт local-part в ограниченный
  `StorageWorker`; только его проверка существующего account добавляет
  получателя в envelope. Неизвестный или неразбираемый локальный адрес
  получает `550` на RCPT, до DATA и до публикации blob.
- После терминатора DATA worker повторно проверяет весь список получателей,
  затем фиксирует письмо в каждом mailbox и только его completion разрешает
  `250 queued`. Unit-тест доказывает, что неизвестный второй получатель не
  даёт частичной записи. Транзакция забирает список RCPT ровно один раз.
- PASS в отдельном чистом Linux Docker volume
  `litterkeep-recipient-routing-test`: собран образ
  `litterkeep-server:recipient-routing`; созданы `bob/test-password` и
  `alice/alice-password`; `python tests/blackbox/smtp_recipient_routing.py
  127.0.0.1 2527 2145` подтвердил доставку только Alice и `550` неизвестному
  получателю. Затем `python tests/blackbox/smtp_imap_attachment.py 127.0.0.1
  2527 2145` снова подтвердил attachment SHA-256
  `3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- PASS на Windows: `cargo fmt --check`; `cargo test --locked` (24 passed);
  `cargo clippy --locked --all-targets -- -D warnings`. Кросс-проверка target
  `x86_64-unknown-linux-gnu` не выполнена: на host отсутствует
  `x86_64-linux-gnu-gcc`, нужный transitive `ring`; это не заменяет Linux
  Docker build и run, которые прошли.
- G04 остаётся NOT RUN: защищённый submission listener, cleanup оборванной
  DATA с отдельной проверкой, лимит получателей, Received/Return-Path и
  гарантии повторного локального delivery ещё не завершены. G03 также NOT RUN,
  поскольку настоящий TLS и SMTP AUTH не подключены к reactor.

### 2026-09-19: G04 abandoned DATA checkpoint

- Добавлен независимый raw-TCP тест `tests/blackbox/smtp_abandoned_data.py`.
  Он получает ответы на EHLO, MAIL, RCPT и DATA, отправляет незавершённое тело,
  закрывает TCP без терминатора и читает Bob mailbox отдельным `imaplib`.
- PASS в новом чистом Linux Docker volume: тест подтвердил zero messages после
  разрыва, затем маршрут Alice и SMTP-to-IMAP attachment regression снова
  прошли. Разрыв DATA очищает только session buffer и не публикует blob.

### 2026-09-19: G04 basic SMTP command checkpoint

- SMTP FSM различает HELO и EHLO, отвечает на NOOP и принимает RFC-совместимый
  пустой reverse-path `MAIL FROM:<>`; RCPT всё ещё требует непустой path.
- PASS: `cargo test --locked` (25 passed), Clippy и format; в Linux Docker
  независимый `tests/blackbox/smtp_basic_commands.py` подтвердил HELO, NOOP,
  пустой MAIL, RSET и QUIT. SMTP-to-IMAP attachment regression также PASS.

### 2026-09-19: reactor I/O boundary checkpoint

- Выделен `reactor/session_io.rs`: он владеет только неблокирующими TCP read и
  write шагами session. Poll-loop, protocol FSM и storage completion остались
  отдельными владельцами. Это подготавливает точную границу для TLS transport.
- PASS: `reactor.rs` содержит 229 строк, `session_io.rs` 48 строк; format,
  `cargo test --locked` (25 passed) и Clippy прошли. При чтении больше нет
  прямого индексирования массива для фактически принятой длины.

### 2026-09-19: G04 relay-policy and recipient-bound checkpoint

- Envelope ограничен 100 получателями. Лимит проверяется самой SMTP
  транзакцией и не обходится асинхронной проверкой account у storage worker.
- На SMTP boundary domain преобразуется в `LocalRoute`. Только `test` и
  `litterkeep.test` попадают в local delivery policy; внешний домен получает
  `550` на RCPT до DATA. Это не заменяет будущую queue-маршрутизацию peer.
- PASS: `cargo test --locked` (26 passed), format и Clippy. Linux Docker
  `smtp_recipient_routing.py` подтвердил адрес Alice, отказ неизвестного
  mailbox и отказ `alice@outside.test`. Первый запуск теста намеренно не был
  засчитан: старый volume содержал письмо Alice и строгая проверка `== 1`
  упала. После удаления только созданного тестового volume сценарий PASS.

### 2026-09-19: граница локальной delivery-policy

- `reactor/protocol.rs` снова укладывается в лимит 300 строк: обработка SMTP и
  IMAP команд осталась в нём, а единственная ответственность
  `reactor/local_delivery.rs` теперь состоит в преобразовании SMTP path в
  локальный mailbox по ограниченной policy. `reactor.rs` перестал назначать
  session через прямое индексирование `Vec` и получает переиспользуемый слот
  через `get_mut`.
- PASS: `cargo fmt --check`; `cargo test --locked` (26 passed); `cargo clippy
  --locked --all-targets -- -D warnings`; `git diff --check`.
- Это локальное соблюдение правил исходников, а не подтверждение нового
  протокольного рубежа. G03 и G04 сохраняют статус NOT RUN, так как TLS ещё не
  присоединён к reactor.

### 2026-09-19: implicit-TLS reactor checkpoint

- `serve-tls` запускает implicit-TLS listener-ы с документированными default
  портами 1465/1993. Каждая session владеет отдельным `ServerTls`, plaintext
  protocol output и TLS wire output разделены, а SMTP/IMAP greeting не попадает
  в сеть до завершения handshake. Poll interest учитывает `rustls` wants-read
  и wants-write, поэтому TLS не создаёт поток на соединение и не теряет
  partial I/O.
- `examples/make_test_pki.rs` использует разрешённый только в dev-dependency
  `rcgen 0.14.10`: выпуск создаёт CA, отдельный leaf с DNS/IP SAN и DER-ключом
  для `serve`; повторный запуск не заменяет существующий CA. Первый выпуск
  обнаружил одинаковый default DN у CA и leaf, затем исправлены разные subject
  DN и Authority Key Identifier. Эта исправленная цепочка принята строгим
  клиентом.
- PASS в Linux Docker: образ `litterkeep-server:tls-reactor`, volume
  `litterkeep-tls-reactor-test`, listener 2993. Независимый стандартный Python
  `tls_imap_login.py` доверил `ca.pem`, проверил IP SAN `127.0.0.1` и выполнил
  IMAP LOGIN Bob. `tls_imap_rejections.py` подтвердил отказ неизвестного CA и
  неверного пароля. PASS также: `cargo test --locked` (26 passed), Clippy и
  format.
- G03 остаётся NOT RUN: отсутствуют SMTP AUTH, ограничение времени handshake,
  admission control и STARTTLS; результат доказывает только implicit TLS IMAP
  transport и защищённую проверку действующих IMAP credentials.

### 2026-09-19: TLS SMTP AUTH PLAIN submission checkpoint

- SMTP AUTH PLAIN теперь разбирается на протокольной границе, а пароль
  проверяет bounded storage worker. Reactor не читает credential-файлы и не
  получает владение store. Успешный completion связывает account с конкретной
  session и её generation.
- TLS EHLO рекламирует только реализованный `AUTH PLAIN`. Plaintext listener
  AUTH не рекламирует и отвечает `538`. Защищённый submission требует AUTH до
  MAIL/RCPT/DATA и разрешает envelope sender только для вошедшего локального
  account. Отдельный peer listener сохраняет возможность пустого reverse-path.
- SMTP и IMAP state transitions разделены по файлам. Новый
  `reactor/smtp_protocol.rs` содержит 239 строк, `reactor/protocol.rs` 165
  строк; `mod.rs` не добавлялся.
- PASS на Windows: `cargo fmt --check`; `cargo test --locked` (29 passed);
  `cargo clippy --locked --all-targets -- -D warnings`; `git diff --check`.
- PASS в новом Linux Docker image `litterkeep-server:smtp-auth` и чистом volume
  `litterkeep-smtp-auth-test`: стандартный Python `SMTP_SSL.login` отправил от
  Alice письмо Bob, отдельный `IMAP4_SSL` получил MIME-вложение, байты совпали.
  SHA-256: `ae8cbf6572ca0f8f11c8890b0eb2eb081bd0c42fa8c92286a4bc1983737c7e66`.
- PASS `smtp_auth_policy.py`: неверный пароль получил `535`, MAIL до AUTH
  получил `530`, sender Bob под Alice получил `553`, plaintext AUTH получил
  `538`, а plaintext EHLO не объявил AUTH.
- G03 остаётся NOT RUN: отсутствуют handshake timeout, полноценный admission
  control и STARTTLS. G04 также остаётся NOT RUN до Received/Return-Path,
  идемпотентности delivery и остальных критериев этапа.

### 2026-09-19: повторный аудит правил исходников

- Устранено прямое позиционное чтение первого сообщения в store-тесте. Старые
  Python blackbox-клиенты больше не читают `sys.argv[n]`, а используют
  `argparse` и именованные поля. Присваивание MIME-заголовков через mapping API
  `EmailMessage` не является индексированием массива.
- Составные логические цепочки заменены исчерпывающим `match` или `if let` в
  MIME base64, queue, SMTP path, orchestrator ownership/wakeup, Sieve mailbox,
  store mailbox и recovery. Внешние строковые состояния queue уже переводятся
  в `State`, домены в `LocalRoute`, а SMTP reply в новых тестах в `Reply`.
- PASS: все Rust/Python файлы не превышают 300 строк, `mod.rs` отсутствует;
  `cargo fmt --check`; `cargo test --locked` (29 passed); `cargo clippy
  --locked --all-targets -- -D warnings`; все изменённые Python CLI приняли
  `--help`; `git diff --check`.

### 2026-09-19: TLS handshake deadline checkpoint

- Каждая implicit-TLS session получает отдельный пятиисекундный handshake
  deadline. Reactor вычисляет ближайший deadline и передаёт его в `poll(2)`,
  поэтому не использует heartbeat, busy-loop или поток-таймер на соединение.
- После пробуждения отдельный `reactor/deadline.rs` удаляет только session, у
  которой rustls всё ещё находится в handshake и deadline истёк. Завершённый
  handshake снимает deadline и дальше живёт по протокольным правилам.
- PASS в Linux Docker image `litterkeep-server:handshake-timeout`:
  `tls_handshake_timeout.py` открыл TCP и не послал ClientHello; сервер закрыл
  его через 5.013 секунды. Сразу после освобождения slot полный TLS SMTP AUTH
  to IMAP attachment сценарий и вся негативная AUTH policy снова прошли.
- PASS на Windows для доступных целей: format, 29 unit tests, Clippy. Unit-тест
  Linux-only deadline модуля на Windows не компилируется и отдельно не
  засчитывается; фактическое Linux-поведение подтверждено blackbox-тестом.
- G03 остаётся NOT RUN: требуется доказать общий admission limit и исчерпание
  лимита без утечки, а также реализовать обязательный STARTTLS-профиль задания.

### 2026-09-19: TLS admission acceptance checkpoint

- Независимый `tls_admission_limit.py` открыл 256 незавершённых TLS sessions.
  257-е соединение было немедленно закрыто сервером, то есть ограничение
  проверено по сетевому поведению, а не по наличию `MAX_SESSIONS` в исходнике.
- После handshake deadline тест подтвердил закрытие всех 256 соединений. Затем
  стандартный TLS SMTP AUTH to IMAP attachment сценарий снова прошёл, что
  подтверждает освобождение и безопасное повторное использование session slot.
- Реализованные части критерия G03 теперь имеют независимые доказательства:
  TLS trust/SAN, верный и неверный пароль, AUTH только под TLS, handshake
  timeout, buffer bounds и connection admission. G03 остаётся RUNNING, потому
  что его зависимость G02 ещё не завершена и итоговый профиль также требует
  STARTTLS. Статусы ранее начатых G01-G05/G09 в верхней таблице исправлены с
  вводящего в заблуждение `NOT RUN` на `RUNNING`.

### 2026-09-19: durable directory sync and crash checkpoint

- После синхронизации временного blob и атомарного rename storage worker теперь
  вызывает Linux `fsync` каталога `blobs`. После синхронизации mailbox journal
  он также синхронизирует каталог `mailboxes`. Эта блокирующая работа остаётся
  вне reactor в единственном storage worker-е.
- PASS в Linux Docker image `litterkeep-server:durable-dir`: независимый
  TLS SMTP-клиент дождался финального success для письма с бинарным вложением;
  процесс был завершён `docker kill` с exit code 137; новый процесс открыл тот
  же volume и независимый TLS IMAP-клиент нашёл то же письмо и точные байты.
- Это доказательство сохранности при аварии процесса после подтверждённого
  commit, а не заявление о моделировании физической потери питания или лжи
  накопителя. G02 остаётся RUNNING: текстовый journal нужно заменить требуемым
  binary framing, добавить атомарный multi-recipient transaction и дисковую
  квоту с проверенными отказами.

### 2026-09-19: versioned binary journal and UID index checkpoint

- Mailbox journal переведён с текстовых строк на собственный binary frame:
  magic `LKJF`, version 1, big-endian sequence UID, transaction id, bounded
  payload length, payload, FNV-1a checksum и отдельный commit marker. Decoder
  использует проверяемый cursor и не индексирует недоверенный массив напрямую.
- Recovery выполняет один линейный проход `O(n)`: недописанный последний frame
  обрезается до начала frame, а полный frame с неверными magic/version/length,
  transaction id/checksum/commit marker останавливает открытие store. Поиск
  следующего случайного magic не выполняется.
- Добавлен долговечный `format-version`. Неизвестный или старый неразмеченный
  store с mail data получает диагностируемый отказ; автоматического мигратора
  пока нет. Это намеренная несовместимость прежних временных test volumes.
- Оптимизирован hot path: single-writer восстанавливает `next UID` каждого
  mailbox один раз при старте и хранит его в `HashMap`. Append использует в
  среднем `O(1)` lookup и обновляет cache только после durable commit, вместо
  повторного полного replay и накопительного `O(n^2)` поведения.
- PASS: 31 Windows unit test, format и Clippy. В чистом Linux Docker volume
  проверены marker, binary prefix `4c 4b 4a 46 00 01`, последовательные
  `bob-1.eml`..`bob-3.eml`, kill с exit 137, recovery и повторный SMTP AUTH to
  IMAP attachment с совпавшими байтами.
- G02 остаётся RUNNING: один общий атомарный multi-recipient transaction,
  failpoint matrix и дисковая quota ещё отсутствуют.

### 2026-09-19: bounded blob quota checkpoint

- Добавлен отдельный `Quota` budget неизменяемых blobs с документированным
  пределом 25 MiB. При старте он одним линейным проходом учитывает все файлы,
  включая orphan; далее single writer делает `O(1)` preflight и обновление
  счётчика. Проверка выполняется до создания временного файла.
- Unit-тест с budget 4 bytes подтвердил `StorageFull` для сообщения 5 bytes,
  пустой mailbox и отсутствие temp/final blob после отказа. Полный набор вырос
  до 32 tests; format и Clippy прошли.
- PASS Linux Docker build `litterkeep-server:blob-quota`. Новый бинарник открыл
  ранее созданный version-1 store и повторил независимый TLS SMTP AUTH to IMAP
  attachment сценарий с точным совпадением байтов.
- G02 остаётся RUNNING до атомарного multi-recipient transaction и failpoint
  matrix. Квота пока ограничивает blobs, но не journal/accounts metadata; это
  явно не заявляется полной квотой всего filesystem tree.

### 2026-09-19: atomic multi-recipient Unit of Work checkpoint

- Store format обновлён до `litterkeep-store-v2`. Один transaction ID теперь
  публикует один immutable `message-<id>.eml` и несколько mailbox links. Все
  mailbox frames синхронизируются раньше единственного global commit frame в
  `journal/transactions.log`; materialized view обновляется только после него.
- Повторный RCPT одному mailbox дедуплицируется с сохранением порядка первого
  появления. Один blob совместно используется всеми mailbox links, поэтому
  объём вложения не умножается на число локальных получателей.
- При ошибке после публикации blob writer переходит из типизированного
  `Writable` в `Poisoned` и больше не принимает изменения до recovery. UID и
  transaction ID из незавершённых frames не переиспользуются после restart,
  но сами frames без global commit остаются невидимыми.
- PASS 35 unit tests. Failpoint после первого mailbox frame подтвердил: нет
  частичной видимости Bob/Alice, повторная запись отвергается poisoned writer,
  recovery продолжает с новым UID. Проверены format и Clippy.
- PASS в чистом Linux Docker volume: независимый TLS SMTP-клиент отправил один
  DATA для Alice и Bob; оба TLS IMAP-клиента получили идентичные RFC822 bytes и
  вложение. На диске был ровно один blob, два mailbox journal по 51 byte и один
  commit frame 26 bytes. После `docker kill` exit 137 оба link сохранились;
  обычный attachment regression также прошёл.
- На этом checkpoint G02 сохранял RUNNING из-за незавершённой зависимости G01.
  Реализованный multi-recipient commit больше не являлся причиной удерживать
  этап незавершённым; полная failpoint matrix относится к G10.

### 2026-09-19: G01 и G02 PASS, реальные session leases

- Reactor теперь регистрирует каждую живую TCP/TLS `Session` как значение
  `ResourceLease<Session>`. `TaskId` является владельцем эксклюзивного lease,
  а не записью в параллельном журнале наблюдения. I/O, storage completion и
  deadline получают session только через lease.
- Все terminal-пути EOF, I/O error и handshake timeout вызывают единый
  `Orchestrator::close`: реальный socket/TLS resource уничтожается, а task,
  resource, ready и wait metadata удаляются. Admission использует фактический
  `resource_count`, поэтому registry является источником решения, а не логом.
- PASS три последовательных цикла независимого Linux blackbox-теста: по 256
  незавершённых TLS sessions, немедленный отказ 257-й, quiescence после
  пятиисекундного deadline и успешный новый TLS-клиент. Тест посылает первый
  байт TLS record до измерения overflow, чтобы не зависеть от TCP backlog.
- PASS Docker Linux test target `litterkeep-server:linux-tests`: 38/38. В него
  входят реальный `UnixStream` wake через `poll(2)`, округление deadline,
  double-issue, lost-wake, cancellation ownership и удаление terminal registry
  records. На Windows PASS 36/36, format и Clippy с `-D warnings`.
- G01 получает PASS: один poll-reactor обслуживает idle sessions без потока на
  соединение, реальный ресурс находится во владении lease, а обязательные wake
  и ownership-тесты прошли на Linux. Файл модели состояний отделён в
  `runtime/model.rs`; `mod.rs` нет, каждый Rust/Python файл не длиннее 300 строк.
- G02 получает PASS по критериям этапа: durable binary journal и immutable blob
  пережили kill/restart, второй writer отвергается единым кроссплатформенным
  `AlreadyExists`, а принятый commit завершается storage worker даже после
  отмены requester. Полная failpoint matrix остаётся обязательной для G10 и не
  объявлена выполненной.
- Текущий этап остаётся G03. Его implicit TLS, AUTH, admission и timeout уже
  проверены, но обязательный STARTTLS-профиль ещё не реализован, поэтому G03
  сохраняет RUNNING.

### 2026-09-19: SMTP STARTTLS transport checkpoint

- Добавлен отдельный типизированный `TlsMode::StartTls`, а SMTP parser получил
  `Command::StartTls`. До upgrade EHLO рекламирует STARTTLS и не рекламирует
  AUTH; после успешного handshake клиент обязан повторить EHLO и получает AUTH.
- Upgrade выполняется только после полной plaintext-записи ответа `220`.
  Накопленный plaintext input очищается, envelope и authentication сбрасываются,
  затем session создаёт новый `ServerTls` с тем же ограниченным handshake
  deadline. TLS records остаются в существующем poll-reactor без нового потока.
- PASS стандартным Python `smtplib.SMTP.starttls()` с проверкой цепочки и SAN:
  Alice прошла AUTH, отправила Bob MIME-письмо, отдельный IMAP-клиент прочитал
  его, SHA-256 вложения `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- PASS: Windows 37/37, format, Clippy; Docker Linux target 39/39. Все исходники
  сохраняют лимит 300 строк, `mod.rs` не добавлен.
- На этом checkpoint G03 оставался RUNNING: SMTP STARTTLS был проверен, но IMAP
  STARTTLS и его независимый клиентский сценарий ещё не были реализованы.
  Implicit TLS для обоих клиентских протоколов уже был проверен.

### 2026-09-19: G03 PASS, IMAP STARTTLS

- IMAP parser переводит `STARTTLS` в отдельный `Command::StartTls`. До upgrade
  CAPABILITY объявляет `STARTTLS LOGINDISABLED`, а LOGIN возвращает
  `NO [PRIVACYREQUIRED]`; пароль не передаётся storage worker по plaintext.
- После tagged `OK` transport переключается на тот же неблокирующий rustls FSM,
  который использует SMTP. Plaintext input очищается, authentication state
  сбрасывается, handshake получает ограниченный deadline и остаётся реальным
  ресурсом lifecycle lease.
- PASS независимым стандартным `imaplib.IMAP4.starttls()`: неизвестный CA
  отвергнут, plaintext LOGIN отвергнут, доверенный CA и IP SAN приняты, Bob
  вошёл и прочитал доставленное через SMTP STARTTLS MIME-вложение. SHA-256:
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- PASS Windows: 38/38, format, Clippy. PASS Docker Linux: 40/40, включая poll,
  deadline и оба typed STARTTLS parser tests.
- G03 получает PASS: независимо проверены implicit TLS и STARTTLS, доверие и
  отказ неизвестному CA, правильный и неправильный пароль, plaintext policy,
  handshake timeout, ограничение буферов, admission и освобождение resources.
  Текущий последовательный этап теперь G04.

### 2026-09-19: G04 PASS, envelope trace headers

- SMTP transaction теперь передаёт в delivery типизированный `Envelope` с
  reverse-path и уже принятыми RCPT. Получатели по-прежнему не извлекаются из
  `To`, `Cc` или `Bcc`.
- Отдельный `smtp_trace.rs` добавляет `Return-Path` и `Received` перед исходными
  RFC822 bytes. Дата вычисляется за O(1) без таблиц с прямым индексированием;
  исходное MIME-тело не разбирается и не сериализуется повторно.
- Unit-тесты проверили обычный и пустой reverse-path, сохранение исходного Bcc и
  нулевого байта. PASS Windows 40/40 и Docker Linux 42/42, format и Clippy.
- PASS независимым SMTP/IMAP STARTTLS blackbox: Alice отправила Bob вложение,
  сохранённый RFC822 начинается с envelope-derived `Return-Path` и `Received`,
  а SHA-256 вложения остался
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- G04 получает PASS по критерию этапа: Alice -> Bob, успех DATA только после
  durable storage completion, сохранность после kill/restart и отсутствие
  публикации оборванной DATA уже проверены предыдущими checkpoint. Расширенная
  матрица потери финального ответа и storage failpoints остаётся в G10 и не
  объявлена выполненной. Текущий последовательный этап теперь G05.

### 2026-09-19: G05 MIME byte-boundary checkpoint

- Вместо прежнего UTF-8-only map добавлен отдельный байтовый header parser.
  Он сохраняет порядок и повторы полей, точные raw ranges и body offset,
  выполняет unfolding continuation lines и отвергает bare LF. Бинарное значение
  одного поля больше не делает недоступным весь RFC822 source.
- Добавлен отдельный однопроходный quoted-printable decoder: hex octets, CRLF
  soft breaks, строгий отказ bare CR/LF, усечённых и неверных escape. Алгоритм
  работает за O(n) и выделяет не больше начального размера encoded input.
- PASS Windows: 44/44, format, Clippy; лимит 300 строк и запрет `mod.rs`
  соблюдены. G05 остаётся RUNNING: multipart line delimiters, MIME parameters,
  encoded-word, charset profiles и IMAP BODYSTRUCTURE ещё не готовы и не
  объявляются проверенными.

### 2026-09-19: G05 charset, encoded-word и parameter checkpoint

- Внешние имена charset преобразуются в `Charset::UsAscii`, `Utf8` или
  `Iso8859_1`; неизвестный профиль не угадывается. US-ASCII и UTF-8 проверяются
  строго, Latin-1 отображается по байтам без повреждения raw source.
- Отдельный encoded-word decoder поддерживает B и Q, underscore-space и hex
  escapes. Неверная оболочка, неизвестная transfer encoding, charset или Q
  escape дают диагностируемый отказ.
- Structured parameter parser переводит известные media types в enum, учитывает
  quoted semicolon и escape, extended value, упорядоченные RFC 2231 segments и
  percent encoding. Пропущенный segment и неизвестный charset отклоняются.
- PASS 49 Windows unit tests, format и Clippy. Независимый blackbox запустил
  собранный `mime_probe` и сравнил encoded UTF-8 subject и кириллический
  `filename*0*`/`filename*1*` с Python `email`: результаты совпали.
- G05 остаётся RUNNING до line-aware multipart дерева, вложенности, raw ranges
  частей и IMAP ENVELOPE/BODYSTRUCTURE.

### 2026-09-19: G05 line-aware multipart checkpoint

- Добавлен bounded MIME tree: максимум 16 уровней и 256 entities. Каждый узел
  хранит абсолютные ranges raw entity, headers и body; исходное сообщение не
  копируется и не сериализуется заново.
- Multipart boundary распознаётся только как полная delimiter line с CRLF и
  допустимым transport padding. Произвольная подстрока `--boundary` внутри тела
  часть не разрезает. Preamble, epilogue и обязательный closing delimiter
  учитываются отдельно; отсутствие закрытия является ошибкой.
- PASS 51 Windows unit test, format и Clippy. Расширенный независимый
  `mime_python_comparison.py` подтвердил совпадение числа частей и точных байтов
  первого body с Python `email`, дополнительно к encoded-word и RFC 2231 имени.
- G05 остаётся RUNNING: нужны декодирование transfer encoding на уровне entity,
  BODYSTRUCTURE/ENVELOPE и корректные IMAP section/partial responses.

### 2026-09-19: G05 IMAP BODYSTRUCTURE checkpoint

- Каждый MIME entity теперь хранит `TransferEncoding` enum и собственные
  parameters. Leaf body декодируется только из абсолютного raw range; 7bit
  проверяет ASCII, base64 и quoted-printable используют наши bounded decoder-ы,
  неизвестная encoding не угадывается.
- Собственный BODYSTRUCTURE encoder строит multipart children в исходном
  порядке, экранирует строковые значения и сообщает размер encoded raw body в
  октетах. Для text parts отдельно считается число CRLF lines.
- IMAP FETCH принимает типизированные одиночные атрибуты `RFC822`,
  `RFC822.SIZE`, `BODYSTRUCTURE`. Неподдержанный атрибут остаётся BAD, а
  повреждённый MIME при BODYSTRUCTURE возвращает NO вместо ложной структуры.
- PASS сетевой SMTP/IMAP STARTTLS blackbox: доставленное multipart/mixed письмо
  вернуло BODYSTRUCTURE с `MIXED`, отдельный RFC822.SIZE и исходный RFC822;
  вложение сохранило SHA-256
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- PASS Windows 56/56, Clippy и format; PASS Docker Linux 58/58. G05 остаётся
  RUNNING до ENVELOPE, BODY section/PEEK и partial FETCH по октетам.

### 2026-09-19: G05 ENVELOPE и BODY section checkpoint

- FETCH parser преобразует `BODY[]`, `BODY.PEEK[part]`, `BODY[part.MIME]` и
  `<offset.count>` в `BodySection`/`Partial`, не передавая строковые команды в
  reactor. Part numbers проходят 1-based tree traversal через безопасный `get`.
- Отдельный `imap_fetch.rs` выбирает raw encoded body либо raw MIME headers по
  абсолютным ranges. Partial применяется после выбора section и считает
  октеты; BODY.PEEK не меняет состояние сообщения.
- Добавлен собственный ENVELOPE encoder. Имена заголовков типизированы,
  From/Sender/Reply-To/To/Cc/Bcc разбираются в name/mailbox/host tuples,
  Sender и Reply-To получают fallback к From, encoded subject декодируется.
- PASS сетевой STARTTLS blackbox: `BODY.PEEK[1]<0.5>` вернул ровно `trans`,
  `BODY.PEEK[2.MIME]` вернул Content-Type вложения, ENVELOPE содержал subject и
  отдельные Alice/Bob mailbox/host, полный RFC822 и SHA-256 вложения совпали.
- PASS Windows 59/59, Clippy и format; PASS Docker Linux 61/61. На этом
  checkpoint G05 оставался RUNNING: ещё требовались составные encoded-word и
  рекурсия `message/rfc822`; HEADER.FIELDS был перенесён в IMAP-этап G06.

### 2026-09-19: G05 PASS, composite words и message/rfc822

- Encoded-word decoder обрабатывает последовательность B/Q words внутри
  неструктурированного header и удаляет только folding whitespace между двумя
  соседними encoded words. Plain prefix и обычный текст сохраняются.
- `message/rfc822` получает дочерний entity с абсолютными raw ranges и
  ограничивает transfer encoding значениями, для которых raw mapping остаётся
  однозначным. BODYSTRUCTURE включает вложенные ENVELOPE, structure и line count.
- PASS 62 Windows unit test, format и Clippy; PASS Docker Linux 64/64.
  Независимый Python differential
  test подтвердил composite UTF-8/Latin-1 header, вложенное message/rfc822 body,
  multipart ranges и RFC 2231 filename.
- G05 получает PASS по критерию этапа: собственный MIME parser и IMAP reading
  проверены стандартным независимым клиентом, включая ENVELOPE, BODYSTRUCTURE,
  raw section, octet partial и точное совпадение бинарного вложения после
  SMTP -> store -> IMAP. Нереализованные HEADER.FIELDS и полноценный
  инкрементальный IMAP grammar остаются явными задачами G06, а не считаются
  выполненными. Текущий последовательный этап теперь G06.

### 2026-09-19: G06 IMAP lexer checkpoint

- Удалён разбор IMAP через `split_whitespace`. Однопроходный lexer за `O(n)`
  выделяет atoms, границы списков и quoted strings с escape; число токенов
  ограничено, чтобы одна команда не создавала неограниченное число объектов.
- LOGIN теперь сохраняет пробелы внутри quoted username/password, а FETCH
  разбирает parenthesized attribute как синтаксические токены, не удаляя
  скобки строковой эвристикой.
- PASS 64 Windows unit test, format и Clippy. G06 остаётся RUNNING: literals,
  sequence-set, folders, UID, flags, APPEND и IDLE ещё не реализованы.

### 2026-09-19: G06 sequence-set и UID FETCH checkpoint

- После сверки RFC 9051 sections 4.1.1, 6.4.5 и 6.4.9 FETCH принимает
  одиночные номера, списки, прямые/обратные диапазоны и `*`. `UID FETCH`
  сохраняет отдельный enum пространства адресации до получения store snapshot.
- Диапазоны не разворачиваются. Они один раз разрешаются относительно snapshot,
  сортируются и объединяются за `O(r log r)`, затем каждое сообщение проверяется
  бинарным поиском за `O(log r)`. Число исходных диапазонов ограничено 64.
- Reactor возвращает все выбранные сообщения в порядке mailbox. UID FETCH
  не подменяет номер untagged FETCH: снаружи остаётся sequence number, а UID
  автоматически включается в каждый ответ.
- PASS 68 Windows unit tests, format и Clippy; PASS Docker Linux 70/70.
  Обычный Windows cross-check не засчитан: в PATH нет Linux C compiler для
  `ring`, поэтому authoritative Linux проверкой служит Docker build/test.
- G06 остаётся RUNNING: literals, flags, APPEND, folders, UIDVALIDITY/UIDNEXT и
  IDLE ещё не реализованы.

### 2026-09-19: G06 durable SELECT metadata checkpoint

- Store read теперь возвращает согласованный `MailboxSnapshot`: messages,
  UIDVALIDITY и UIDNEXT берутся одним owner-ом из одного materialized view.
  Snapshot вынесен в отдельный файл, поэтому чтение не смешано с записью.
- SELECT и EXAMINE различают READ-WRITE/READ-ONLY, возвращают FLAGS,
  PERMANENTFLAGS, UIDVALIDITY и UIDNEXT. INBOX преобразуется в enum на границе
  parser-а. FETCH до успешного SELECT получает NO, а не читает mailbox по одной
  лишь аутентификации.
- UIDVALIDITY равен 1 при действующем контракте: журналы mailbox не удаляются,
  UID не перераспределяются. Будущий DELETE обязан сохранить allocator либо
  назначить новую validity; это явно зафиксировано у реализации snapshot.
- PASS 69 Windows unit tests, format и Clippy; PASS Docker Linux 71/71.
  Независимый STARTTLS SMTP/IMAP blackbox подтвердил отказ FETCH до SELECT,
  `UIDVALIDITY 1`, `UIDNEXT 2`, `UID FETCH 1:*`, MIME attachment SHA-256
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
  После restart того же контейнера и volume отдельный клиент повторно получил
  один message, неизменный UIDVALIDITY, UIDNEXT 2 и UID 1.
- G06 остаётся RUNNING: literals, APPEND, mutable flags, folders и IDLE ещё не
  реализованы.

### 2026-09-19: G06 synchronized APPEND checkpoint

- APPEND INBOX принимает синхронизирующий `{octets}` literal: continuation
  отправляется только после TLS-аутентификации, проверки mailbox и лимита 10
  MiB. Необъявленный LITERAL+ не принимается и не рекламируется.
- `LiteralReceiver` принадлежит конкретной session, поглощает вход частями и
  отделяет ровно объявленные октеты от завершающего CRLF и следующей команды.
  Большой literal накапливается в bounded receiver, а общий input buffer не
  обязан вместить сообщение целиком.
- Store request ставится только после полного literal. Tagged OK с
  `APPENDUID 1 uid` выдаётся лишь после durable append completion; ошибка store
  возвращает NO. Оборванная сессия не публикует частичный literal.
- PASS unit-тест всех внутренних границ двух фрагментов; PASS 70 Windows tests,
  format и Clippy; PASS Docker Linux 73/73.
- Независимый Python `imaplib` через STARTTLS добавил 97 октетов с NUL,
  получил `APPENDUID 1 1`, затем SELECT/FETCH вернули точные исходные байты.
  После restart с тем же volume подтверждены один message, UIDVALIDITY 1,
  UIDNEXT 2 и UID 1.
- G06 остаётся RUNNING: APPEND flags/date, mutable flags, folders и IDLE ещё не
  реализованы.

### 2026-09-19: G06 durable FLAGS checkpoint

- Системные `Answered/Flagged/Deleted/Seen/Draft`, набор и операция
  replace/add/remove вынесены в общий enum-домен. Parser сравнивает wire atoms
  только на границе и передаёт дальше bitset, а не строки.
- FETCH FLAGS, STORE, STORE.SILENT и UID STORE работают через sequence-set.
  Двухфазный completion сначала разрешает snapshot в устойчивые UID, затем
  ставит отдельную mutation владельцу store. Tagged OK и untagged FETCH FLAGS
  выдаются только после успешного durable completion; `.SILENT` подавляет
  только untagged response.
- Store пишет весь batch UID одним append-only frame с version, count,
  checksum и commit marker, затем `sync_all` и directory sync. Диапазоны UID
  проверяются бинарным поиском, дубликаты сортируются и удаляются до записи.
  Неизвестный UID отклоняется до создания журнала; uncertain write отравляет
  writer до recovery.
- PASS 73 Windows tests, format и Clippy; PASS Docker Linux 76/76.
  Независимый `imaplib` подтвердил обычный STORE response, отсутствие FETCH у
  `.SILENT`, UID STORE и последующий FETCH FLAGS. После restart того же volume
  `\\Seen` у первого и `\\Draft` у второго сообщения сохранились.
- G06 остаётся RUNNING: APPEND flags/date, folders, COPY/MOVE/EXPUNGE и IDLE
  ещё не реализованы.

### 2026-09-19: G06 event-driven IDLE checkpoint

- `IDLE` и его единственная continuation `DONE` представлены enum-типами на
  границе parser-а. Selected session сохраняет lease реального соединения и
  остаётся зарегистрированной в Linux `poll`; отдельный поток или busy loop на
  ожидающую сессию не создаётся.
- `EXISTS` публикуется всем подходящим IDLE-сессиям только после успешного
  durable `Append`/`AppendMany` completion worker-а. Список затронутых mailbox
  хранится в типизированном pending-контексте, сортируется и дедуплицируется,
  поэтому одна фиксация не увеличивает счётчик одной сессии дважды.
- `DONE` завершает исходный tag ответом `OK IDLE completed`. Типизированный
  IMAP `NOOP` после этого подтверждает возврат к обычной грамматике команд.
- PASS 75 Windows tests, format и Clippy; PASS Docker Linux 78/78.
- Независимый Python 3.14 `imaplib` двумя STARTTLS-соединениями подтвердил
  `CAPABILITY IDLE`, пустой SELECT, durable APPEND из второй сессии, `* 1
  EXISTS` в первой, успешные DONE и следующий NOOP.
- G06 остаётся RUNNING: APPEND flags/date, folders, COPY/MOVE/EXPUNGE, очередь
  событий с определённым поведением при отставании и профиль 32 одновременных
  IDLE-сессий ещё не реализованы или не проверены.

### 2026-09-19: G06 APPEND metadata checkpoint

- APPEND grammar принимает ограниченный список системных flags и quoted
  INTERNALDATE перед синхронизирующим literal. Строковые flag atoms и названия
  месяцев преобразуются в enum/bitset на границе parser-а.
- Собственный Gregorian-конвертер проверяет календарную дату, время и numeric
  zone, хранит секунды Unix и выдаёт канонический UTC INTERNALDATE без locale и
  timezone процесса. FETCH INTERNALDATE читает значение из mailbox snapshot.
- Mailbox journal frame v2 фиксирует UID, transaction id, initial flags,
  INTERNALDATE и blob reference под одной checksum и commit marker. Поэтому
  APPENDUID не может быть подтверждён раньше метаданных. Reader продолжает
  распознавать существующие v1 frames с безопасными значениями по умолчанию.
- Последующие STORE mutations перекрывают initial flags, но при отсутствии
  flag journal snapshot использует flags атомарного APPEND frame.
- PASS 80 Windows tests, format и Clippy; PASS Docker Linux 83/83.
  Независимый Python 3.14 `imaplib` передал `\\Seen`, `\\Draft` и
  `19-Sep-2026 12:34:56 +0300`; FETCH вернул оба флага и
  `19-Sep-2026 09:34:56 +0000`. После restart контейнера с тем же volume
  отдельный verify-only запуск получил те же значения.
- G06 остаётся RUNNING: folders, COPY/MOVE/EXPUNGE, HEADER.FIELDS, UID SEARCH,
  subscriptions, Recent, очередь IDLE-событий и расширенный concurrency profile
  ещё не реализованы или не проверены.

### 2026-09-19: G06 durable folder catalog checkpoint

- Store содержит account-scoped каталог со стабильными монотонными folder ID.
  Пользовательское имя находится только в checksum-защищённом journal frame;
  путь mailbox строится из проверенного account и числового ID, поэтому даже
  Unicode или вложенное имя не становится filesystem path.
- При первом обращении идемпотентно создаются INBOX, Sent, Drafts, Trash, Junk,
  Archive, Invoices и тестовый дочерний `Invoices/2026`. SPECIAL-USE хранится
  enum-значением, а не выводится из английского имени. Прерванный хвост журнала
  обрезается, повреждённый полный frame отвергается.
- Типизированные CREATE и LIST проходят через bounded storage worker. LIST
  возвращает delimiter `/`, специальные атрибуты и различает поддержанные
  профили `*`, `%` и exact name. Повторный CREATE получает ALREADYEXISTS.
- PASS 82 Windows tests, format и Clippy; PASS Docker Linux 85/85.
  Независимый Python 3.14 `imaplib` подтвердил default folders, SPECIAL-USE,
  CREATE `Projects/2026`, LIST `*`, отсутствие дочерней папки в LIST `%` и
  восстановление тех же результатов после restart с неизменным volume.
- G06 остаётся RUNNING: named-folder SELECT/APPEND, RENAME/DELETE,
  subscriptions, STATUS, COPY/MOVE/EXPUNGE, modified UTF-7, HEADER.FIELDS,
  UID SEARCH, Recent и полный LIST wildcard profile ещё не готовы.

### 2026-09-19: G06 named-folder mailbox checkpoint

- SELECT разрешает account-scoped имя и получает внутренний mailbox key вместе
  со snapshot одним worker-действием. Дальнейшие FETCH/STORE используют key, а
  не повторяют строковый поиск имени и не могут перейти в каталог другого
  account. Unit test подтверждает разные keys для одинаковых имён Bob и Alice.
- APPEND сначала отправляет bounded `ResolveFolder` единственному владельцу
  store. Literal continuation выдаётся только после успешного resolution;
  неизвестная папка получает `NO [TRYCREATE]` до передачи тела. Session затем
  сохраняет проверенный key до durable append completion.
- Один и тот же внутренний key используется selected session, IDLE notification
  и APPEND pending context, поэтому поступление в named folder не смешивается с
  INBOX или одноимённой папкой другого аккаунта.
- PASS 82 Windows tests, format и Clippy; PASS Docker Linux 85/85.
  Независимый Python 3.14 `imaplib` подтвердил TRYCREATE, CREATE
  `Projects/2026`, APPEND с `\\Draft` и NUL-байтом, SELECT, точный RFC822 FETCH
  и FLAGS. После restart того же container volume отдельный запуск получил
  один message с теми же байтами и флагом.
- G06 остаётся RUNNING: RENAME/DELETE, subscriptions, STATUS,
  COPY/MOVE/EXPUNGE, modified UTF-7, HEADER.FIELDS, UID SEARCH, Recent и полный
  LIST wildcard profile ещё не реализованы.

### 2026-09-20: G06 durable folder lifecycle checkpoint

- `RENAME` и `DELETE` разобраны в типизированные варианты до reactor-а и
  выполняются bounded storage worker-ом, который единолично владеет store.
  Tagged `OK` выдаётся только после `write_all`, `sync_all` и directory sync.
- Каталог использует append-only create/rename/delete events. Переименование
  поддерева является одним checksum-защищённым frame и сохраняет внутренние
  ID. Это O(1) запись в журнал и O(n) неизбежное обновление затронутого snapshot,
  без полного переписывания каталога.
- Replay хранит high-water mark ID отдельно от активного списка. После DELETE
  и повторного CREATE старый ID не возвращается, а недописанный mutation tail
  отсекается без публикации операции. Отдельный crash-unit-test это проверяет.
- INBOX нельзя удалить или переименовать. DELETE родителя с потомками
  отклоняется `[CANNOT]`; RENAME в существующее имя, внутрь собственного
  поддерева и из отсутствующей папки получает типизированный `NO`.
- PASS: 84 Windows tests and 87 Linux Docker tests, format, Clippy
  `-D warnings`, `git diff --check`,
  отсутствие `mod.rs`, файлы не длиннее 300 строк и запрещённых смешанных
  boolean-условий.
- Независимый `imaplib` через STARTTLS проверил subtree RENAME с сохранением
  сообщения, отказ DELETE родителя, DELETE leaf, повторный CREATE с пустым
  новым mailbox и тот же результат после restart контейнера.
- G06 остаётся RUNNING: subscriptions/LSUB, STATUS, COPY/MOVE/EXPUNGE/CLOSE,
  HEADER.FIELDS, UID SEARCH, modified UTF-7, полный LIST wildcard/SPECIAL-USE
  профиль, Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 subscriptions and STATUS checkpoint

- `SUBSCRIBE`, `UNSUBSCRIBE`, `LSUB` и поддержанный профиль `STATUS` проходят
  через типизированный parser, bounded worker и одного владельца store.
- Подписки хранятся независимо от существования mailbox в append-only журнале.
  В памяти используется `BTreeSet`: идемпотентное обновление имеет O(log n),
  а LSUB получает детерминированный порядок без дополнительной сортировки.
- LSUB возвращает удалённую либо ещё не созданную подписку с `\Noselect`.
  SUBSCRIBE не создаёт mailbox, а DELETE/RENAME не переписывают подписки.
- STATUS читает согласованный snapshot и поддерживает `MESSAGES`, `UIDNEXT`,
  `UIDVALIDITY`, `UNSEEN`, не меняя selected state. `RECENT` сознательно
  получает BAD как неподдержанный до реализации нормативной сессионной модели,
  вместо ложного успешного `RECENT 0`.
- PASS: 86 Windows tests, strict Clippy и format. Linux Docker: 89 tests.
  Независимый `imaplib` через STARTTLS проверил две записи с разными flags,
  durable subscribe/unsubscribe, `\Noselect` и точные STATUS counters до и
  после restart.
- По уточнённому требованию Docker использует один фиксированный образ и bind
  mount локальной `./data/g06-subscriptions-status`; named volumes и новые
  runtime-папки для каждого прогона больше не создаются. `.dockerignore`
  исключает `data`, `runtime`, `target` и `.git` из build context.
- G06 остаётся RUNNING: COPY/MOVE/EXPUNGE/CLOSE/CHECK, UID SEARCH,
  modified UTF-7, полный LIST/SPECIAL-USE профиль, Recent и согласование
  sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 durable EXPUNGE checkpoint

- `EXPUNGE` сначала читает snapshot выбранной папки, вычисляет sequence number
  каждой `\Deleted` записи, затем передаёт устойчивые UID единственному writer.
  Untagged `EXPUNGE` и tagged OK выдаются только после fsync tombstone frame.
- Новый append-only `.expunges` journal replay-ится после mailbox и flags
  журналов. Он скрывает только окончательно удалённые UID, не меняет UID
  оставшихся и не откатывает UIDNEXT. Неподтверждённый хвост frame не публикуется.
- EXAMINE сохраняет `selected_read_only` в session и возвращает
  `NO [READ-ONLY]` для EXPUNGE. Бинарные blobs пока намеренно не удаляются:
  они могут разделяться между mailbox, а отдельный GC нужен вместе с анализом
  ссылок и quota semantics.
- Исправлен decoder mailbox journal: frame всегда содержит сначала глобальный
  transaction id, затем UID mailbox. Новый regression test создаёт между двумя
  письмами Bob независимую транзакцию Alice, поэтому больше не маскирует
  различие идентификаторов и проверяет restart.
- PASS: 88 Windows tests, strict Clippy, format и structural audit. Linux
  Docker target `test` успешно выполнил `cargo test --locked`; runtime image
  прошёл проверку реальным Python `imaplib` по STARTTLS. В чистой папке
  `ExpungeFresh` независимый клиент проверил два `\Deleted` сообщения,
  sequence responses `1`, затем `2`, сохранение второго сообщения. После
  restart того же фиксированного контейнера tombstone был повторно подтверждён.
  Отдельно существующий mailbox `Expunge` подтвердил запрет EXPUNGE после
  EXAMINE и durable replay. Все данные остались в `./data/g06-expunge`.
- G06 остаётся RUNNING: COPY/MOVE/CLOSE/CHECK, UID EXPUNGE и UIDPLUS,
  UID SEARCH, modified UTF-7, полный LIST/SPECIAL-USE профиль, Recent и
  согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 CLOSE and CHECK checkpoint

- `CHECK` допускается только в selected state. Все текущие mutating store
  операции уже подтверждаются fsync до IMAP OK, поэтому команда не создаёт
  ложную дополнительную запись и подтверждает именно выбранный mailbox.
- `CLOSE` в read-write state следует двухфазному пути EXPUNGE: сначала
  snapshot определяет Deleted UID и sequence-map, затем один writer durable
  фиксирует tombstone. До этой фиксации selected state сохраняется. После неё
  CLOSE отправляет только tagged OK, без untagged EXPUNGE, и снимает selection.
- `CLOSE` после EXAMINE не пытается изменять mailbox: он только снимает
  read-only selection. Некорректные аргументы `CHECK` и `CLOSE` отклоняются
  parser-ом как BAD.
- PASS: 89 Windows tests, format и strict Clippy. Независимый Python
  `imaplib` по STARTTLS создал `CloseFresh` в `./data/g06-expunge`, выполнил
  CHECK, STORE `\\Deleted`, CLOSE и новый SELECT, который вернул ровно 0
  сообщений. Linux runtime image был собран из текущих исходников.
- G06 остаётся RUNNING: COPY/MOVE, UID EXPUNGE и UIDPLUS, UID SEARCH,
  modified UTF-7, полный LIST/SPECIAL-USE профиль, Recent и согласование
  sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 SEARCH checkpoint

- Базовый `SEARCH` и `UID SEARCH` имеют отдельные typed варианты addressing
  и key. Поддерживаются `ALL`, `SEEN`, `UNSEEN`, `DELETED`, `UNDELETED`.
  Другие search keys не имитируются пустым ответом и получают BAD в parser-е.
- Результат строится из единого worker snapshot: обычный SEARCH возвращает
  sequence number, а UID SEARCH возвращает устойчивый UID той же записи.
  Поэтому последующие удаления соседних сообщений не меняют identity UID.
- PASS: 90 Windows tests, format, strict Clippy и structural audit. Linux
  runtime image с STARTTLS прошёл независимый Python `imaplib` сценарий в
  `./data/g06-expunge`: UNSEEN вернул sequence `2 3`, UID SEARCH SEEN вернул
  `1`, UID SEARCH DELETED вернул `3`.
- G06 остаётся RUNNING: COPY/MOVE, UID EXPUNGE и UIDPLUS, modified UTF-7,
  полный LIST/SPECIAL-USE профиль, Recent и согласование sequence-map двух
  сессий ещё не завершены.

### 2026-09-20: G06 UID EXPUNGE checkpoint

- `UID EXPUNGE <uid-set>` проходит typed parser и ограничивает durable
  tombstone только UID из множества, уже имеющие `\\Deleted`. Обычный EXPUNGE
  остаётся операцией по всем Deleted сообщениям выбранной папки.
- UIDPLUS объявляется только вместе с реально исполняемыми APPENDUID и UID
  EXPUNGE, а не как несвязанный capability token.
- PASS: 91 Windows tests, format, strict Clippy и structural audit.
- G06 остаётся RUNNING: COPY/MOVE, modified UTF-7, полный LIST/SPECIAL-USE
  профиль, Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 UID EXPUNGE Linux black-box checkpoint

- Добавлен независимый STARTTLS-сценарий
  `tests/blackbox/imap_uid_expunge.py`. Он создаёт три письма, помечает UID 1
  и UID 2 как `\\Deleted`, исполняет `UID EXPUNGE 1`, затем проверяет,
  что в mailbox осталось два письма и точные байты письма UID 2 доступны через
  `UID FETCH 2 RFC822`.
- Прогон в фиксированном контейнере `litterkeep-linux-test` с bind mount
  `./data/g06-expunge` завершился PASS как до restart (`committed`), так и
  после него (`replayed`). Имена mailbox `UidExpungeFresh` и
  `UidExpungeFresh2` оставлены в том же локальном каталоге данных; named
  volumes и дополнительные runtime-каталоги не создавались.
- PASS: `python -m py_compile tests/blackbox/imap_uid_expunge.py`,
  `cargo test --all-targets --locked` (91 tests), strict Clippy, format,
  `docker build -f docker/Dockerfile.linux-test --target test -t
  litterkeep-server:linux-test .` и независимая проверка реальным Python
  `imaplib` через STARTTLS.
- G06 остаётся RUNNING: COPY/MOVE, modified UTF-7, полный LIST/SPECIAL-USE
  профиль, Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 modified UTF-7 checkpoint

- Добавлен собственный `imap::modified_utf7` без внешней codec-зависимости.
  Он строго декодирует wire-имя до Unicode на parser boundary, канонически
  кодирует имена в LIST/LSUB/STATUS и отвергает некорректные shift/Base64/UTF-16
  последовательности вместо подстановки повреждённого текста.
- Новой логической папке соответствует уже существующий account-scoped Unicode
  каталог. Поэтому wire-имя `&ZeVnLIqe-` не может стать filesystem path:
  folder catalog всё ещё выдаёт только числовой internal key.
- PASS: `cargo test --all-targets --locked` (95 tests), strict Clippy,
  format, Linux `docker build -f docker/Dockerfile.linux-test --target test
  -t litterkeep-server:linux-test .` и независимый `imaplib` STARTTLS
  сценарий `tests/blackbox/imap_modified_utf7.py`. Он создал, SELECT-нул и
  получил через LIST RFC-имя `&ZeVnLIqe-`, затем подтвердил его после restart
  фиксированного контейнера c bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: COPY/MOVE, полный LIST/SPECIAL-USE профиль, Recent и
  согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 durable COPY checkpoint

- Реализованы `COPY` и `UID COPY` через один owner Store. Новый journal
  frame v3 отделяет transaction ссылки от transaction immutable blob, поэтому
  одна COPY transaction может atomically опубликовать несколько destination
  links на уже зафиксированные blobs без копирования payload и без частично
  видимого sequence-set. Старые v1/v2 frames продолжают декодироваться.
- Reactor сначала получает единый source snapshot, преобразует sequence-set в
  устойчивые UID, затем resolve-ит destination account-scoped folder и только
  после durable commit отправляет `[COPYUID 1 source-uids destination-uids]`.
  Пустая выборка не получает фиктивный пустой COPYUID.
- PASS: `cargo test --all-targets --locked` (97 tests), strict Clippy,
  format. Независимый `imaplib` STARTTLS сценарий
  `tests/blackbox/imap_copy.py` проверил обычный COPY, низкоуровневый UID
  COPY с tagged COPYUID, неизменный source, copied `\\Seen` и состояние
  destination до и после restart фиксированного Linux-контейнера. Данные
  добавлены только в bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: MOVE, полный LIST/SPECIAL-USE профиль, Recent и
  согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 LIST wildcard and SPECIAL-USE checkpoint

- LIST pattern больше не ограничен тремя exact значениями. Для wildcard внутри
  имени используется bounded NFA: `*` проходит delimiter, `%` не проходит
  slash, состояние не разворачивается рекурсивным backtracking.
- Реализована selection форма RFC 6154 `LIST (SPECIAL-USE) "" "*"`.
  Она выдаёт только папки с durable enum `SpecialUse`; CAPABILITY объявляет
  `SPECIAL-USE` именно потому, что эта extended form исполняется. Обычный
  LIST по-прежнему может вернуть attributes для папок, но не рекламирует
  неисполненную CREATE-SPECIAL-USE/RETURN форму.
- PASS: `cargo test --all-targets --locked` (98 tests), strict Clippy,
  format и structural audit. Независимый STARTTLS `imaplib` сценарий
  `tests/blackbox/imap_list_special_use.py` подтвердил CAPABILITY,
  `LIST "" "Invoices/%"`, RFC-строку extended LIST и отсутствие
  `Invoices/2026` в выборке только special-use папок.
- G06 остаётся RUNNING: MOVE, часть extended LIST RETURN/CREATE-SPECIAL-USE,
  Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 atomic MOVE checkpoint

- `MOVE` и `UID MOVE` получают typed enum transfer на parser boundary и
  исполняются единственным владельцем Store. Хранилище сначала fsync-ит
  destination links и transaction-зависимый source tombstone, затем публикует
  одну global commit record. До этой записи restart не показывает ни link, ни
  tombstone; после неё source и destination меняются вместе.
- Unit tests подтверждают оба варианта recovery: незакоммиченный tombstone не
  скрывает source, а завершённый MOVE переживает reopen. Reactor выдаёт
  `COPYUID`, untagged EXPUNGE с согласованными sequence numbers и уменьшает
  selected EXISTS только после успешного durable completion.
- PASS: Windows `cargo test --all-targets --locked` (101 tests), strict Clippy,
  format и structural audit без Rust-файлов более 300 строк. Собран Linux image
  `litterkeep-server:linux` и Docker test target (105 tests); независимый `tests/blackbox/imap_move.py` через
  STARTTLS выполнил MOVE `1,3`, подтвердил source=1 и destination=2, затем
  повторил проверку после restart того же контейнера. Использован только bind
  mount `./data/g06-expunge`; CA для клиента: `./data/pki/ca.pem`.
- G06 остаётся RUNNING: часть extended LIST RETURN/CREATE-SPECIAL-USE, Recent
  и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 two-session EXPUNGE synchronization checkpoint

- После успешного durable EXPUNGE reactor публикует уже рассчитанные sequence
  numbers всем другим IDLE-сессиям, selected в том же mailbox. Инициатор
  получает собственный completion один раз, поэтому не возникает дублирования
  EXPUNGE или двойного уменьшения EXISTS. Не-IDLE selected session также
  обновляет свой внутренний EXISTS, не получая несвоевременный wire event.
- PASS: Windows `cargo test --all-targets --locked` (101 tests), strict Clippy,
  format и Docker Linux test target (105 tests). Независимый STARTTLS
  `tests/blackbox/imap_idle_expunge.py` открыл
  две сессии, создал три сообщения, удалил два из второй сессии и получил в
  первой IDLE-сессии последовательность `EXPUNGE 1`, `EXPUNGE 1`; DONE и
  повторный SELECT подтвердили ровно одно оставшееся сообщение. Данные остаются
  в bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: часть extended LIST RETURN/CREATE-SPECIAL-USE, Recent
  и полная синхронизация всех change classes между несколькими сессиями ещё не
  завершены.

### 2026-09-20: G06 LIST RETURN SPECIAL-USE checkpoint

- Parser принимает `LIST "" "*" RETURN (SPECIAL-USE)` как extended форму
  обычного LIST. Она не фильтрует папки, а требует special-use attributes в
  ответах выбранного набора, которые уже берутся из durable enum каталога.
- PASS: Windows 102 unit tests и strict Clippy; независимый STARTTLS клиент
  `tests/blackbox/imap_list_return_special_use.py` получил `OK` и `\Sent`.
- G06 остаётся RUNNING: CREATE-SPECIAL-USE, Recent и остальные change classes
  multi-session синхронизации ещё не завершены.

### 2026-09-20: G06 CREATE-SPECIAL-USE checkpoint

- CREATE `(USE (\\Archive))` проходит typed parser, bounded worker и durable
  folder journal; LIST возвращает сохранённый special-use attribute.
- PASS: 104 Windows tests, strict Clippy, STARTTLS Linux create/list и replay
  `Archive2` после restart с bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: Recent и полная multi-session синхронизация.

### 2026-09-20: G06 durable IMAP Recent checkpoint

- Добавлен отдельный durable журнал `journal/recent.log`: delivery links
  становятся кандидатами `\Recent` только после общего transaction commit,
  SELECT атомарно получает snapshot и фиксирует claim до wire-success. Поэтому
  `\Recent` не попадает в persistent MessageFlags, не выдаётся второму SELECT
  и не возвращается после restart. COPY, MOVE и EXPUNGE синхронизируют ту же
  projection; incomplete tail журнала обрезается без потери committed delivery.
- SELECT сообщает `RECENT`, FETCH/STORE возвращают session-local `\Recent`, а
  STATUS поддерживает typed `RECENT`. Корреляция нового worker request вынесена
  в отдельный file-level компонент, сохраняя production reactor в лимите
  300 строк без `mod.rs`.
- PASS: Windows `cargo test --all-targets --locked` (107 tests), strict
  Clippy, format, `git diff --check` и audit всех Rust-файлов (не более 300
  строк). Linux release image собран; Docker test target прошёл 111 tests.
  Независимый STARTTLS `tests/blackbox/imap_recent.py` подтвердил durable
  APPEND -> STATUS RECENT -> SELECT/FETCH `\Recent` одной сессией, а его
  `--verify-restart` подтвердил отсутствие повторной выдачи после restart.
  Данные остаются в bind mount `./data/g06-recent-4`, CA: `./data/pki/ca.pem`.
- G06 остаётся RUNNING: полная синхронизация остальных change classes между
  несколькими selected/IDLE сессиями ещё не реализована.

### 2026-09-20: G06 two-session FLAGS synchronization checkpoint

- Успешный durable `STORE` теперь публикует другим IDLE-сессиям того же
  selected mailbox untagged `FETCH (UID ... FLAGS ...)`. Инициатор не получает
  вторую копию completion. `\Recent` кодируется из собственного состояния
  получателя, поэтому session-local marker не становится общим persistent flag.
- PASS: Windows `cargo test --all-targets --locked` (107 tests), strict
  Clippy, format, `git diff --check`, structural audit. Docker Linux test
  target прошёл 111 tests. Независимый STARTTLS
  `tests/blackbox/imap_idle_flags.py` подтвердил, что watcher в IDLE получает
  `\Seen` от writer с `+FLAGS.SILENT`, после чего тот же watcher читает
  durable `\Seen` через UID FETCH. Данные: bind mount `./data/g06-flags-2`.
- G06 остаётся RUNNING: остаются согласование delivery/Recent с уже selected
  сессиями и другие multi-session change classes.

### 2026-09-20: G06 selected-session Recent delivery checkpoint

- После durable APPEND reactor сопоставляет опубликованный UID с каждым уже
  selected mailbox и добавляет его в session-local `\Recent`. Поэтому
  существующая IDLE-сессия сохраняет проверяемый `EXISTS` event и может сразу
  получить `\Recent` через FETCH без записи этого marker в MessageFlags.
- PASS: Windows `cargo test --all-targets --locked` (107 tests), format;
  Linux release image собран. Независимый STARTTLS
  `tests/blackbox/imap_idle_recent.py` подтвердил APPEND в selected mailbox,
  `EXISTS 1` в IDLE и последующий `UID FETCH 1 (FLAGS)` с `\Recent`.
  Данные: bind mount `./data/g06-idle-recent`.
- G06 остаётся RUNNING: ещё нужна полная matrix concurrent change classes и
  явное решение политики `\Recent` для вновь подключающихся selected clients.
