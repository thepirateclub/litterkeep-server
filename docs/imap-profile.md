# Matrix of the current IMAP4rev1 and the target IMAP4rev2

> **About this document.** Line-by-line matrix of IMAP4rev1 and the target IMAP4rev2 with the status of every command group and its verifiable boundary.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Contents

- [Target versions and profile criteria](#target-versions-and-profile-criteria)
- [Command matrix by session state](#command-matrix-by-session-state)
- [message/global profile and MIME decoding](#messageglobal-profile-and-mime-decoding)
- [SEARCH key audit per RFC 9051](#search-key-audit-per-rfc-9051)
  - [Charset profiles and basic SEARCH keys](#charset-profiles-and-basic-search-keys)
  - [RFC 9051 key mapping table](#rfc-9051-key-mapping-table)
  - [SEARCH profile boundaries and literal intake](#search-profile-boundaries-and-literal-intake)
  - [Checkpoint log: keywords, BINARY and FETCH BODY](#checkpoint-log-keywords-binary-and-fetch-body)
  - [Checkpoint log: lexer, tags and command boundaries](#checkpoint-log-lexer-tags-and-command-boundaries)
  - [Checkpoint log: SELECT and EXAMINE lifecycle](#checkpoint-log-select-and-examine-lifecycle)
  - [Checkpoint log: strings, lists and flags](#checkpoint-log-strings-lists-and-flags)
  - [Advertised profile and claimed limitations](#advertised-profile-and-claimed-limitations)
  - [Command parsing boundaries and error handling](#command-parsing-boundaries-and-error-handling)
- [EAI UTF-8 extension: RFC 9755 audit](#eai-utf-8-extension-rfc-9755-audit)
  - [UTF-8 mode standard and server position](#utf-8-mode-standard-and-server-position)
  - [RFC 9755 obligations table](#rfc-9755-obligations-table)
  - [Open RFC 9755 obligations and UTF8=ONLY](#open-rfc-9755-obligations-and-utf8only)

### Target versions and profile criteria

The server still advertises IMAP4rev1. The modern baseline targeted for the next
stage is RFC 9051 IMAP4rev2; it cannot be advertised until the mandatory
semantics and the network acceptance tests pass. The presence of an extension is
not inferred from a client having used the corresponding command by chance.

The matrix fixes the actually executable profile, not a wish list.
`PASS` means that a typed parser, a reactor state transition and a
profile test exist. `PARTIAL` means deliberately limited semantics that
CAPABILITY does not present as a broader extension.

### Command matrix by session state

| State or section | Commands and properties | Status | Verifiable boundary |
|---|---|---|---|
| Any | CAPABILITY, ID, NOOP, LOGOUT | PARTIAL | RFC 2971 ID command is typed, validated, and answered `* ID NIL` to avoid implementation fingerprinting; client parameters are discarded and never affect operations. CAPABILITY advertisement, NIL/list forms, malformed duplicate rejection, privacy response, and post-error NOOP recovery pass independent Linux STARTTLS wire acceptance; other unlisted IMAP extensions remain unsupported |
| TLS | STARTTLS | PASS | Reset of the plaintext state and a step-by-step rustls handshake |
| Authentication | LOGIN | PASS | TLS only, a shared Argon2 worker, a repeated login is forbidden; username/password accept atom, quoted-string, synchronizing and LITERAL+ non-synchronizing literals within 64 KiB; the independent STARTTLS acceptance checks both forms |
| Authentication | AUTHENTICATE PLAIN | PASS | Continuation, cancel, atom/quoted astring, empty SASL-IR marker, authzid policy, TLS only; the other mechanisms are not advertised |
| Mailbox discovery | LIST, LSUB, STATUS | PARTIAL | CAPABILITY advertises implemented `LIST-EXTENDED` (RFC 5258) and `LIST-STATUS` (RFC 5819); modified UTF-7, `*`/`%`, root discovery, empty selection `()`, slash-delimited relative reference prefix with canonical response names, SUBSCRIBED, RECURSIVEMATCH with CHILDINFO for SUBSCRIBED/SPECIAL-USE, RETURN (CHILDREN), RETURN (SUBSCRIBED), and RETURN STATUS (MESSAGES/RECENT/UIDNEXT/UIDVALIDITY/UNSEEN/DELETED/SIZE/APPENDLIMIT) verified; RECENT reports unclaimed durable messages before SELECT and session-owned recent UIDs after SELECT, consistently in STATUS and LIST STATUS; combined UTF8=ACCEPT LIST/STATUS wire responses preserve Unicode mailbox names; missing names are selected only by SUBSCRIBED and retain `\\NonExistent`; multi-namespace/breakout naming is not implemented |
| Namespace | NAMESPACE | PASS | Advertised and tested; personal namespace is `""` with `/` delimiter, unavailable classes are NIL |
| Extension negotiation | ENABLE | PARTIAL | Atom and quoted-string token forms are distinguished; unquoted arguments are validated against the IMAP atom production, invalid atom-special characters receive tagged BAD, and valid unsupported capability atoms remain ignored; authenticated `ENABLE UTF8=ACCEPT` activates a connection-local mode and returns `ENABLED UTF8=ACCEPT`; RFC 9755 UTF8=ACCEPT is advertised and independently checked before TLS, after TLS/LOGIN and after ENABLE; remaining RFC 9755 and IMAP4rev2 behavior is not claimed |
| Mailbox mutation | CREATE, DELETE, RENAME | PASS | Durable journal, subtree rename, retired folder IDs |
| Subscription | SUBSCRIBE, UNSUBSCRIBE | PASS | Durable subscription independent of the folder existence |
| Selection | SELECT, EXAMINE | PASS | UIDVALIDITY, UIDNEXT, FLAGS includes the system flags and the keywords that exist in the mailbox; PERMANENTFLAGS includes `\\*` for new keywords only in a read-write selection; EXAMINE reports `PERMANENTFLAGS ()`; a repeated SELECT/EXAMINE closes the previous folder with `OK [CLOSED]` before the new responses, a successful selection includes a LIST with the canonical name and the special-use attribute of the mailbox, and a failed select leaves the authenticated state without a selected folder |
| Message insertion | APPEND | PARTIAL | Bounded synchronizing literal and RFC 7888 `LITERAL+` non-synchronizing literals up to APPENDLIMIT (25 MiB); strict parenthesized system flags/keywords, quoted-string-only RFC date-time INTERNALDATE, APPENDUID after fsync; RFC 3502 §3.1 zero-length message literal returns tagged `NO` and cancels the whole APPEND/MULTIAPPEND without mailbox mutation; independent STARTTLS acceptance sends a valid first MULTIAPPEND message followed by `{0}`, verifies `NO`, then confirms after SELECT that no prefix was published; oversized non-synchronizing payload gets `BYE [TOOBIG]` and connection closure; oversized synchronizing APPEND is rejected with tagged TOOBIG before payload; MULTIAPPEND preserves per-message flags/keywords/date, advertises the extension and returns ordered APPENDUID; independent acceptance checks distinct flags and exact RFC822 bytes including NUL and 0xFF plus non-sync payload above 4096 octets |
| Reading | FETCH, UID FETCH | PARTIAL | Grouped items, ALL/FAST/FULL with an RFC-compatible expansion of FULL into BODY, BODY and BODYSTRUCTURE, UID, the RFC822/RFC822.HEADER/RFC822.TEXT legacy items with the correct wire labels, BODY sections/partial and BINARY/BINARY.PEEK/BINARY.SIZE; decoded MIME leaf parts, Base64/QP, literal8 for NUL, decoded partial, UNKNOWN-CTE and the Seen effects are verified. UID FETCH always gets exactly one UID item regardless of the request list; a plain FETCH does not insert it without a request. Separate Rust unit tests and an independent Linux STARTTLS acceptance check this. After a durable Seen transition the BODY and BINARY FETCH include the updated FLAGS when they were not requested explicitly; BODY.PEEK, BINARY.PEEK and a repeated read without a change of FLAGS do not get them automatically. RFC 3516 BINARY is declared in CAPABILITY before STARTTLS, after TLS and after LOGIN. RFC 6532 message/global accepts the identity, Base64 and quoted-printable CTE with a separate source for the decoded nested ranges; an unknown CTE keeps the opaque octet-stream fallback. The full rev2 grammar remains |

### message/global profile and MIME decoding

For `message/global` the `7bit`, `8bit`, `binary`, Base64 and quoted-printable
CTEs build a nested structure; a non-identity CTE gets its own decoded source,
and its offsets are not mixed with the original wire ranges.
BODYSTRUCTURE returns `MESSAGE/GLOBAL`, `BODY[1]` stays the encoded wire body,
`BODY[1.1]` selects the decoded nested part, and the BODY HEADER and BINARY selection
reach into the nested parts. An unknown CTE keeps the opaque
`application/octet-stream` fallback. Unit vectors for the ownership,
ENVELOPE and the section numbering are verified; an independent Linux STARTTLS black-box additionally
compares the encoded BODY[1], the decoded BODY[1.1], the nested UTF-8 header, BINARY and the full
RFC822 literal. The Litterkeep app was not checked. This does not change CAPABILITY and does not
advertise IMAP4rev2.
| Search | SEARCH, UID SEARCH | PARTIAL | All basic RFC 9051 SEARCH keys are recognised and evaluated; an exhaustive parser check enumerates every key. UTF-8 default, UTF-8/US-ASCII CHARSET, BADCHARSET, synchronizing string literals up to 64 KiB, ESEARCH RETURN ALL/COUNT/MIN/MAX, decoded MIME text and the exclusion of an unknown CTE from the text projection are verified; MIME text charset allowlist: US-ASCII, UTF-8, ISO-8859-1..10, ISO-8859-11, ISO-8859-13..16, TIS-620, IBM866, KOI8-R/U, GBK, GB18030, Big5, EUC-JP, ISO-2022-JP, Shift_JIS, EUC-KR, Windows-874, Windows-1250..1258; UTF-16/32 are excluded by the MIME suitability policy. Independent SMTP -> IMAP STARTTLS acceptance covers a Windows-874 body, 33 single-byte charset labels (29 mapping families, including RFC 1556 ISO-8859-6/8 E/I), GBK/GB18030, Big5/Big5-HKSCS, EUC-JP, ISO-2022-JP, Shift_JIS and EUC-KR bodies, as well as RFC 2047 Subject encoded-words UTF-16/32 and ISO-2022-JP with a UTF-8 SEARCH literal. The matrix contains 42 charset labels, including the IANA alias `csUTF8` for UTF-8. KOI8-R/U mappings are unit-tested over all the octets of RFC 1489/2319, the WHATWG GB18030 decoder is checked against all 23 940 two-byte pointers and the boundary four-byte ranges, Big5 against all 19 782 pointer positions and the four two-character exceptions, EUC-JP against both 8 836 wire pointer branches JIS 0208/0212, and half-width kana. Shift_JIS is tested against all 11 280 two-byte pointers, including the EUDC private-use extension, and against all the single octets; EUC-KR against all 23 940 two-byte pointer slots of its pinned WHATWG index. ISO-2022-JP state-machine unit tests cover ASCII, Roman, Katakana, JIS 0208 and the refusal without a final ASCII reset; SMTP -> IMAP wire tests separately confirm the text of every multibyte body. This is wire coverage of the listed variants, not of all MIME charsets and fields. RETURN SAVE and `$` SEARCH criterion work per session with SELECT/error reset and MIN/MAX selection; FETCH/UID FETCH and STORE/UID STORE `$` map saved UIDs to the active number space, COPY/UID COPY `$` resolve saved UIDs in their respective addressing modes, UID EXPUNGE `$` intersects saved UIDs with `\\Deleted`, empty sets are safe, and pipelined SEARCH SAVE followed by COPY is verified; ESEARCH and SEARCHRES are advertised; non-base extensions remain |
| Flag mutation | STORE, UID STORE | PASS | System flags and case-insensitive keywords: replace/add/remove, SILENT, one durable batch, read-only denial; the parenthesized and unparenthesized forms are parsed without dropping the delimiters; nested/unbalanced lists and quoted flags are rejected. The UID STORE response includes the mandatory UID item, verified independently with a message whose UID 2 differs from the sequence number 1 |
| Copy and move | COPY, UID COPY, MOVE, UID MOVE | PASS | Atomic links/tombstones, COPYUID, multi-session projection |
| Deletion | EXPUNGE, UID EXPUNGE, CLOSE | PASS | Durable tombstones, UIDPLUS, sequence-map notifications |
| Selection lifecycle | UNSELECT | PASS | Advertised and tested; returns to authenticated state without expunging `\\Deleted` |
| Synchronization | CHECK | PASS | Tagged completion after previously completed durable operations |
| Waiting | IDLE, DONE | PASS | Event wakeup without a session thread and without a polling loop |
| Special use | LIST SPECIAL-USE, RETURN, CREATE USE | PASS | Durable enum attributes, without a guess by the folder name |

## SEARCH key audit per RFC 9051

### Charset profiles and basic SEARCH keys

The charset profiles RFC 1556 ISO-8859-6-E/I and ISO-8859-8-E/I are recognised as
separate typed variants and enter the SMTP -> IMAP body matrix. SEARCH compares
Unicode code points after the corresponding ISO-8859-6/8 octet mapping; the server does not
apply an explicit control presentation or an implicit bidi layout algorithm and
does not claim support for the visual rendering of these profiles.

The basic sequence-set, UID, ALL, system/session flag, internal/sent
date, size, NOT/OR and grouping keys have a parser and an evaluator. HEADER/BODY/TEXT and
MIME decoding are verified separately. At the same time the syntax and the semantics do not fully
match the RFC:

### RFC 9051 key mapping table

| RFC 9051 key group | Actual state |
|---|---|
| `<sequence-set>`, UID, ALL, ANSWERED/UNANSWERED, DELETED/UNDELETED, DRAFT/UNDRAFT, FLAGGED/UNFLAGGED, SEEN/UNSEEN, RECENT/OLD/NEW | The parser/evaluator are implemented; RECENT is a session projection |
| BEFORE/ON/SINCE, SENTBEFORE/SENTON/SENTSINCE, LARGER/SMALLER | The parser/evaluator are implemented; the dates are compared by calendar day according to the profile |
| NOT, OR, a parenthesised group and the implicit AND | The parser/evaluator are implemented |
| HEADER, BODY, TEXT | The parser/evaluator and the MIME charset/transfer decoding are implemented; independent wire acceptance covers Windows-874, 33 single-byte charset labels (29 mapping families, including RFC 1556 ISO-8859-6/8 E/I) and the separate labels TIS-620/ISO-8859-11; the matrix contains 42 labels in total. The remaining body charsets are not part of these scenarios |
| SUBJECT, FROM, TO, CC, BCC | The parser/evaluator use the same decoded ENVELOPE projection as FETCH ENVELOPE; the RFC 2047 Subject encoded-words UTF-16 and UTF-32 in generic/BE/LE forms are delivered independently and are found through a UTF-8 SEARCH literal. The address is compared by name, mailbox, host and `mailbox@host`, without the RFC 5322 wire delimiters. The obsolete null members are skipped without losing the mailbox order. The addr-spec delimiter is chosen outside a quoted local-part and a domain-literal; a quoted-pair inside a quoted display-name is decoded. An obsolete source-route is discarded before the target addr-spec is parsed. The CFWS around the dots of an obsolete dot-separated local-part/domain is normalised; the content of a quoted local-part and of a domain-literal is preserved. The trailing CFWS after the `;` of a group is separated from its content. The RFC 5322 groups are serialised as start/member/end address structures; the comments are removed before ENVELOPE/SEARCH. The remaining obsolete forms and the malformed syntax limit the shared parser |
| Address groups and addr-spec | FETCH ENVELOPE, SEARCH address keys | PARTIAL | RFC 5322 group markers, empty groups, nested and trailing comments, obsolete null members, display-name quoted-pair, RFC 2047 encoded words in unquoted display phrases, quoted local-part containing `@`, source-route destination, obsolete CFWS around dots, and address SEARCH are covered; malformed encoded words remain visible and quoted display names remain literal; other obsolete forms and grammar mismatches remain limited |
| KEYWORD, UNKEYWORD | A typed parser/evaluator and the network checks are implemented; the keywords are part of APPEND v4 and the versioned flags journal v2. The old append v1-v3 and flags v1 are read as before; FETCH FLAGS, STORE and COPY/MOVE preserve the keywords. STORE updates both classes of flags with one checksum/commit frame |

### SEARCH profile boundaries and literal intake

The basic set of the RFC 9051 SEARCH keys is closed by the parser/evaluator acceptance; that does not
mean the full SEARCH wire grammar. The quoted strings and the synchronizing literals
up to 64 KiB are verified. The RFC 7888 `LITERAL+` capability is advertised together with
bounded wire receivers for LOGIN, SEARCH and APPEND; an independent STARTTLS client sent
all three literals without waiting for a continuation and compared the stored APPEND octets.
The bounded synchronizing-literal receiver accepts both `astring` arguments of `LOGIN` and the
mailbox `astring` of the commands CREATE, DELETE, RENAME, SUBSCRIBE, UNSUBSCRIBE, LIST,
LSUB, STATUS, COPY, MOVE, SELECT and EXAMINE, including UID COPY/MOVE. An independent
raw STARTTLS client checked the continuation and the successful execution of CREATE, LIST,
SELECT with a literal mailbox. SEARCH/UID SEARCH and APPEND have their own
verified paths. The non-synchronizing literals are limited to 64 KiB for the command
arguments and to APPENDLIMIT for APPEND; exceeding it closes the connection after BYE.
This does not mean the support of the literals in the other arguments of all the commands. The address
SEARCH criteria and FETCH ENVELOPE use one projection; the nested comments
and quoted-pairs are removed from the semantic fields. The obsolete forms and the malformed
RFC 5322 syntax remain limitations of the address parser.

The wire framing of a synchronizing literal counts the announced octets, so a CRLF
inside the payload does not end the value early. The independent STARTTLS LOGIN
acceptance passed a CRLF inside the password literal, got the usual authentication
refusal and then logged in successfully with the correct credentials in the same session.
A raw NUL is rejected as a binary string. The LOGIN password literal is preserved
byte for byte, including the invalid UTF-8 sequences, and is passed to the authentication owner;
the username, mailbox and SEARCH string literals remain text-valued productions and
require a UTF-8 representation valid for their mode. This is not the support of the full
arbitrary octet space in all the IMAP arguments.

### Checkpoint log: keywords, BINARY and FETCH BODY

G14 keyword checkpoint (2026-09-24): the custom flags and KEYWORD/UNKEYWORD passed
the Windows tests, the Linux Rust 1.97 strict Clippy/254 tests and the STARTTLS IMAP black-box
with a repeated opening of the data directory. The rest of the G14 profile remains RUNNING.

G14 BINARY checkpoint (2026-09-25): the RFC 9051 BINARY, BINARY.PEEK and
BINARY.SIZE are recognised as typed FETCH items. The MIME CTE is decoded before
the partial range is applied; a NUL in the answer is serialised as literal8, the text
parts are normalised to CRLF, an unknown transfer encoding gives a tagged
NO [UNKNOWN-CTE]. An independent STARTTLS black-box delivered an attachment over SMTP,
compared every decoded octet through BINARY and checked SIZE, partial, Seen/PEEK and
UNKNOWN-CTE. This is not a check of the Litterkeep application. The other RFC 9051 FETCH items,
the commands and the extensions are not closed yet, so the profile remains IMAP4rev1/PARTIAL.

G14 FETCH BODY checkpoint (2026-09-25): a separate BODY data item returns a
non-extensible MIME body structure. The FULL macro expands exactly like FLAGS,
INTERNALDATE, RFC822.SIZE, ENVELOPE and BODY; BODYSTRUCTURE remains a separate
request. The Windows/Linux unit gates and the independent STARTTLS SMTP/IMAP attachment
black-box passed. The Litterkeep app was not checked; IMAP4rev2 is not advertised.

### Checkpoint log: lexer, tags and command boundaries

G17 tokenizer checkpoint (2026-09-26): unquoted atom rejects an embedded
double quote; quoted-string remains a separate token form. Independent STARTTLS
wire acceptance received tagged BAD and then issued NOOP successfully to verify
session recovery. This closes only the quoted-special delimiter case, not the
remaining atom-special context rules or IMAP4rev2 profile.

G17 tag checkpoint (2026-09-26): parser and malformed-UTF-8 recovery use the
RFC 9051 `tag` production, not generic `atom`: `]` is accepted as a response
special and `+` is excluded. Unit and STARTTLS wire acceptance cover valid tag,
error correlation, and safe fallback for the excluded character.

G17 command boundary checkpoint (2026-09-26): argument extraction no longer
skips list delimiter tokens. Command verbs and UID subcommands require atom
tokens, so parenthesized or quoted `NOOP` cannot be accepted as a command.
Parser tests and independent STARTTLS acceptance verify tagged BAD and session
recovery. This does not yet make the generic lexer enforce every atom-special.

G18 FETCH sequence-set checkpoint (2026-09-27): FETCH consumes only its next
token for the mandatory sequence-set instead of searching past list delimiters.
The malformed command `FETCH (1 FLAGS` is rejected with tagged BAD; the
independent STARTTLS grammar scenario also verifies NOOP recovery. This closes
only this FETCH boundary, not the remaining atom productions or IMAP4rev2.

G18 ENABLE atom checkpoint (2026-09-27): each unquoted ENABLE capability is
validated as an IMAP atom before Enum conversion. Independent STARTTLS grammar
acceptance confirms `ENABLE X%` receives tagged BAD, while valid unknown
`X-UNKNOWN` is ignored with an empty ENABLED response and tagged OK. The ENABLE
and UTF8=ACCEPT profile remains PARTIAL; IMAP4rev2 is not claimed.

### Checkpoint log: SELECT and EXAMINE lifecycle

G18 SELECT lifecycle checkpoint (2026-09-27): SELECT/EXAMINE clear the previous
selected mailbox before attempting a replacement and emit untagged `OK [CLOSED]`
before responses for the new mailbox. An unsuccessful selection leaves no
mailbox selected and does not expunge messages. Independent Linux STARTTLS
acceptance verifies repeated SELECT ordering, failed SELECT state, FETCH
rejection and NOOP recovery. A follow-up SELECT response checkpoint verifies
the required RFC 9051 LIST response for SELECT INBOX, SELECT of a unique
user-created mailbox, and EXAMINE INBOX with READ-ONLY completion.
This closes only these scoped response/lifecycle requirements; the full
IMAP4rev2 profile remains unclaimed.

G18 SELECT LIST attribute checkpoint (2026-09-27): the selected mailbox
projection carries the folder catalog's typed special-use value into the
mandatory SELECT/EXAMINE LIST response. Independent Linux STARTTLS acceptance
checks `\\Archive` for the default Archive mailbox; other base LIST attributes
remain subject to their separate profile coverage.

G18 EXAMINE permissions checkpoint (2026-09-27): read-only selection now sends
`PERMANENTFLAGS ()` rather than advertising writable system flags and keywords.
Independent Linux STARTTLS wire acceptance asserts the empty list on EXAMINE
and confirms read-write SELECT retains its writable PERMANENTFLAGS. This does
not imply other mailbox ACL profiles.

G18 SELECT defined keywords checkpoint (2026-09-27): the FLAGS response unions
the mailbox snapshot's existing persistent keywords with system flags. A Store
unit vector checks keyword union/deduplication; independent Linux STARTTLS wire
acceptance APPENDs a unique keyword and verifies it in FLAGS after both SELECT
and EXAMINE. Other optional mailbox flag namespaces remain unsupported.

### Checkpoint log: strings, lists and flags

G17 astring checkpoint (2026-09-26): LOGIN and SELECT/EXAMINE now validate raw
atom tokens against `ASTRING-CHAR`; special characters require quoted form,
except the permitted `]` response-special. LOGIN rejects excess arguments.
Parser cases and independent STARTTLS acceptance verify `%` rejection in LOGIN;
this does not yet imply every command argument production is covered.

G17 LIST checkpoint (2026-09-26): basic LIST validates its reference as an
astring and its pattern as `list-mailbox`; bare wildcards `%` and `*` and
response-special `]` remain legal, while brace/backslash/quote require a quoted
string. Parser tests and independent STARTTLS acceptance verified rejection of
bare `{` and successful completion of a bare `*` pattern.

G17 flag grammar checkpoint (2026-09-26): STORE accepts exactly one valid
parenthesized flag-list or a non-empty unparenthesized sequence of flag atoms;
nested, unbalanced, quoted, and trailing structural forms are rejected. APPEND
uses the same strict flag production and rejects quoted flags before requesting
literal bytes. Parser and independent STARTTLS acceptance cover these errors.

G17 APPEND date-time checkpoint (2026-09-26): RFC 9051 `date-time` is
accepted only as quoted-string. An unquoted date-looking atom is rejected
before literal continuation; a valid quoted date still reaches continuation
and completes with an empty literal in independent STARTTLS acceptance. This
does not close remaining APPEND grammar or the IMAP4rev2 profile.

### Advertised profile and claimed limitations

CAPABILITY advertises `IMAP4rev1`, `IDLE`, `UIDPLUS`, `MOVE`, `SPECIAL-USE`,
`BINARY` per RFC 3516, `NAMESPACE`, `UNSELECT`, `ENABLE`, `ESEARCH` and `SEARCHRES`.
It also advertises RFC 2971 `ID`; the server returns `* ID NIL` and does not reveal
the build or the host fingerprint. ENABLE ignores the unknown and the not yet supported extensions per the RFC. In the authenticated state
`ENABLE UTF8=ACCEPT` activates the mode on the current connection and returns
`ENABLED UTF8=ACCEPT`; the capability is advertised after the verification of its base
contract, although the whole RFC 9755 profile is not closed yet. `APPENDLIMIT=26214400` advertises a single limit for the message size
in octets; STATUS and LIST RETURN STATUS return the same value for every mailbox.
`IMAP4rev2` is not claimed.
`AUTH=PLAIN SASL-IR` is available only inside TLS. Before STARTTLS the server returns
`LOGINDISABLED`; a plaintext listener without TLS also does not advertise the password
mechanism and answers `PRIVACYREQUIRED` to LOGIN/AUTHENTICATE.

During a live delivery every already selected session gets its own `\Recent`.
The first following SELECT atomically takes the durable candidate; the next SELECT
does not see it. This deterministic rule stays within the ambiguity that RFC 3501
allows for several simultaneously open connections.

IMAP4rev2, CONDSTORE, QRESYNC, COMPRESS, SORT and THREAD are not claimed.
The basic RFC 9051 SEARCH keys are recognised and evaluated, but the additional
SEARCH/ESEARCH extensions and the full formal SEARCH profile stay outside the
claimed coverage. The missing capabilities are not masked by an empty successful
result.

### Command parsing boundaries and error handling

The parser produces a typed refusal for the extra arguments of CAPABILITY, LOGOUT,
NOOP, IDLE and STARTTLS. A tagged BAD carries a readable reason text: an empty
resp-text is allowed in RFC 9051, but the standard Python imaplib aborts the
parsing of a response with an empty BAD, so the interoperability is checked as well.
The quoted-pair in a quoted-string of the current IMAP4rev1 profile allows only `\"`
and `\\`; an arbitrary `\x` is now rejected lexically, and the original tag
is preserved in the tagged BAD. Before ENABLE the legacy quoted-string rejects the raw
high-bit characters; after ENABLE UTF-8 is allowed in a quoted-string, but not in an
atom. A synchronizing
SEARCH literal is handled separately from the legacy atom/quoted-string: the UTF-8
BODY/UID literals and an empty literal are checked by an independent STARTTLS client.
The high-bit characters inside an atom are rejected with tagged BAD while UTF8=ACCEPT is not
enabled; this is verified by a unit test and by an independent STARTTLS acceptance. After ENABLE
UTF-8 is allowed only in a quoted-string, and malformed UTF-8 and the forbidden C0/C1
mailbox scalars must get tagged BAD; their independent STARTTLS acceptance
also covers the recovery of the session through NOOP. The consecutive
synchronizing literals in one SEARCH HEADER, including two continuations, are also
verified; the trailing pipelined command is preserved. The unit and the wire acceptance
confirmed the tagged BAD and the NOOP execution after the error. UTF8=ACCEPT is already
switched through ENABLE, but it is not advertised through CAPABILITY before the full
profile; IMAP4rev2 is not claimed.
Only the RFC `SP` counts as a command grammar separator: HTAB does not turn into a
space between the tag and the verb, and a CTL is rejected in an atom. HTAB stays allowed
in a quoted-string, while NUL/CR/LF are rejected there. These boundaries are covered by
a unit test; the independent STARTTLS acceptance confirmed the tagged BAD on a TAB between
the tag and NOOP and the following successful NOOP.
If a command line does not decode as UTF-8, the reactor keeps a safe
ASCII tag before the BAD answer; the corrupted UTF-8 inside the tag itself uses the
untagged fallback without reflecting the untrusted bytes. An independent STARTTLS
black-box checked the tagged BAD on a malformed UTF-8 quoted LOGIN, the untagged BAD
for a corrupted tag without an octet reflection and a successful NOOP after the errors.
The tag recovery applies exactly the grammar `tag = 1*<ASTRING-CHAR
except "+">`: the closing bracket is allowed as a `resp-special`, whereas plus
is forbidden. This does not match the atom grammar and is checked by separate cases.
The fix itself does not change the advertised profile.

## EAI UTF-8 extension: RFC 9755 audit

### UTF-8 mode standard and server position

The current standard for the UTF-8 mode of IMAP4rev1 is
[RFC 9755](https://www.rfc-editor.org/rfc/rfc9755.html), published in 2025
and replacing RFC 6855. It does not extend IMAP4rev2: RFC 9051 already includes
this feature set. The server remains IMAP4rev1, advertises the verified
`UTF8=ACCEPT`, but does not claim the full coverage of all the requirements of RFC 9755.

### RFC 9755 obligations table

| RFC 9755 obligation | Current state | What is required for PASS |
|---|---|---|
| Capability and ENABLE | `UTF8=ACCEPT` is advertised; an independent STARTTLS checks the discovery before TLS, after TLS/LOGIN and after ENABLE. `ENABLE UTF8=ACCEPT` after the authentication activates the session mode and returns `ENABLED UTF8=ACCEPT`; a new end-to-end test confirms an APPEND refusal before ENABLE and a success after it, a UTF-8 mailbox CREATE/LIST and a UTF-8 ENVELOPE. The NFC output for LIST/LSUB/STATUS and the historical aliases is verified by separate acceptance. The mailbox validator rejects the unassigned points of Unicode 17.0.0 | Extend the acceptance to the remaining RFC 9755 response paths and run a full RFC 9755 audit; do not advertise `UTF8=ONLY` |
| UTF-8 quoted-string | The mode-aware lexer is switched on by the session ENABLE; an independent wire test confirms the BAD for a UTF-8 CREATE before ENABLE and the successful mailbox commands after it. FETCH ENVELOPE and the MIME strings of BODY/BODYSTRUCTURE use the RFC 3501 literals before ENABLE and the quoted UTF-8 after it; the independent STARTTLS acceptance checked both answers, including the RFC 2231 filename/parameters and the decoded Content-Description. The malformed UTF-8, the C0/C1 controls and the UTF-8 atom get BAD, followed by a successful NOOP | Check the remaining IMAP response string paths and the full set of RFC 9755 |
| Mailbox names | CREATE/DELETE/RENAME, SUBSCRIBE/UNSUBSCRIBE, APPEND, STATUS, COPY/MOVE, UID COPY/MOVE and LIST/LSUB use the mode-aware parser; an independent STARTTLS client verified the mutation and the transfer between UTF-8 names, including the tagged COPYUID and the RFC822 byte match. The Unicode 17 NFC is applied to the new names and to the output. The existing decomposed folder journals are not rewritten: the Store resolves an NFC-equivalent alias by the stable folder ID; a full Store reopen confirms that the previous mailbox bytes are preserved. The ambiguous canonical aliases are rejected. A CREATE with an unassigned U+0378 gets BAD; the unassigned scalars are checked against Unicode 17.0.0 | The remaining response paths require acceptance; the RFC 9755 profile gate is not closed yet |
| Responses with mailbox names | LIST/LSUB/STATUS choose the modified UTF-7 in Legacy and the NFC UTF-8 quoted output after ENABLE; independent wire tests verified LIST/LSUB/STATUS and the composed output for the old decomposed folder record; the combined `LIST RETURN STATUS` after ENABLE verified the adjacency of the answers, the UTF-8 mailbox and the requested numeric fields. NAMESPACE still emits only the empty personal prefix | Keep the legacy representation for a client that has not enabled it; check the namespace/shared prefixes when such names appear |
| APPEND internationalized headers | The independent Python `imaplib` STARTTLS acceptance confirms the NO and the empty INBOX before ENABLE, then the APPENDUID and an exact RFC822 FETCH byte match after ENABLE; four unit tests cover a UTF-8 header, a high-bit body, empty headers and a missing separator | Keep the acceptance for the full profile gate; check the malformed header policy, the mailbox mode and CAPABILITY separately together with the remaining RFC 9755 obligations |
| SEARCH charset | After ENABLE the server answers BAD to an explicit CHARSET in the selected mailbox state; an independent STARTTLS client checked `CHARSET UTF-8` and the mixed case `cHaRsEt uTf-8`, then ran an implicit UTF-8 SEARCH ALL successfully and found a message by `SUBJECT "café"`. A unit test checks the refusal before the store routing | RFC 9755 SHOULD: keep BAD for any explicitly given CHARSET in the UTF8 mode; keep the evaluator behavior for the remaining criteria |
| LOGIN and SASL | LOGIN uses the legacy IMAP string grammar; AUTHENTICATE PLAIN is a separate typed path | Do not extend LOGIN for UTF-8 credentials; pass the UTF-8 identity through AUTHENTICATE and check the identity policy separately |
| message/global | An independent STARTTLS client enabled `UTF8=ACCEPT` before APPEND and confirmed `MESSAGE/GLOBAL`, the encoded `BODY[1]`, the decoded nested BODY/BINARY sections and the original RFC822 literal byte-exact | Keep this behavior under the remaining IMAP profile changes; RFC 9755 for IMAP4rev1 allows BODYSTRUCTURE as `message/rfc822` or as the RFC 3501 variant |
| UTF8=ONLY | Not supported and not advertised | A separate profile: it implies UTF8=ACCEPT, and the clients still send `ENABLE UTF8=ACCEPT`; it requires the rejection of the legacy modified UTF-7 and `NO [CANNOT]` for the UTF-8-dependent commands before ENABLE |

### Open RFC 9755 obligations and UTF8=ONLY

`UTF8=ACCEPT` is now advertised after the verification of its capability contract and
the main mode-dependent wire paths. This does not close the whole RFC 9755 profile. `UTF8=ONLY`
is not a synonym of `UTF8=ACCEPT` and requires a separate strict mode. See
[RFC 5198](https://www.rfc-editor.org/rfc/rfc5198.html) for the Net-Unicode
normalisation, the BOM and the stability of the Unicode tables. The RFC 9755 Sections 3-7
are the normative source for the next stage; a separate runtime acceptance
has not been run yet. The initial lexer check covers only the mode selection and
is not a separate acceptance item with PASS.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
