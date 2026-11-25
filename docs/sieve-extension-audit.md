# Inventory of modern Sieve extensions

> **About this document.** Inventory of modern Sieve extensions: what is advertised, what is implemented inside, what is not started.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Contents

- [Scope of the inventory and the advertised capability](#scope-of-the-inventory-and-the-advertised-capability)
- [Status of the extensions and the next necessary boundaries](#status-of-the-extensions-and-the-next-necessary-boundaries)
- [Server position and implementation history](#server-position-and-implementation-history)
- [Current remainder of RFC 5229 Variables](#current-remainder-of-rfc-5229-variables)
- [State of RFC 5228 encoded-character](#state-of-rfc-5228-encoded-character)
- [Historical record T421-T429](#historical-record-t421-t429)
- [Evolution of VariableStore and matcher backtracking](#evolution-of-variablestore-and-matcher-backtracking)
- [Base RFC 5228: address lists](#base-rfc-5228-address-lists)

### Scope of the inventory and the advertised capability

Reconciliation date: 2026-09-29. The inventory is limited to the extensions that noticeably change
the compatibility with the modern Sieve clients. It is not a complete list of the
IANA registry and does not replace the separate acceptance for each extension.

The real `SIEVE` capability from `src/managesieve/session.rs`:
`fileinto subaddress envelope copy vacation vacation-seconds`.
A unit test of the capability confirms exactly this list. An extension is
considered supported only when it is advertised and has a parser,
the validation, the typed execution semantics and the profile tests.

### Status of the extensions and the next necessary boundaries

| Capability | RFC | Status in litterkeep | Next necessary boundary |
|---|---|---|---|
| `fileinto` | RFC 5228 | Implemented, advertised | The `:create` scenarios and the modern mailbox selectors remain outside the basic support. |
| `subaddress` | RFC 3598 | Implemented, advertised | Check all the admissible recipient-part variants and the inter-client address conventions. |
| `copy` | RFC 3894 | Implemented, advertised | Check the combinations with the actions and the delivery on an independent profile. |
| `vacation-seconds` | RFC 6131 | Implemented, advertised | The remaining vacation boundaries and the interaction with the extensions require a separate profile. |
| `variables` | [RFC 5229](https://www.rfc-editor.org/rfc/rfc5229.html) | Partial: the `set` AST/parser/runtime, the per-script store, the modifiers, the capture references and the `string` test work; the common condition/action strings, the comparator and the envelope-part arguments are expanded at runtime; the capability is not advertised | The variables in the arbitrary byte strings, a joint acceptance profile and the ManageSieve discovery/interoperability. |
| `encoded-character` | [RFC 5228 §2.4.2.4](https://www.rfc-editor.org/rfc/rfc5228.html#section-2.4.2.4) | Partial: `parse_octets` decodes `${hex:...}`/`${unicode:...}` in all the string tokens after the quoted escape and the dot-unstuffing; the matcher and the fileinto/redirect actions store the byte-valued arguments; the capability is not advertised | Run the octets through the vacation/set and the other AST/runtime boundaries; test the raw-byte ManageSieve scripts and the full RFC matrix; only then decide on the advertisement. |
| `body` | [RFC 5173](https://www.rfc-editor.org/rfc/rfc5173.html) | Not implemented, not advertised | The MIME-aware `:raw`, `:content`, `:text`; do not allow the matches across the part boundaries and limit the CPU/memory. |
| `reject`, `ereject` | [RFC 5429](https://www.rfc-editor.org/rfc/rfc5429.html) | Not implemented, not advertised | A Sieve evaluation before the SMTP acceptance of DATA is needed for the protocol-level refusal; a post-accept DSN is not equivalent to the `ereject`. |
| `imap4flags` | [RFC 5232](https://www.rfc-editor.org/rfc/rfc5232.html) | Not implemented, not advertised | Implement the `setflag`, `addflag`, `removeflag`, `hasflag` and `:flags` with the durable mailbox flag mutation. |
| IMAP events | [RFC 6785](https://www.rfc-editor.org/rfc/rfc6785.html) | Not implemented, not advertised | The IMAP flag support and a defined event context for the Sieve execution are required. |
| `mailboxid` | [RFC 9042](https://www.rfc-editor.org/rfc/rfc9042.html) | Not implemented, not advertised | A stable mailbox OBJECTID, the `:mailboxid` of `fileinto` and the `mailboxidexists` are required. |
| Mailbox metadata | [RFC 5490](https://www.rfc-editor.org/rfc/rfc5490.html) | Not implemented, not advertised | The `mailboxexists`, the metadata tests and the metadata-based processing. |
| Special-use delivery | [RFC 8579](https://www.rfc-editor.org/rfc/rfc8579.html) | Not implemented, not advertised | The `specialuse`, the `:specialuse` and the reconciliation with the typed special-use catalog are required. |

### Server position and implementation history

The RFC Editor points to the RFC 5229, 5429, 6785 and 9042 as the updates of the RFC 5228.
The table also includes the RFC 5173 and the basic widely used extensions;
the other Sieve standards, their dependencies and the errata require a further audit.
As of the date of this record the correct position of the server is a partial implementation with a narrow
list of the advertised extensions, not a full modern Sieve profile.

T421 added a helper in `src/sieve/matching/glob.rs` that returns the byte
captures for the `*` and the `?` as the existing whole-string matcher executes.
T422 wired these results into the per-script execution state: the capture zero
contains the matched input, the subsequent captures follow the order of the wildcards;
an erroneous match does not overwrite the state, and the composite tests
preserve the mandatory left-to-right order and the short-circuit.

### Current remainder of RFC 5229 Variables

**The current remainder of the RFC 5229 after T445.** The `set`, the `string`, the modifiers, the one
expansion pass, the numeric captures with the leading zeros, the unknown names and the
per-script scope are now wired to the parser/runtime. The `string` preserves the spaces,
compares both lists, updates the captures of a successful `:matches` and takes part in the
short-circuit. The conditions `header`, `address`, `envelope` and `exists` expand
their string arguments right before the evaluation, only if the
`variables` is required. The `fileinto`, the `redirect` and all the string fields of the `vacation` are expanded
at the point of the action and re-pass the path, mailbox and MIME checks; a redirect still
passes the Store allowlist/hop-limit. The RFC 5229 backslash quoting is already
removed by the lexer before the expansion; a regression covers the `\${foo}`, the `\\${foo}` and a
literal-dollar single-pass pattern. The compile-time AST pass rejects a namespace
if its capability is not required; the `${variables.unknown}` is considered an unknown
name and is expanded into an empty string. The string arguments of the comparator
are also expanded at the point of the condition and are resolved again only into the
supported enum values; an unknown result causes a runtime failure. The envelope-part
string list is also expanded at the point of the `envelope` test and is parsed
again into the `from`/`to`. The extension is still not advertised: the RFC 5228
`encoded-character` expansion (section 2.4.2.4) is partially implemented after T446,
but is not advertised because of the incomplete byte-valued action path. The production
`parse_octets(&[u8])` preserves the quoted/multiline string bytes; the `StringMatch`
keys and the `Condition::String.sources` in the AST are also byte-valued. The `is`, the `contains` and the
`matches` compare the octets without a lossy UTF-8, and the substring search uses
a linear KMP. A parser-to-runtime regression confirms a literal with the octets
`FF 80`. After T447 the arguments of the `fileinto` and the `redirect` are also preserved as
the octets in the AST; the runtime converts them into UTF-8 when checking the mailbox/address,
so a binary target is syntactically accepted, but ends in a typed runtime
error. The vacation/set AST and the `VariableStore` remain textual: a byte literal with
an interpolation marker still requires UTF-8. Therefore an arbitrary `hex` octet
does not yet pass the whole script path, and a variable cannot safely be
interpolated into an arbitrary binary literal.
The normative order:
the quoted-string escape and the dot-unstuffing, then the `encoded-character`, then the RFC 5229
variable expansion. It is not allowed to apply a lossy UTF-8 to the `hex` or to declare the Variables
available before the byte-exact tests for the whole profile. T443 added a
limited ManageSieve -> SMTP -> IMAP wire acceptance for the dynamic selectors,
but the test manually uses the `require "variables"`, which is not advertised.
This does not confirm the client compatibility, the full RFC conformance matrix or the
readiness to enable the Variables capability. T445 checks the parser-to-condition
runtime path at the unit level, but not a ManageSieve wire acceptance with the raw bytes and not the
full Sieve script path through the actions.

### State of RFC 5228 encoded-character

**The RFC 5228 encoded-character after T447.** The initial `require
"encoded-character"` enables the substitution of the `${hex:...}` with the octets and the `${unicode:...}`
with the UTF-8 scalar values after the lexical unescaping and the dot-unstuffing. The names of a
sequence are case-insensitive; the malformed input remains literal, a nested
sequence is handled per the RFC, a well-formed invalid Unicode scalar
sequence gives a parse error. The parser passes the result into the byte-valued matcher
AST; the fileinto/redirect preserve the bytes until the UTF-8 check on the runtime boundary.
The vacation/set arguments and several other AST fields still require UTF-8,
so the extension is recognized for the scripts that explicitly require it, but is not
published in the ManageSieve capability list. This is not a full extension profile.

### Historical record T421-T429

The historical record T421-T429 below describes the state at the dates of those stages;
its conclusions about the complete absence of the `set`/`string` are superseded by the current status above.
Before enabling the capability the following was required:
the one-pass interpretation of a variable-ref (an unknown variable
becomes an empty string, an incorrect reference remains as is), the case-
insensitive names, the extension only after the handling of the quoting/encoded-character,
the runtime expansion at the moment the command is reached, the `set` only with a constant
valid name and a ban on changing the match variables, a `string` test without
an implicit trim, and all the six modifiers with the RFC precedence and a ban on the two
modifiers of the same precedence. The runtime must support at least 128 distinct
variables, 32 ASCII characters of the name and 4000 characters of the value, as well as the `${0}`
and the decimal match-indexes; the leading zeroes are allowed, a missing index
is expanded into an empty string.
The behaviour of the Unicode character length and the bounded
memory expansion have to be fixed separately. As long as these requirements are not verified together, the `variables`
must remain non-advertised.

### Evolution of VariableStore and matcher backtracking

T425 added an independent `VariableStore`: the ASCII case-insensitive names,
up to 128 variables, the names up to 32 characters, the stored values are truncated to
4000 Unicode scalar values. The `expand` performs a single pass, expands the numeric
captures and the named references, leaves the incorrect references and returns an empty
string for the unknown valid names. The numeric references accept any decimal
representation of the index, including the leading zeroes; an unavailable or
an overflowing machine-type index is expanded into an empty string. A separate
local limit of the result -
65 536 characters; its excess returns a typed error. This is a tested
library core, but not a part of the AST/executor pipeline. In particular, the lexer does not yet
pass the provenance of the escaping/encoded-character, the parser does not parse and
validate the variable references, and the engine is not connected to the typed errors of the parser.
T426 added the typed errors into the
engine for a valid namespace reference without an enabled namespace extension;
the parser-time diagnostics are still missing. The previous T426/T427 wrongly
interpreted the numeric references as a single digit or a syntax error; the RFC 5229
allows the leading zeroes and expands an unavailable index into an empty string.
The discrepancy was corrected in T428. The `set` and the `string` are absent, so the
capability remains hidden.

T428 also replaced the greedy glob capture matching with the backtracking, in order to
comply with the non-greedy distribution of the wildcard captures and to reset the values of
the failed branches. The search is limited to one million steps. T429 corrected the path of
the budget exhaustion: the matcher returns a typed work-limit error, the evaluator
passes it as the `ExecutionError::MatcherLimit`, and the delivery planner chooses the
implicit keep and writes a diagnostic instead of a false `NoMatch`. A Store integration
test verifies that a partial `discard` is not applied, the message stays in the
INBOX, and the typed diagnostic is read after reopening the Store.

## Base RFC 5228: address lists

The Sieve `address` uses the common RFC 5322 mailbox parser. Before T424 the projection
was limited to the first 64 mailboxes and silently ignored the rest of the
valid header, which contradicted the rule of "any address/key match". The limit
was removed; the cost is limited by the maximum size of an accepted message
(25 MiB). A regression test verifies the preservation of the 65th address. This eliminates
only the hidden truncation and is not a claim of the full compatibility of the
address-test with the RFC 5228.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
