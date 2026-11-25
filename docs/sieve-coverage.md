# Sieve and ManageSieve

> **About this document.** Sieve and ManageSieve profile: advertised capability, RFC 5229 and RFC 5228, acceptance by independent peers.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Contents

- [Server profile and advertised extensions](#server-profile-and-advertised-extensions)
- [ManageSieve authentication: PLAIN and SCRAM](#managesieve-authentication-plain-and-scram)
- [Literal framing and TLS negotiation](#literal-framing-and-tls-negotiation)
- [Script catalog management: SETACTIVE, DELETESCRIPT, RENAMESCRIPT](#script-catalog-management-setactive-deletescript-renamescript)
- [Command syntax, string values and script names](#command-syntax-string-values-and-script-names)
- [Catalog quotas and execution error handling](#catalog-quotas-and-execution-error-handling)
- [RFC 3894 copy and repeated fileinto](#rfc-3894-copy-and-repeated-fileinto)
- [RFC 5230 vacation: base profile, remaining RFC gaps](#rfc-5230-vacation-base-profile-remaining-rfc-gaps)
  - [Vacation action: parameters, reply conditions and suppression history](#vacation-action-parameters-reply-conditions-and-suppression-history)
  - [Redirect path and the MIME reply contract](#redirect-path-and-the-mime-reply-contract)
  - [MIME structured values and Content-Type/Disposition parameters](#mime-structured-values-and-content-typedisposition-parameters)
  - [RFC 2231 extended payload and ISO-8859 profiles](#rfc-2231-extended-payload-and-iso-8859-profiles)
  - [windows-125x and windows-874 profiles](#windows-125x-and-windows-874-profiles)
  - [Script lexer, comments and match conditions](#script-lexer-comments-and-match-conditions)
  - [envelope, fileinto and redirect tests](#envelope-fileinto-and-redirect-tests)
  - [header test and string comparison](#header-test-and-string-comparison)
  - [Byte-level charset profiles and token case-insensitivity](#byte-level-charset-profiles-and-token-case-insensitivity)
  - [End-to-end wire test of variables substitution](#end-to-end-wire-test-of-variables-substitution)

### Server profile and advertised extensions

There is a bounded parser/executor for the mandatory profile, fileinto, redirect,
keep/discard, header/address/size tests and diagnostics. ManageSieve supports
STARTTLS, SASL PLAIN, SCRAM-SHA-1 and SCRAM-SHA-256, literals, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, GETSCRIPT,
SETACTIVE, DELETESCRIPT and RENAMESCRIPT.

The vetted list of advertised extensions and the selected gaps of the modern RFCs
are consolidated in a [separate inventory](sieve-extension-audit.md); the capability
does not advertise unsupported actions/tests.

### ManageSieve authentication: PLAIN and SCRAM

The RFC 5804 §2.1 string initial-response for AUTHENTICATE PLAIN is now accepted
also in bounded client-to-server literal form. The literal passes through the same
TLS-before-auth policy; an independent Linux check before TLS confirms the
argument parsing with a `NO "encryption required"` response, but does not check the password.
SCRAM-SHA-1 is wired to the TLS-only capability and ManageSieve AUTHENTICATE.
An independent Python TLS peer verifies the client proof, the server signature, the rejection
of an incorrect proof and a subsequent NOOP. Litterkeep app interoperability and the STARTTLS
SCRAM wire acceptance were not tested.

The RFC 5804 §2.1 challenge/response flow for PLAIN is supported as well: the absence of
an initial-response leads to an empty quoted challenge; the response may be a quoted
string or a bounded client literal, and `*` cancels the exchange. T262 verified the
non-synchronizing literal response with an independent Linux STARTTLS peer. T275 separately
verified the synchronizing literal after the challenge: the client received the mandatory
continuation before sending the payload and then successfully authenticated Bob. A repeated
login after a cancellation was verified as well. Full ManageSieve SASL compliance is still
not reached. T327 added typed bounded-worker access to the verifier through the
single owner of Accounts.
The native SHA-1/HMAC-SHA-1/PBKDF2 modules are now wired into the SCRAM StoredKey and
ServerKey derivation. The full example of RFC 5802 §5.1 confirms the client proof and
the server signature, and the iteration/salt bounds are limited to a single expensive KDF call.
An Argon2 verifier by itself is not a SCRAM verifier. SASLprep,
the Argon2 `.phc2` account storage and the safe legacy `.phc` migration are implemented;
T317 confirms the legacy transition with an independent SMTP TLS client, but that is not
a ManageSieve SASL verification. T319 added crypto primitive checks of the SCRAM proof and
of the server signature computation. T320 added a versioned randomly salted sidecar for
new accounts and strict verifier loading. T321 backfills the sidecar for
an existing `.phc2` only after a successful password login; the legacy `.phc`
migration also publishes a SCRAM sidecar. T322 added a bounded syntax parser;
T323 fixed its ASCII-only value restriction according to RFC 5802 §7 and added a
typed client-first parser with `n,,`, the SASLprep username, the nonce and optional
extensions. Server-first/client-final/server-final semantic parsing and the SCRAM
state machine are wired in T328. T324 added a server-first builder, T325
a client-final verifier, and T326 a typed server-final message. T328 wired the
ManageSieve session/reactor and verified the SHA-1 exchange with an independent TLS peer;
at the T328 checkpoint SHA-256/PLUS were not yet implemented. The details of the current state
are recorded in the [dependency ledger](dependencies.md).

The current profile advertises SCRAM-SHA-256 and, when a TLS 1.3 exporter is available,
SCRAM-SHA-256-PLUS; both use a separate SHA-256 verifier sidecar and an Accounts owner worker.
Independent Linux peers over implicit TLS and STARTTLS compare the server signature,
an incorrect proof, UNAUTHENTICATE and a repeated session run; the STARTTLS path also
verifies the empty SASL list before the upgrade and the capability after it. This is not a Litterkeep
app check. A separate Rustls peer verifies PLUS over a live implicit-TLS
socket and compares the server signature; the proof is computed with the shared
SHA/HMAC/PBKDF2 helpers of the project, so the cryptographic implementation is not independent. The TLS 1.3 `tls-exporter` is verified
between rustls endpoints and is used by ManageSieve only after a completed
TLS 1.3 handshake.

### Literal framing and TLS negotiation

For client-to-server literals the receiver first reads exactly the declared number of
octets, then the mandatory CRLF terminating the command line; only after that does it
start parsing the next pipelined command. This matches the framing of RFC 5804
§§1.1 and 4. A final string argument is not considered complete right after the
payload. An independent Linux STARTTLS wire test confirmed the expectation of the
delimiter and the preservation of the next command; the integration with the Litterkeep client was not
verified by this test. See [RFC 5804](https://www.rfc-editor.org/rfc/rfc5804.html).

The ManageSieve TLS negotiation follows RFC 5804 §§1.7, 2.2: before STARTTLS the
server advertises an empty SASL mechanism list, then after TLS it reissues the
capability block with SASL PLAIN and without STARTTLS. The same refreshed block
is sent after a successful AUTHENTICATE and includes the authenticated OWNER;
an explicit CAPABILITY preserves OWNER. A repeated STARTTLS is rejected in both
authenticated and unauthenticated encrypted sessions. An independent Linux
socket acceptance verifies the greeting, the explicit CAPABILITY, the TLS transition,
the PLAIN authentication and the post-auth transition. The implicit-TLS greeting is
unit-tested; a separate implicit-TLS network acceptance and the Litterkeep client
interoperability remain unverified. The capability generation always returns
`VERSION "1.0"` with `IMPLEMENTATION` and `SIEVE`, as required for a server
implementing this RFC version; unit and Linux socket tests check the greeting
and the common post-transition capability path. The recommended RFC 5804
`UNAUTHENTICATE` extension is advertised in all states. Its command resets the
ManageSieve FSM and the separate authenticated-account owner of the reactor, keeps
TLS active, and returns the refreshed capabilities without OWNER. Unit tests
and the independent TLS socket acceptance verify the command denial after the
transition and the successful reauthentication on the same TLS session.
The RFC 5804 §2.13 NOOP is accepted before
authentication; an optional quoted tag is returned in the required `(TAG
"...")` response code with quoted-string escaping. Unit tests and the
independent `tests/blackbox/managesieve_noop_tag.py` Linux socket acceptance
verify the untagged/tagged NOOP, the escaping and LOGOUT. The NOOP literal tags are
supported in the synchronizing and non-synchronizing forms, while the other command
literal arguments remain unverified. The RFC 5804 §2.1 mechanisms PLAIN and
SCRAM-SHA-1 are advertised after TLS. T328 independently verified the SCRAM
client proof, the server signature, the invalid-proof response and the subsequent NOOP
recovery over implicit TLS. STARTTLS SCRAM and the Litterkeep app interoperability
remain unverified.

### Script catalog management: SETACTIVE, DELETESCRIPT, RENAMESCRIPT

The RFC 5804 §2.8 requires `SETACTIVE` to carry an `active-sieve-name`; the empty
string disables the active filtering. The parser now rejects a missing argument and
maps `""` to the `None` deactivation value of the store, while non-empty script names
remain unchanged. Parser tests cover missing, empty, ordinary and extra
arguments; the independent NOOP wire scenario also asserts that a missing
`SETACTIVE` is rejected as a syntax error. RFC 5804 §2.8 additionally says
that disabling with an empty string while no script is active is not an error, and
recommends `NONEXISTENT` for an unknown script. The missing-script errors of the Store now
retain a typed cause; ManageSieve returns `NO (NONEXISTENT)` without changing
the original `NotFound` kind used internally. The independent Linux TLS acceptance
`tests/blackbox/managesieve_setactive.py` verifies the empty deactivation before
the activation, the unknown-name rejection, the activation in LISTSCRIPTS, the deactivation of
that active script, the idempotent empty deactivation and the deletion after the disable.
This checks the server and the Store owner, not the Litterkeep app interoperability.

The RFC 5804 §2.10 forbids deleting the active script and recommends the `ACTIVE`
response code. The Store now carries a typed active-script failure to the
ManageSieve completion layer, which replies `NO (ACTIVE)` without mutating the
catalog. The same independent TLS scenario attempts DELETESCRIPT while the
script is active, confirms the error and the unchanged ACTIVE listing, then
deactivates and successfully deletes it. This is server/store wire coverage,
not the Litterkeep app interoperability.

The RFC 5804 §2.11 recommends `ALREADYEXISTS` when RENAMESCRIPT targets an
existing script and permits renaming the active script while keeping it active
under its new name. The typed Store collision errors now produce
`NO (ALREADYEXISTS)`; the Store reopen coverage verifies the collision atomicity and the
durable active-name transfer. The independent Linux TLS acceptance
`tests/blackbox/managesieve_rename.py` verifies the exact collision response,
both original scripts after the rejection, then activates and renames one script
and confirms its bytes and the ACTIVE projection. This is not the Litterkeep app
interoperability.

### Command syntax, string values and script names

The ManageSieve command tokenization now rejects malformed quoted strings instead
of silently dropping an invalid suffix: unterminated strings and unsupported
backslash escapes become syntax errors. The parser regression and the independent
Linux wire acceptance in `tests/blackbox/managesieve_noop_tag.py` verify both
forms while retaining the valid NOOP tag behavior. T328 has since implemented and
independently wire-tested the required SCRAM-SHA-1 mechanism. SCRAM-SHA-256/PLUS,
the STARTTLS SCRAM acceptance and the Litterkeep app interoperability remain open.

The RFC 5804 §1.2 and §4 define the command string values as quoted or literal. The
optional tag of NOOP accepts the bounded sync and non-sync literal-c2s forms;
the independent Linux wire acceptance checks the continuation exchange, the direct
non-sync payload, the exact TAG echoes and the pipeline preservation. GETSCRIPT,
DELETESCRIPT and SETACTIVE also accept final sync/non-sync literal-c2s values.
The zero-octet SETACTIVE literal maps to the same disable state as a quoted
empty string. The Linux unit and wire checks cover the command conversion and the framing.
PUTSCRIPT accepts a literal script name followed by a quoted or literal source;
both synchronization modes are covered by the Linux unit tests and the independent
wire coverage exercises a sync name plus a quoted source and a non-sync name plus a
non-sync source with a pipelined NOOP. An additional independent TLS acceptance
uses a non-sync literal name plus a sync source, then performs a successful
RENAMESCRIPT with two non-sync literal names and activates the result. After a
server restart against the same Store, LISTSCRIPTS preserves the renamed ACTIVE
entry and GETSCRIPT returns the byte-identical source. This proves the tested
literal mutation and persistence paths, not every command string position or the
Litterkeep application interoperability. The RENAMESCRIPT receiver unit tests also
cover the mixed forms, the empty quoted old name and the pipeline preservation.
HAVESPACE accepts a literal name plus one strict decimal size; the independent wire
coverage confirms the parser/state handling before the authentication and the pipeline
preservation, not the authenticated quota response. The RFC 5804 §2.1 declares both
the AUTHENTICATE mechanism and the optional initial response as strings, so either may
use the quoted or literal framing. The receiver now accepts a quoted or sync/non-sync
literal PLAIN mechanism followed by no response, a quoted response, or a
sync/non-sync literal response. It consumes the complete command before
returning the typed unsupported-mechanism result, preserving a pipelined NOOP.
The Linux unit coverage exercises a synchronizing mechanism, and the independent TLS
wire acceptance now separately checks the synchronizing literal mechanism and
response: the server sends a continuation after each marker, then authenticates
Bob. The same peer rejects a literal CRAM response and successfully executes
the following NOOP. The previous non-synchronizing literal/literal combination
also passed the independent wire acceptance.
The other command string positions not listed above remain unverified or unsupported.

The RFC 5804 §1.6 ManageSieve script names now reject the C0/C1 controls, U+2028/U+2029,
a leading BOM and the Unicode 17.0.0 unassigned scalars. The server accepts the
required 128 Unicode-character boundary (up to 512 UTF-8 octets) and does not
impose a filesystem-style slash restriction. `LISTSCRIPTS` emits the names as
quoted strings with the quote and the backslash escaped, so a stored name cannot forge
another response line. The Rust unit checks and an independent STARTTLS client
verify the Unicode/special-character round-trip, the CRLF/control rejection, the
128/129 boundary and the exact listing bytes. This does not claim that the complete
Net-Unicode NFC is now applied to the stored and transmitted script names. The incoming
decomposed names remain accepted and resolve through their NFC alias. The legacy
catalogs are normalized in memory; the canonical-name collisions fail closed rather
than silently dropping a script. The RFC 5198 private-use guidance remains
outstanding, as does the Litterkeep app interoperability.
See [RFC 5804 §1.6](https://www.rfc-editor.org/rfc/rfc5804.html#section-1.6)
and [RFC 5198 §2](https://www.rfc-editor.org/rfc/rfc5198.html#section-2).

### Catalog quotas and execution error handling

The ManageSieve script count, the per-script byte size and the serialized account catalog
are bounded in the Store. The RFC 5804 §1.5 requires a `NO` response containing
the `QUOTA` response code when an imposed quota rejects a command. The Store quota
failures now retain a typed source marker across the owner boundary, and the
mutation completion maps only that marker to `NO (QUOTA)`; even a filesystem
`StorageFull` error retains the generic failure response. The Store tests exercise the
script-count boundary and the reactor response tests distinguish the quota from the disk
errors. The independent Linux TLS acceptance
`tests/blackbox/managesieve_quota.py` uploaded 16 scripts, checked that the
17th receives the exact `NO (QUOTA) "quota exceeded"`, and verified that LISTSCRIPTS
still returns exactly the original 16. This does not verify the Litterkeep client
interoperability.

On a runtime error the partial delivery plan is discarded atomically and only
the implicit keep is applied; the committed diagnostic stores the account, the script version,
the transaction ID and the error type. The offline command `sieve-errors <data-dir>
<account>` prints TSV with a column `outcome=implicit-keep-only`, explicitly reflecting
the absence of partially applied Sieve actions. The Store regression verifies the
durable diagnostic and the fallback after a reopen; the CLI unit regression
verifies the report line. An independent network scenario confirms the implicit
keep on the redirect-limit and the redirect-loop; after stopping the server the command
was run against the same persistent store and confirmed both committed CLI lines.
The server now publishes the effective limit `MAXREDIRECTS "16"` in every
ManageSieve capability block. The capability and the executor use a single
`MAX_REDIRECTS_PER_EVALUATION`; the unit boundary test confirms 16 successful
actions and the rejection on the 17th.

### RFC 3894 copy and repeated fileinto

The RFC 3894 `copy` is implemented for `fileinto` and `redirect`; the script must explicitly
declare `require "copy"`, and `fileinto` additionally requires `fileinto`.
`fileinto :copy` and `redirect :copy` add an action without cancelling the implicit
keep. The unit tests verify the capability gating and the interaction with a subsequent
`discard`. The independent ManageSieve -> SMTP -> IMAP acceptance verifies
`fileinto :copy`: the full RFC822 bytes in INBOX and `Invoices` match. The new
`tests/blackbox/sieve_redirect_copy_peer.py` activates `redirect :copy` through
ManageSieve, verifies the original local copy through Bob IMAP, stops the
single Store owner and drains the durable job to an independent SMTP peer.
The peer envelope, the exact RFC822 bytes, the MIME attachment and its decoded octets
match the local copy. An independent test peer was verified, not Litterkeep
or an arbitrary third-party MTA. The rule was checked against the RFC 3894 §3.

A repeated `fileinto` into the same folder is not an error and does not create a second
instance of the message according to the RFC 5228 §2.10.3. The Store-level regression verifies
the preservation of the single reference after a restart; the independent ManageSieve -> SMTP
-> IMAP acceptance executes two identical `fileinto :copy`, reads both
folders and compares the exact RFC822 bytes.

## RFC 5230 vacation: base profile, remaining RFC gaps

### Vacation action: parameters, reply conditions and suppression history

The parser/executor support the single `vacation` action and its RFC parameters;
ManageSieve advertises `vacation` and `vacation-seconds`. Besides the RFC 5230 `:days`,
the RFC 6131 `:seconds` is supported; both parameters cannot be given at once, and the seconds
are limited to the valid RFC range up to the local policy. The intervals are converted
into a common durable suppression expiry. The local policy limits the interval
from 60 seconds to 365 days, including `:seconds 0`, and uses seven days by
default. The Store creates a separate RFC 5322 response with `MAIL FROM:<>`, a durable
suppression claim, the loop/list/automated-message guards, an RFC 2047 Subject and a
quoted-printable UTF-8 body. `:mime` is validated taking the current 7BIT-only
SMTP transport into account. The unit and
Store tests verify the generation, the safety and the durable suppression. The independent
Linux acceptance in `./data/g17-sieve-vacation-rfc45` activated a script through
ManageSieve, delivered three messages through SMTP, passed exactly one of the two
allowed vacation replies to an independent SMTP peer and read all three original
RFC822 messages through IMAP after a restart. The peer verified the null reverse-path, the
address and the reply fields. The test confirms the Sieve/Store/SMTP/IMAP integration, but not the integration with the
Litterkeep app or an arbitrary third-party MTA.

According to the RFC 5230 §4.5 a reply is allowed only when the owner address is listed in To,
Cc, Bcc, Resent-To, Resent-Cc or Resent-Bcc. The final RCPT must be a known
owner address, and `:addresses` adds the additional addresses without
restricting the reply to them alone. The unit and network acceptance verify the
additional alias, the duplicate suppression and the rejection on a foreign To.

The suppression history survives a reopen, is recorded only for a committed
transaction ID and has a test for a capacity of at least 1000 response identities. The
queue payload and the claim are published in a single Store transaction.
The RFC 5230 §4.2 capacity regression restored 1000 unique claims with
the maximum allowed lengths of all three keys; a separate recovery
check removes an incomplete tail. When the 8 MiB limit is reached the Store compacts only
the committed claims, keeps the last entry for each key, excludes the
expired claims and deletes the oldest first, without dropping below 1000 entries.
The replacement journal uses a temporary file and a backup; the recovery is tested
for the failures before and after the publication of the new snapshot. The Windows suite: 358 tests;
the Linux Docker target: 399 tests, strict Clippy and the release build. A physical disk/FS failure
during the sync and the rename was not modelled.
The profile also verifies the requirements of the RFC 5230:
the deduplication of the replies by the sender address and the vacation reply identity,
the `:days` period, a maximum of at least 1000 history entries, the restrictions
on the auto-replies for the daemon/list messages, one vacation action per script and
the preservation of the implicit keep. The generated reply uses an empty SMTP
reverse-path and the reply headers/links defined by the standard.
In `Auto-Submitted` the recognition of `no` trims only the RFC WSP (SP/HTAB), so that
a Unicode NBSP does not lift the loop protection; the malformed NBSP values leave the
auto-reply suppressed. The keyword is now parsed into a typed value with the RFC 3834
CFWS comments and the RFC 2045 token/quoted-string optional parameters; one
syntactically valid `no` allows a reply, while the other keywords, the malformed
values and a repeated field suppress it. The parser checks are unit-only; a separate
runtime acceptance for these variants did not exist initially. The grammar was checked against
[RFC 3834 §5.1](https://www.rfc-editor.org/rfc/rfc3834.html#section-5.1) and
[RFC 2045 §5.1](https://www.rfc-editor.org/rfc/rfc2045.html#section-5.1).
A separate Linux Docker acceptance `tests/blackbox/sieve_autosubmitted_wire.py`
passed eight independent input messages over SMTP through an active ManageSieve
script: two valid `no` with CFWS/parameters produced exactly two replies, while the
automated/extension keywords, a malformed parameter, a duplicate field and an NBSP
produced none. All eight original messages were read through IMAP; an independent
SMTP peer verified the null reverse-path and both envelope destinations. An independent
client forced `SMTPUTF8` for all the input messages, although the capability
is not advertised; this is a server-side wire acceptance, not a conformance check of
the declared SMTPUTF8 or of a Litterkeep client.

### Redirect path and the MIME reply contract

An ordinary Sieve redirect still references the blob of the incoming message, whereas
`Store::enqueue_outbound_payload` creates a separate transaction/blob for the
generated message. The third-party MTA integration is not verified. The requirements
were checked against the RFC 5230 §§4.1-4.7 and 5.1-5.8.

The RFC 5230 `:mime` CHECKSCRIPT now rejects a repeated
`Content-Transfer-Encoding` and a body that is not decoded by the declared
7bit/base64/quoted-printable. The common typed MIME decoder is used;
its base64 checks the finality of the padding and the zero pad bits. A unit regression
covers the invalid characters, the data after the padding, an incorrect QP escape,
the non-ASCII 7bit and the duplicate CTE. The independent
`tests/blackbox/managesieve_vacation_mime.py` over TLS ManageSieve confirmed
the CHECKSCRIPT for the correct 7bit, quoted-printable and Base64 entities, as well as
the rejection of a corrupted Base64, a duplicate CTE and an encoded multipart.
The common CTE parser accepts only one ASCII token with an outer SP/HTAB; the Unicode
whitespace and the tspecials are not normalized into a known encoding name. The RFC comments
in a MIME entity are removed before the token is parsed. An independent CHECKSCRIPT
confirmed the acceptance of `base64` with comments and the rejection of an NBSP after the token.
The `MIME-Version` is also checked before storing a :mime script: at most
one version 1.0 value is allowed, the RFC 822 comments are removed without gluing
through a real whitespace inside the value; an unknown version, the repeated fields
and a whitespace-split declaration are rejected. This prevents emitting a MIME
reply with an unsupported or incorrectly normalized version. The boundaries
are verified by the unit tests and by an independent ManageSieve CHECKSCRIPT acceptance.

The separate `vacation_mime` validator applies the restrictions of the RFC 2045 §§2.7, 6.7
and 6.8: a 7bit body cannot contain NUL, a bare CR/LF and the lines longer than 998
octets; Base64/QP require CRLF, the permitted encoded octets and the lines of at most
76 characters. QP also forbids the unencoded trailing spaces and tabs.
The unit cases verify both line boundaries, NUL, a bare LF and an SP before a soft break.
The RFC 2045 [§6.7](https://www.rfc-editor.org/rfc/rfc2045.html#section-6.7)
allows the whitespace before the terminating `=` of a soft line break:
this is a part of the `qp-segment`, not transport-padding. An independent ManageSieve CHECKSCRIPT
accepts such a correct soft break and rejects a malformed hex escape and
an unencoded trailing SP/HTAB. These cases verify the end-to-end parser
Sieve,
the MIME validator and the wire response, but not all the edge-case line lengths that
remain unit-only.

### MIME structured values and Content-Type/Disposition parameters

The RFC 2045 §6.4: the encoded transfer is forbidden for the composite `multipart/*` and
`message/*`. The validator parses `Content-Type` into a typed media type, rejects
base64/QP on these composite entities and a repeated Content-Type. The unit cases
verify `multipart/mixed`, `multipart/related`, `message/partial`, a comment
between the type and the slash, a quoted boundary with the parentheses, a permitted 7bit
multipart and a repeated field. The independent ManageSieve acceptance additionally
confirmed the rejection of an encoded `multipart/mixed`; the remaining listed
structured-value boundaries remain unit-only.

The common MIME structured-value parser removes the nested RFC comments outside
the quoted-strings and checks `type/subtype` as two non-empty RFC token parts
separated by exactly one slash. The unknown but syntactically correct types
are preserved as a typed `Other`; the empty values, the values containing whitespace/tspecials and
an extra slash are rejected. The unit tests cover the comments around the
separator and inside a quoted parameter, the nested/unterminated comments and the
incorrect tokens. An independent TLS ManageSieve CHECKSCRIPT now also
accepts `multipart (outer(inner))/related` with a quoted boundary containing the
parentheses, and rejects a subtype with a forbidden tspecial. The remaining boundaries from
the unit set have not yet received a separate wire case.

For the Content-Type/Content-Disposition parameters the RFC 2045 §5.1 attribute=value,
token/quoted-string and the list of the tspecials are checked. The RFC 2231 attribute
grammar excludes the reserved characters, and the continuation numbers must
start at the section 0 and continue without the leading zeros. The unit cases cover
the spaces/tspecials in the unquoted values, the corrupted names, an inner unescaped quote,
a correct escaped DQUOTE, an empty quoted value and a
filename continuation. A network MIME acceptance of these new boundaries was not performed.
T329 added a network regression for a conflicting regular/extended filename in
the Content-Disposition and confirmed that the MIME tree does not publish the disputed name,
while preserving the original RFC822 bytes. This is a conservative ambiguity policy, not a
normative precedence rule of the RFC 2231. The next acceptance T330 checks
the RFC 2183 §2.8: an unknown charset of a single `filename*` is skipped, while
a known `size=12` and the disposition are preserved; the RFC822 literal remains exact.
An unknown disposition type must be projected as `attachment`; the local
unit tests and the IMAP STARTTLS acceptance T331 confirm the fallback and the preservation of the
filename with a byte-exact preservation of the original RFC822 literal.
The RFC 2183 `size` accepts only the non-empty decimal digits: `size=12x` is
discarded separately, preserving a valid `filename`; this is covered by the unit and IMAP
STARTTLS acceptance. The date parameters are closed by the narrow checks T333-T336;
T337 separately confirms that the RFC 2183 MIME preserves a numeric-only timezone,
even when the RFC 5322 Date parser accepts the registered obsolete zones.
T333 added the quoted RFC 2183 date parameters with a calendar validation,
a numeric timezone, a weekday comparison and the leap second `:60`; the unit and IMAP STARTTLS
verified the accepted and the rejected forms. T334 added an RFC 5322 pivot for a numeric-
zone obsolete 2/3-digit year. The named zones and some other obsolete
RFC 822 variants are still uncovered, so the full date-time compatibility is not
claimed.
T335 fixed the RFC 5322 widths of the day and clock fields, including the optional seconds and
`:60`; an independent IMAP BODYSTRUCTURE test rejects the ambiguous and the excessively
wide numeric fields.
The whitespace around the `=` in the parameters remains open and is not tightened separately.

### RFC 2231 extended payload and ISO-8859 profiles

The RFC 2231 extended payload decoder accepts only the attribute-chars or the
percent-encoded octets, and the hexadecimal digits in the escapes are accepted regardless of the
case per the ABNF. The unit tests verify the lowercase/mixed-case UTF-8 escapes in
the common parser and in the Sieve vacation `:mime` validator; an independent IMAP wire test
confirms the decoded filename in the BODYSTRUCTURE. The mixed encoded and
unencoded continuation segments are checked. The charset is decoded by an explicit allowlist of the known
profiles: US-ASCII, UTF-8, UTF-32, UTF-32BE, UTF-32LE,
ISO-8859-1, ISO-8859-2, ISO-8859-3, ISO-8859-4,
ISO-8859-5, ISO-8859-6, ISO-8859-7, ISO-8859-8,
ISO-8859-9, ISO-8859-10, TIS-620 (ISO-8859-11), ISO-8859-13, ISO-8859-14,
ISO-8859-15, ISO-8859-16,
Windows-1250, Windows-1251, Windows-1252, Windows-1253, Windows-1254,
Windows-1255, Windows-1256, Windows-1257, Windows-1258, Windows-874,
UTF-16, UTF-16BE and UTF-16LE. The UTF-16 and UTF-32 profiles are available for the RFC 2047
and RFC 2231, but are excluded from the MIME `text/*` decoding during the search: UTF-16 by
the RFC 2781, and UTF-32 because of the IANA suitability registration. The RFC 2231 initial
section may omit the charset and the language:
an untagged ASCII payload is accepted, while the high-bit octets without a charset
are rejected, since the Unicode API cannot safely guess their encoding.
The language tag is checked against the well-formed BCP 47 structure of the RFC 5646 and is preserved
without a case change in the `StructuredValue` and in every MIME tree entity. The check
does not prove the subtag membership in the dated IANA Language Subtag Registry.
The RFC 2231 charset/language regressions are unit-only.

The ISO-8859-15 decodes all 256 octets according to the table of the Unicode Consortium and
accepts the IANA labels `ISO_8859-15`, `Latin-9` and `csISO885915` together with
the preferred name. The full-byte regression verifies the euro and all the eight
differences relative to the ISO-8859-1; the connection through a network client was not separately
verified. The ISO-8859-16 also decodes the full 256-octet Latin-10 profile,
including the Romanian comma-below letters, the Central European differences and the IANA aliases
`ISO_8859-16:2001`, `ISO_8859-16`, `latin10`, `l10`, `iso-ir-226`,
`csISO885916`; the mapping is covered by a full-byte oracle, but the network connection was not
verified.

### windows-125x and windows-874 profiles

The registered IANA charset [windows-1250](https://www.iana.org/assignments/charset-reg/windows-1250)
is decoded according to the [full Microsoft/Unicode table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1250.TXT): all the 256 octets are compared independently, the five undefined positions
0x81/0x83/0x88/0x90/0x98 are rejected, and the IANA alias `cswindows1250` is accepted.
The `cp1250` is intentionally not treated as an alias, according to the registry. The new checks are unit-only.

The IANA charset `windows-1253` is implemented by a separate Greek mapping: all the bytes
are checked against the Microsoft/Unicode table, the 17 undefined octets are not substituted with a
replacement character, the alias `cswindows1253` is accepted, and the `cp1253` is not
accepted. A network client acceptance for this charset was not performed.

The IANA charset [windows-1254](https://www.iana.org/assignments/charset-reg/windows-1254)
is decoded according to the full [Microsoft/Unicode CP1254 table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1254.TXT).
An octet-by-octet oracle verifies all the 256 values and the seven undefined positions;
the Turkish I/Ğ/Ş differences and the alias `cswindows1254` are verified, the `cp1254` is not accepted.
A network client acceptance for this charset was not performed.

The IANA charset [windows-1255](https://www.iana.org/assignments/charset-reg/windows-1255)
is decoded according to the full [Microsoft/Unicode CP1255 table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1255.TXT).
The oracle compares all the octets, including the Hebrew points, the Yiddish ligatures, the RTL marks
and the 23 undefined positions; the `cswindows1255` is accepted, the `cp1255` is not considered a
registered alias. A network client acceptance was not performed.

The IANA charset [windows-1256](https://www.iana.org/assignments/charset-reg/windows-1256)
is decoded according to the full [Microsoft/Unicode CP1256 table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1256.TXT).
An independent oracle verifies every octet, including the Arabic extended
letters, the combining marks, ZWNJ/ZWJ and LRM/RLM; the `cswindows1256` is accepted,
the `cp1256` is not a registered alias. A network client acceptance
was not performed.

The IANA charset [windows-1257](https://www.iana.org/assignments/charset-reg/windows-1257)
is decoded according to the [Microsoft/Unicode CP1257 mapping](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1257.TXT).
The full range of the octet values is verified, the 12 undefined positions are rejected;
the Lithuanian/Latvian diacritics and the `cswindows1257` are checked, whereas
the `cp1257` is not accepted as an unregistered alias. A network client
acceptance was not performed.

The IANA charset [windows-1258](https://www.iana.org/assignments/charset-reg/windows-1258)
is decoded according to the [Microsoft/Unicode CP1258 mapping](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1258.TXT).
The octet-by-octet check covers all the values, including the combining grave, acute,
hook, tilde and dot-below; the Vietnamese sample preserves the decomposed scalar order
without a normalization. The `cswindows1258` is accepted, the `cp1258` is not an alias per IANA.
A network client acceptance was not performed.

The IANA charset [windows-874](https://www.iana.org/assignments/charset-reg/windows-874)
is implemented according to the [Microsoft/Unicode CP874 table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP874.TXT)
separately from the TIS-620/ISO-8859-11. The full octet space and the 31 undefined positions
are covered by the tests; a Thai sample and the baht sign verify the assignment, which differs
from the ISO-8859-11. The `cswindows874` is accepted as an IANA registry alias, the `cp874`
according to the registration is not an alias. A wire acceptance was not performed.

The registered IANA charset windows-1252 is now decoded as a separate
profile, without mixing it with the ISO-8859-1. All the 32 octets of the range
0x80..0x9F are verified, an RFC 2231 filename with an encoded Euro/en dash and the MIME body decoding in the
IMAP SEARCH projection. Only the
canonical MIME name windows-1252 is supported; the WHATWG aliases such as cp1252 are not accepted
as the IANA MIME labels. The new checks are unit-only.

### Script lexer, comments and match conditions

The quoted and `text:` multiline strings accept CRLF; the multiline form preserves the
terminating CRLF of the body, removes one dot-stuffing octet and is limited to the 65 536
bytes. For the `text:` the SP/HTAB and the hash-comments are allowed before the mandatory
CRLF, but not the bracket-comments. The lexer unit tests and the independent ManageSieve
CHECKSCRIPT verify respectively the payload/dot-stuffing/empty body and
the acceptance of a correct script with the rejection on a missing terminator.
In the base Sieve the `require` is allowed only before the first command, but several
consecutive `require` declarations are permitted; a unit regression verifies both the rules
per the RFC 5228 §3.2. A quoted-string preserves the CRLF, allows a `quoted-other`
escape with the backslash removed and rejects the NUL; from the whitespace
separators outside the strings only SP/HTAB/CRLF are accepted. The `allof`/`anyof`
accept a non-empty comma-separated `test-list` in the parentheses; the square
brackets remain the form of a non-empty `string-list`, where a comma is mandatory between
the elements and is not allowed at the edges. The unit and ManageSieve CHECKSCRIPT acceptance
verify these boundaries per the RFC 5228 §§2.4.2, 5.2-5.3 and 8.1-8.2. This does not imply
the full coverage of the RFC 5228 grammar and the extensions.
A hash-comment must end with CRLF; the preliminary check also rejects the
single CR/LF and the NUL in the quoted strings and in both kinds of the comments. The unit and
ManageSieve CHECKSCRIPT acceptance verify a correct CRLF and the rejection of a bare LF
per the RFC 5228 §§2.1-2.3, 8.1.
The `:user` and `:detail` are implemented as the RFC 5233 `subaddress`; both require an explicit
`require "subaddress"`, and the ManageSieve greeting and CAPABILITY declare this
extension. The server uses the first `+` in the local-part as the separator: without it
the `:detail` does not create a value for the comparison, and the `user+@domain` creates an empty
detail. The CHECKSCRIPT and the address unit tests verify these cases. The other subaddress separator
policies are not configurable yet. The ADDRESS-PART, COMPARATOR and
MATCH-TYPE before the positional lists are accepted in any order, as
prescribed by the RFC 5228 §2.6.2; a repetition of one such argument is rejected.
A unit test verifies the case-sensitive semantics of `i;octet`, while the ManageSieve
CHECKSCRIPT and the SMTP -> Sieve -> IMAP acceptance verify the grammar and the execution of a
reordered script. The `:matches` compares a UTF-8 byte stream with a width wildcard `?`
of one octet for the mandatory comparators `i;octet` and `i;ascii-casemap`;
the `\*`/`\?` in a Sieve string become a literal wildcard after the lexical unescape.
The unit and SMTP -> Sieve -> IMAP acceptance verify these rules per the RFC 5228 §2.7.1.
The `address` uses the common RFC 5322 parser, checks the known address-list
fields (including the mandatory RFC 5228 From/To/Cc/Bcc/Sender/Resent-From/
Resent-To), the RFC 5322 reply/resent fields and the `Disposition-Notification-To` from
the RFC 8098 §2.1, whose value is defined as a `mailbox-list`. The mailboxes inside a
group are compared, the group name is not. The unit and an independent
ManageSieve -> SMTP -> IMAP acceptance verify the MDN field; the message lands in the
Invoices, and the original INBOX copy matches byte-for-byte thanks to the implicit keep and
the `fileinto :copy`. It is also verified that a match with a Subject of the same name
does not substitute an address match. The field matrix remains a bounded allowlist:
the arbitrary custom address-list headers are not confirmed.

### envelope, fileinto and redirect tests

The Sieve `envelope` supports `from` and `to`, the `ADDRESS-PART`, the match-type and the
comparator per the RFC 5228 §5.4; the script must declare `require "envelope"`.
ManageSieve advertises the capability. The `from` is taken from the original SMTP MAIL FROM before the trace header;
the null reverse-path is compared as an empty string regardless of the ADDRESS-PART.
The `to` is taken from the RCPT TO matched to the local mailbox, and is not derived from
the MIME `To`. With several SMTP paths to one mailbox the delivery plan
is computed once from the first matching RCPT. The unit tests verify the
capability, the from/to and the null sender; an independent SMTP -> Sieve -> IMAP acceptance
uses a real RCPT TO as the condition for placing the message into a folder.

The supported optional Sieve action `fileinto` is rejected during the static
validation if the script has not declared `require "fileinto"`. This is verified by a unit
regression and by an independent ManageSieve CHECKSCRIPT; the same network acceptance
confirms the delivery with the declared capability.

The Sieve `redirect` checks the mailbox by the RFC 5228 §2.4.2.3 grammar through the common
RFC 5322 parser: one `addr-spec` or a quoted phrase before
the `<addr-spec>` is allowed. The mailbox lists, the source routes, the groups and the malformed addresses
are rejected during the static validation. The unit tests and a separate independent
ManageSieve CHECKSCRIPT `tests/blackbox/managesieve_redirect_validation.py`
verify the acceptance and the rejection. The end-to-end redirect delivery after the completion of the source
Store through a durable queue to an independent SMTP peer was verified in T119:
the envelope, the full RFC822 octets with the local implicit-keep copy and the
MIME attachment bytes are compared. This does not confirm the interaction with the Litterkeep or
an arbitrary external MTA.

### header test and string comparison

The Sieve `header` decodes the RFC 2047 encoded-word with the common MIME decoding before
the comparison in UTF-8 and ignores the leading/trailing SP and HTAB per the RFC 5228 §5.7.
The unit covers the full conversion of the ISO-8859-1 and ISO-8859-2, the ASCII subsets
of the other registered preferred ISO-8859 MIME labels, a UTF-8
encoded-word and the edge spaces. For the ISO-8859-2 the whole range
A0..FF and the Polish text are tested. The ISO-8859-3 decodes the assigned positions and rejects
the undefined octets A5, AE, BE, C3, D0, E3 and F0; the other unsupported high-bit
tables are not converted. The ISO-8859-4 checks all the assigned positions and
rejects the undefined octets A5, AE, BE, C3, D0, E3 and F0. The ISO-8859-5 checks
all the 256 bytes, including the C1 controls and the Cyrillic rendering, separate from
the Windows-1251. The ISO-8859-6 checks the Arabic letters and punctuation, preserves the
ASCII/C1 controls, and rejects the undefined positions as an error. The ISO-8859-7
checks the Greek, the Euro, the final sigma and the undefined bytes AE, D2, FF.
The ISO-8859-8 checks the Hebrew text, the directional marks, the ASCII/C1 controls,
all the assigned positions and the rejection of the undefined octets. This is the visual-order base profile;
the RFC 1556 -i/-e remain separate unsupported profiles.
The ISO-8859-9 checks all the 256 octets against the Unicode table, the Turkish differences
and the preservation of the C1 controls; the IANA aliases `latin5` and `l5` are also accepted.
The ISO-8859-10 checks all the 256 octets of the Nordic/Latin-6 mapping against the Unicode table,
the ASCII/C1 controls, the Icelandic/Nordic text and the IANA aliases `latin6`/`l6`.
The TIS-620 uses the official sparse mapping with the NBSP and the Thai letters; it tests
a Thai sample, the C1 range and every assigned/undefined octet. The preferred IANA
name TIS-620 points to the ISO-8859-11 as an alias; the undefined DB..DE and FC..FF
are rejected.
The ISO-8859-13 checks the Baltic Rim Unicode mapping for all the 256 bytes,
including the C1 controls, the Baltic letters and the modified quotes/diacritics;
the IANA preferred label and the registered `csISO885913` are accepted.
The ISO-8859-14 checks the full Celtic mapping, the letters differing from the Latin-1,
the C1 range and the IANA aliases `latin8`, `l8`, `iso-celtic`, `csISO885914`.
This implements the mandatory ASCII-subset minimum of the
[RFC 5228 §2.7.2](https://www.rfc-editor.org/rfc/rfc5228.html#section-2.7.2),
not the full set of the converters. An independent
ManageSieve -> SMTP -> IMAP acceptance sends an encoded UTF-8 Subject and
checks its Unicode matching when choosing the folder. The RFC 2047 decoder also
requires a non-empty `encoded-text` of the printable ASCII octets without the SP
and the `?`; the Q-escape `=01` remains a valid encoded octet. The unit tests cover
the raw whitespace/control/non-ASCII and the preservation of a corrupted word in the header
projection; the new grammar cases are not separately verified over the network. The common header
parser recognizes a whole encoded-word only at the beginning of the field-body or after the
SP/HTAB and leaves an embedded/concatenated token as the plain text per the RFC 2047 §6.1.
The RFC 2231 §5 encoded-word form `charset*language` is available through the typed
`decode_with_metadata`: the BCP 47 language is preserved separately from the decoded text.
The ordinary `decode`, `decode_header` and `decode_phrase` return only the text;
the raw message headers remain the source of the metadata. The new language-tag
boundaries are covered unit-only, the current IANA registry validity is not checked.
This boundary is still covered unit-only. The header parser also ignores the SP/HTAB
between the field-name and the `:`, as required by §2.4.2.2;
a network test confirms that the `Subject \t:` is routed by Sieve, and the IMAP
FETCH returns the original byte header without a rewriting. The header matcher
accepts an empty key and distinguishes the `:contains` (the empty substring is contained in
the value of an existing field) from the `:is` (an empty value is not equal to a non-empty one).
An invalid field name in a `header` test is accepted as never matching,
not as a script error; these cases are verified by a unit test and a ManageSieve CHECKSCRIPT.

For the `:contains` with the mandatory comparator `i;ascii-casemap` the KMP is applied:
the search runs in O(n + m), keeps the O(m) normalized key bytes and a prefix table,
without copying the whole header value. The comparison folds only the ASCII
bytes; the other UTF-8 octets remain exact. An adversarial unit test checks
a linear miss on a large value and a long repeating key; in one
local Windows Rust debug run it changed from 1.29s to 0.01s. This is one
measured regression configuration, not a production benchmark. The `i;octet`
`:contains` and the glob `:matches` are not affected by this optimization; the full
RFC 5228 profile and the acceptance through the Litterkeep application are not confirmed.

### Byte-level charset profiles and token case-insensitivity

The IANA charset [IBM866](https://www.iana.org/assignments/charset-reg/ibm866)
is decoded according to the published [Unicode CP866 table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP866.TXT).
All the 256 octets are compared with a separate expected mapping; the Russian
letters, the Ё/ё, the Ukrainian Є/є, the pseudographics, the currency sign and the NBSP are checked. The registered
names IBM866, CP866, csIBM866 and 866 are accepted. The check covers
the MIME charset decoder with the unit tests; the network use and the client
connection were not separately verified.

The RFC 5228 §2.4.1 requires the insensitivity of all the non-string tokens to the case,
preserving the case of the quoted and multiline strings. The parser converts the commands, the tests,
the capability declarations, the match/address/size tags and the comparator identifiers into
the typed enums before the AST is built. The regression tests execute the upper and
the mixed case for HEADER, FILEINTO and ADDRESS, while simultaneously confirming
that the folder name `Invoices/2026` remains exact. This is only a parser and
executor unit check; an independent ManageSieve wire client and the Litterkeep application did not take part in it.
The full RFC 5228 compatibility is not claimed.

The registered labels UTF-32, UTF-32BE and UTF-32LE are decoded as the Unicode
scalars in the RFC 2047 encoded-word and the RFC 2231 extended values. The generic UTF-32 per
IANA uses the big-endian without a BOM and removes only the BE BOM; the invalid
surrogates and the scalars above U+10FFFF are rejected. These labels are not allowed for
the MIME `text/*` decoding, because IANA marks them as unsuitable for the MIME text.
The unit tests verify the decoder, the RFC 2047 and the RFC 2231 parser integration.
An independent STARTTLS SMTP -> file store -> IMAP acceptance also
confirms the preservation of the encoded Subject and the delivery of the decoded attachment filename
`😀` in the BODYSTRUCTURE. The check was performed by the script
`tests/blackbox/smtp_imap_utf32_charset.py`; the POP3, Sieve, ManageSieve and the
Litterkeep application did not take part in this scenario.

A separate integration regression checks the IBM866 not only with the decoder:
the RFC 2047 Base64 encoded-word of the subject `Привет` is decoded from the CP866 octets, and
the RFC 2231 `filename*` returns `Привет.txt`. This is a parser unit acceptance;
the sending of such a message through SMTP and the viewing through IMAP/POP3 were not verified.

### End-to-end wire test of variables substitution

In T443 an independent Python wire client uploaded a script through the ManageSieve
CHECKSCRIPT/PUTSCRIPT/SETACTIVE, where the comparator and the `envelope` part were expanded
from the variables, then sent a message through SMTP and read the result through
IMAP: the single copy ended up in the `Invoices`, and the INBOX remained empty. This
test intentionally confirms that the greeting does not advertise a partial `variables`;
the script requires the capability manually. Consequently, the internal server
path is verified, but not the interoperability of a compatible client and not the readiness of the extension
for the advertisement. In the existing literal black-box the completion of the RFC 5804
literal was fixed: the client now sends the terminating CRLF after the declared bytes; the expected
rejection of the deletion of the active script is checked against the RFC `NO (ACTIVE)`. Both acceptances
were performed by an independent client against a Linux Docker listener; the Litterkeep application
was not used.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
