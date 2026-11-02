\---

name: reverse-engineer-anything

description: Reverse engineer native, managed, Electron/JavaScript, packaged, firmware, and browser targets with REA, and build agent environments (AgentEnv bundles) of web applications from a live instance or a capture. Use shipped-artifact or requested runtime evidence to explain features, compare versions, decompile code, or guide a reconstruction. Skip REA for ordinary source-repository architecture analysis.

metadata:

&#x20; version: "59"

\---



\# REA



Use REA when a claim depends on a shipped binary or package, decompilation,

requested runtime evidence, or comparison with

behavior not established by available source. For ordinary analysis of a

complete source repository, use normal repository tools and do not run REA

readiness or provider commands.



\## Connect only when needed



Installing this skill supplies instructions; it does not register the REA MCP

server, install analysis engines, or add tools to an already-running agent.

The exception is a Claude Code or Codex plugin install: the plugin bundles

this skill and registers its own `rea` server, outside the state that

`rea setup` and `rea doctor` manage.



If REA tools are available and their registration is not known to be stale,

proceed directly to the target. Do not run diagnostics or setup before every

investigation. Use the connected server's actual tool list and input schemas;

a skill installed from repository main may describe capabilities absent from

an older npm release. Keep complete inline Evidence when that server does not

advertise retained references.



If tools are absent, a call is blocked by the client, or a required provider is

unavailable, read \[connection and recovery](references/connection-and-recovery.md).

Diagnose the observed blocker, continue useful supported work, and resume the

original task after repair. Client approval, shell PATH, registration and engine

availability have different remedies; avoid repeating setup for all of them.

If the host times out without a REA result, do not repeat the unchanged call or

assume work stopped. Read the recovery guide's deadline distinction before retrying.

If the MCP connection drops, retry that call once only when it is safe to

repeat or known not to have started. If it may already have changed the

target, or it fails again, tell the user and ask them to restart the MCP

client. If REA could not start its connection, retry once, then tell the user

and ask them to restart the MCP client. Do not retry a startup failure that

returns the same unchanged result. For plugin installs, follow

\[plugin recovery](references/connection-and-recovery.md#rea-installed-as-a-client-plugin).

For setup-owned registrations, run `rea doctor` when REA could not start

or tools stay missing after that restart. For `cleanup\_incomplete`, a retryable

capacity limit, or `configuration\_invalid`, follow the recovery guide's ladder

before asking for a restart. Read that guide before a second repair attempt.



Choose the installation owner before repairing registration. Plugin installs use

\[plugin recovery](references/connection-and-recovery.md#rea-installed-as-a-client-plugin);

only setup-owned registrations use the guide's

\[registration repair plan](references/connection-and-recovery.md#repair-registration-when-needed).

A plugin-only doctor result of `missing` does not call for `rea setup`.



REA setup never installs or upgrades Node.js, npm, Homebrew, Java, Ghidra, IDA,

JADX, Kuna, Binwalk, or Unblob. Use existing prerequisites; do not install unrelated

software to repair MCP registration. For an unsupported client, use manual stdio

registration with an existing installation’s absolute runtime and entry script, or continue through the CLI.



A concrete CLI fallback for an operator-supplied JavaScript tree or ASAR is:



```bash

npx -y rea-agents@latest analyze-javascript-application /absolute/path/to/app --json

```



No MCP registration or native engine is required. Read the returned Evidence,

graph, limitations, and unknowns with the same care as an MCP result; the CLI

returns the Evidence record directly. This fallback does not establish that

MCP is configured. Native CLI tasks still require their selected engine.

Prefer an existing `rea` CLI. When none is installed and REA package acquisition

is within the authorized scope, use `npx -y rea-agents@latest` in its place from a

trusted directory outside the target project, with absolute target paths. Managed

setup binds the current installation directly; retain its runtime and package

cache entry, or restore that installation before repairing its registration.

For setup-owned registration, configuration repair starts with a read-only setup plan; show paths, backups and

skill replacement before applying authorized changes. Preserve existing approval

and target-execution boundaries.



\## Route the target first



Choose the first tool from the target the user supplied. Use `open\_binary` for

active-target native or archive workflows; target-free tools take their own

explicit path or endpoint and do not need it.



\- ASAR or extracted JavaScript/Electron tree:

&#x20; `analyze\_javascript\_application`.

\- Archive/package member inventory (ZIP/APK/IPA/MSIX/AppX, DMG, or a PyInstaller

&#x20; executable): `open\_binary` with the supplied local path; use `inspect\_artifact`

&#x20; when its graph and findings help answer the question. A PyInstaller CArchive

&#x20; is the application layer; the bootloader is only the native prefix. Start with

&#x20; `detail: "summary"` for large inventories, then use retained inventory views

&#x20; for selected member fields/pages. A complete small inventory can cost less

&#x20; than extra calls and schemas. Runtime options are metadata, not files.

&#x20; Compact delivery still inventories and hashes members. See the

&#x20; \[Python packaged applications guide](https://github.com/morluto/rea/blob/main/docs/python-packaged-applications.md).

\- Bun `--compile` executable: `open\_binary`, then `inspect\_artifact`; the Bun

&#x20; module graph is the application layer. Extract stored modules and analyze the

&#x20; JavaScript. See \[Bun standalone executables](https://github.com/morluto/rea/blob/main/docs/bun-standalone.md).

\- Apple app plist, Interface Builder, asset catalog, or keyed archive resources:

&#x20; `open\_bundle\_resources`, then the selected resource tool. This does not

&#x20; require `CFBundleExecutable` or a deep provider. Resource sessions have no

&#x20; executable digest or binary snapshot; use `open\_binary` for native code.

\- Android APK code, classes, methods, or incoming references:

&#x20; `inspect\_android\_package`, then focused Android tools when advertised.

&#x20; Archive member inventory still uses the archive route above.

\- Managed PE/CLI assembly: `inspect\_managed\_artifact`.

\- Firmware image: `inspect\_firmware\_regions` when advertised. Use

&#x20; `extract\_firmware` when extraction is requested, with a caller-selected new

&#x20; absolute output directory. These tools use caller-supplied Binwalk/Unblob on

&#x20; Linux; see the \[firmware guide](https://github.com/morluto/rea/blob/main/docs/firmware-analysis.md).

\- ESP32-family app or bootloader image (`0xE9` magic): `inspect\_artifact`

&#x20; without a provider; see the ESP section of `references/native-and-artifacts.md`.

\- .NET NativeAOT PE/ELF: native Ghidra analysis. Native code can yield pseudocode;

&#x20; consult the NativeAOT workflow in `references/native-and-artifacts.md` for

&#x20; optional metadata recovery and its supported host/layout boundary.

\- Native program behavior, live calls, instrumentation, or command-run effects:

&#x20; choose the matching \[native runtime workflow](references/runtime-observation.md#native-runtime-questions).

&#x20; `open\_binary` and `trace\_native\_values` establish static facts; they do not

&#x20; observe execution. Runtime tools have distinct launch, attach, mutation and

&#x20; cleanup effects, and do not require a preliminary static session unless their

&#x20; own contract says so.

\- User-owned browser page already open: `list\_browser\_targets`.

\- Browser action the agent must perform (click, type, navigate):

&#x20; `capture\_browser\_scenario` with declared actions. See

&#x20; \[runtime observation](references/runtime-observation.md) for connect/launch

&#x20; and the focused follow-up browser tools.

\- Agent environment (AgentEnv bundle) of a web application: read

&#x20; \[environments](references/environments.md) before recording; it records with

&#x20; `capture\_browser\_scenario` and writes the bundle with `export\_environment\_bundle`.

\- Fiddler Everywhere traffic or a native `.saz` capture: read the

&#x20; \[Fiddler workflow](references/runtime-observation.md#fiddler-everywhere-traffic-debugging).

&#x20; Inspect saved SAZ with `inspect\_web\_network\_capture` and `format: "saz"`;

&#x20; connect Everywhere's separate MCP server for live traffic.

\- Retained HAR/SAZ/native mitmproxy/aioquic qlog capture: `inspect\_web\_network\_capture`. Select

&#x20; original record ordinals when useful; inspect producer fields and byte/number

&#x20; sidecars without fetching recorded URLs or claiming live attribution. HAR/SAZ/qlog

&#x20; old-generation heap is `REA\_CAPTURE\_DECODER\_OLD\_GENERATION\_MIB` (default 192 MiB,

&#x20; floor 8, maximum 16384), set where REA starts. Host memory and `NODE\_OPTIONS` do not

&#x20; raise it. See

&#x20; \[historical captures](https://github.com/morluto/rea/blob/main/docs/web-network-captures.md) for the exact upstream

&#x20; profile and credential exclusions. Qlog support starts with aioquic 1.5.0

&#x20; JSON 0.3; stream coverage and endpoint loss declarations do not recover bodies

&#x20; or establish network loss. Prefer detail summary, then kind qlog trace/stream

&#x20; pages; select one trace, stream ID and direction for contribution/range pages.

&#x20; Facts retain all-recorded-events-before-selection scope. Other qlog layouts

&#x20; and PCAP correlation are unsupported.

\- User-owned Electron runtime already open: `list\_electron\_targets`. Before an electron-main Inspector attempt, check packaged fuses with `inspect\_electron\_fuses`; disabled `EnableNodeCliInspectArguments` means the app ignores `--inspect`.

\- Packet-field discovery: `inspect\_packet\_fields` searches installed protocols

&#x20; and field names/types before choosing fields for `inspect\_packet\_capture`.

&#x20; Query/page the relevant records; registrations do not establish observed traffic.

\- Recorded packet captures: `inspect\_packet\_capture` with selected TShark fields,

&#x20; display filters and decode-as rules. Requires Linux, util-linux prlimit and

&#x20; TShark 4.x on an absolute `PATH` entry or via the optional absolute

&#x20; `REA\_TSHARK\_COMMAND` override; see

&#x20; \[capture guide](https://github.com/morluto/rea/blob/main/docs/packet-captures.md).

&#x20; Packet limits count input packets before filtering. Output defaults to 50 matching

&#x20; frames; use `match\_offset` / `match\_limit` for later pages, or explicit null

&#x20; limit for complete matching output. Preserve the source digest across pages.

&#x20; This does not perform live capture.

&#x20; For address/DNS/SNI/protocol summaries use `inspect\_pcap\_capture` with MCP

&#x20; detail summary, then retained kind pcap pages. Requires Wireshark 4.x tshark

&#x20; and capinfos, with Linux prlimit or Windows Job Objects; see

&#x20; \[packet summaries](https://github.com/morluto/rea/blob/main/docs/pcap-analysis.md).

&#x20; Select an exact DNS question for address pages; page exhaustion does not prove

&#x20; network completeness. This reads recordings and does not acquire traffic.

\- Decoder discovery: `inspect\_signal\_decoders` searches installed decoder IDs

&#x20; or selected decoder help, including channels and options. Query and page the

&#x20; relevant details before choosing settings; this does not scan hardware.

\- Recorded logic/signal traces: `decode\_signal\_recording` with explicit decoder,

&#x20; channel and protocol settings. On Linux with util-linux prlimit, supply sigrok-cli 0.7.2 through

&#x20; `REA\_SIGROK\_COMMAND`; see \[recording guide](https://github.com/morluto/rea/blob/main/docs/signal-recordings.md).

&#x20; Output defaults to 50 literal lines; select annotation classes and page with

&#x20; `line\_offset` / `line\_limit`, or explicit null for all remaining output.

&#x20; Window counts are display lines, not annotations or samples. Preserve source digests across pages.

&#x20; This reads existing files; it does not acquire hardware signals.

\- Explicit local WebAssembly artifact: `inspect\_wasm\_artifact`. Supply WABT 1.0.42

&#x20; through `REA\_WABT\_BIN\_DIRECTORY`; retain exact byte/tool identity and decoded WAT.

&#x20; Select JavaScript glue and candidate paths explicitly and preserve URL/path

&#x20; ambiguity. No implicit fetch or target execution; WAT is not original source.

&#x20; See the \[WASM guide](https://github.com/morluto/rea/blob/main/docs/wasm-artifacts.md).

\- Explicit local Android APK (or ZIP/AAB) signing: `inspect\_apk\_signature`.

&#x20; Prefer its summary or selected signer/digest/lineage page; retain complete

&#x20; Evidence and use `inspect\_analysis\_view` kind `apk-signature` for follow-ups.

&#x20; Reports which schemes are present (v1 JAR, v2, v3, v3.1), each signer's

&#x20; certificate SHA-256 fingerprint (v1 included, from its PKCS#7 block), v3 key-rotation lineage, source stamp, and

&#x20; whether each signed digest still matches the bytes (self-consistency). This

&#x20; is identity and integrity, not a trust or authenticity verdict.

&#x20; See the \[APK signature guide](https://github.com/morluto/rea/blob/main/docs/apk-signature.md).

\- Explicit local EVM bytecode carrier: `inspect\_evm\_interface` with caller-selected

&#x20; `raw` or `hex` encoding. The owned worker deadline is `timeout\_ms` (default

&#x20; 30000 ms, range 1..2147483647, next call). It does not raise the fixed byte,

&#x20; address-space, or CPU allowances. Preserve carrier/decoded digests and treat selectors,

&#x20; argument strings and mutability as inferred candidates; this performs no chain

&#x20; lookup or target execution. For the complete inferred interface without hex or

&#x20; duplicate stdout, discover support and select `detail: "summary"`; complete

&#x20; Evidence stays retained under its original identity. Use the returned

&#x20; `parent\_evidence\_id` for exact function or `bytecode\_hex`/stdout/stderr pages.

&#x20; Candidate coverage does not establish interface completeness. See the

&#x20; \[EVM bytecode guide](https://github.com/morluto/rea/blob/main/docs/evm-bytecode.md).

\- Embedded Go compiler, module or build-setting metadata: `inspect\_go\_binary`

&#x20; with the explicit local file. This built-in target-free inspection requires no

&#x20; Go installation or disassembler. Preserve raw module bytes and source ranges;

&#x20; absent metadata does not prove a binary is not Go, and unparsed text records

&#x20; remain explicit. See the

&#x20; \[Go binary guide](https://github.com/morluto/rea/blob/main/docs/go-binaries.md).

\- Embedded CUDA/HIP code objects: `inspect\_gpu\_code\_objects`; inventory targets

&#x20; first, then decode selected payloads and query retained GPU Evidence.

\- Bundled GGUF or safetensors model declarations: `inspect\_model\_artifact`

&#x20; with the explicit local file. Default header hashing leaves full-file identity

&#x20; unknown; `hash: "full"` additionally reads all weights for SHA-256. MCP returns

&#x20; retained summaries; optional `view` selects one page in the initial call. Use

&#x20; `inspect\_analysis\_view` kind `model-artifact` for

&#x20; batched metadata keys, tensor names or type totals, selected fields and pages.

&#x20; Preserve header-scoped indices and array preview omissions; do not infer model

&#x20; behavior from declared architecture or tensor types. See the

&#x20; \[model file guide](https://github.com/morluto/rea/blob/main/docs/model-artifacts.md).

\- Unity IL2CPP names, member owners and string literals from `global-metadata.dat`:

&#x20; `inspect\_il2cpp\_metadata` with the explicit local file; MCP defaults to compact

&#x20; counts. Query sections, images, types, methods, fields, properties, events,

&#x20; parameters or string\_literals through `inspect\_analysis\_view`

&#x20; with the returned `parent\_evidence\_id`, using exact filters, batch indices and

&#x20; selected fields. Parameters accept `method\_indices`, avoiding a scan of all

&#x20; parameter pages. Missing collections in historical Evidence remain unknown.

&#x20; Versions 24, 27, 29, 31 and 39; no Unity, IL2CPP

&#x20; binary or disassembler is run. Unrecognized magic (possibly encrypted) and

&#x20; other versions are unsupported. Optional matching `binary\_path` enables static native addresses and type references; query `method\_addresses` with indices or addresses/states, preserving every shared-body method and both artifact identities. Without a linked binary native counts remain unknown. Byte-pattern fallback is inferred; no runtime behavior or same-build pairing is proved. See the

&#x20; \[IL2CPP metadata guide](https://github.com/morluto/rea/blob/main/docs/il2cpp-metadata.md).

\- Embedded cargo-auditable Rust dependency metadata: `inspect\_rust\_binary`

&#x20; with the explicit local ELF, PE, or thin Mach-O file. This built-in

&#x20; target-free inspection requires no Rust installation or disassembler. Preserve

&#x20; package order, dependency indices, and the section file range. A missing

&#x20; `.dep-v0` section is null metadata, not proof the binary has no dependencies.

&#x20; A rustc commit hash is an inference from a read-only `/rustc/<40-hex>/` path.

&#x20; For selected crates, use `view: {collection: "packages", names: \[...],

&#x20; fields: \["name", "version", "source"]}` on the first call; explicitly include

&#x20; `dependencies` for direct index references. Use `detail: "summary"` for counts

&#x20; and ranges, or retained `inspect\_analysis\_view` Rust package/edge pages for

&#x20; subsequent filters. Keep the complete parent Evidence accessible; CLI compact

&#x20; delivery requires `--evidence\_output`.

&#x20; See the

&#x20; \[Rust binary guide](https://github.com/morluto/rea/blob/main/docs/rust-binaries.md).

\- PE Authenticode certificate table: `inspect\_pe\_certificates` with the explicit

&#x20; local PE32/PE32+ file. This built-in target-free inspection requires no

&#x20; Windows SDK. Preserve WIN\_CERTIFICATE ranges, types, payload SHA-256, and

&#x20; PKCS#7 signer/X.509 identity fields, including `subject\_key\_identifier\_hex`

&#x20; when the signer uses that form. A missing security directory is observed

&#x20; absence of that table, not proof the image is unsigned in every catalog sense.

&#x20; Signatures are not verified. See the

&#x20; \[PE certificate guide](https://github.com/morluto/rea/blob/main/docs/pe-certificates.md).

\- PE import and delay-load tables: `inspect\_pe\_imports` with the explicit

&#x20; local PE32/PE32+ file. This built-in target-free inspection requires no

&#x20; Windows SDK. Preserve DLL names, named and ordinal functions, ILT/IAT RVAs,

&#x20; delay-load descriptors, and the pefile import hash of the standard import

&#x20; table. A missing import directory is observed absence of that table, not

&#x20; proof the image never imported APIs through another table. Bound snapshots

&#x20; and loaded IAT contents remain unobserved. See the

&#x20; \[PE import guide](https://github.com/morluto/rea/blob/main/docs/pe-imports.md).

\- PE TLS callbacks and load configuration: `inspect\_pe\_load\_config` with the

&#x20; explicit local PE32/PE32+ file. This built-in target-free inspection requires

&#x20; no Windows SDK. Preserve callback VAs as strings with their RVA, file offset

&#x20; and section, and the size-covered security cookie, SafeSEH, CFG,

&#x20; EH-continuation and XFG fields. Guard-flag mitigation claims are inferred

&#x20; linker declarations, not proof of enforcement. A missing directory is observed

&#x20; absence of that declaration. See the

&#x20; \[PE load configuration guide](https://github.com/morluto/rea/blob/main/docs/pe-load-config.md).

\- ELF note tables: `inspect\_elf\_notes` with the explicit local ELF32/ELF64

&#x20; file. This built-in target-free inspection requires no disassembler. Preserve

&#x20; PT\_NOTE and SHT\_NOTE ranges, owners, types, descriptor SHA-256, and GNU

&#x20; build-id or ABI-tag fields. A missing table is observed absence of those

&#x20; declarations, not proof the image never carried notes. See the

&#x20; \[ELF notes guide](https://github.com/morluto/rea/blob/main/docs/elf-notes.md).

\- Apple `.ips` crash reports: `inspect\_apple\_ips` with the explicit local

&#x20; file. This built-in target-free inspection requires no Xcode, atos, or

&#x20; pwntools. Preserve first-line metadata, `bug\_type` as a string, process and

&#x20; exception facts, lossless integer registers, usedImages UUIDs, and

&#x20; documented classification matches. Only `bug\_type` `"309"` yields a crash

&#x20; body; other types, including `"288"` stackshots, return no crash result.

&#x20; Opened Mach-O and dSYM UUID joins remain unobserved. See the

&#x20; \[Apple IPS guide](https://github.com/morluto/rea/blob/main/docs/apple-ips.md).

\- Linux kernel module metadata: `inspect\_kernel\_module` with the explicit local

&#x20; ELF32/ELF64 relocatable file. This built-in target-free inspection requires

&#x20; no Linux host or `modinfo`. Preserve `.modinfo` key order and duplicate keys,

&#x20; `\_\_versions` CRC hex, section file ranges, and the appended signature

&#x20; trailer's PKCS#7 SHA-256. A missing section is observed absence when section

&#x20; names are known, not proof the module never carried that metadata.

&#x20; `struct module` is not decoded. The signature is not validated. gzip, xz, and

&#x20; zstd containers are unsupported; decompress into a separate file with an

&#x20; external codec utility, then inspect the decompressed `ET\_REL` image. See the

&#x20; \[kernel module guide](https://github.com/morluto/rea/blob/main/docs/kernel-modules.md).

\- CPython 3.7–3.13 `.pyc` header, nested marshal code objects and bytecode

&#x20; identity: `inspect\_python\_pyc` with the explicit local file. This built-in

&#x20; target-free inspection requires no Python installation. Preserve magic,

&#x20; PEP 552 hash or mtime/size, nested names, and bytecode SHA-256. Bytecode is

&#x20; not decompiled, and a decoded header does not prove the matching `.py` is

&#x20; present. See the

&#x20; \[CPython pyc guide](https://github.com/morluto/rea/blob/main/docs/python-pyc.md).

\- Erlang BEAM IFF container: `inspect\_erlang\_beam` with the explicit local

&#x20; `.beam` file. This built-in target-free inspection requires no Erlang

&#x20; installation. Preserve chunk names, atom order, export/import/lambda tables,

&#x20; and the Code header. For selected facts, prefer `detail: "summary"` plus a

&#x20; `beam` view (collection, names/indices, fields and paging), retaining the full

&#x20; Evidence. CLI summary requires `--evidence\_output`. Missing table counts stay

&#x20; null; decoded empty tables count zero. Bytecode is not executed. See the

&#x20; \[Erlang BEAM guide](https://github.com/morluto/rea/blob/main/docs/erlang-beam.md).



\- Explicit local SQLite database snapshot: `inspect\_sqlite\_database`. Inspect

&#x20; with Node.js 24.x >=24.15 or 26+ providing native SQLite limits. Read schema

&#x20; first (compact inventory by default), or select an exact ordinary table

&#x20; and `columns` when rows are needed. Defaults are selected-table schema and

&#x20; 20 rows; request `schema\_scope: "database"` for the complete stored schema. Preserve both database and existing sibling WAL identity; row samples

&#x20; report truncation and have unspecified order. Use a quiescent snapshot rather

&#x20; than assuming a live database/WAL pair was copied atomically. See the

&#x20; \[SQLite guide](https://github.com/morluto/rea/blob/main/docs/sqlite-databases.md).

\- Explicit Linux ELF file for offline layout, symbols, relocations or static

&#x20; mitigation evidence: `inspect\_binary\_layout`. This target-free operation uses

&#x20; caller-supplied pwntools without opening a disassembler database. Preserve its

&#x20; raw locations and inference/coverage limits; see the

&#x20; \[offline binary guide](https://github.com/morluto/rea/blob/main/docs/binary-diagnostics.md).

\- Supplied Linux x86-64 ELF core: `inspect\_recorded\_crash`. Read every recorded

&#x20; thread, raw note and source-bound register without opening a live target.

&#x20; Historical PIDs do not select live processes. Request `include\_debugger\_context`

&#x20; only when core-only mapping candidates help; see the

&#x20; \[recorded crash guide](https://github.com/morluto/rea/blob/main/docs/recorded-crashes.md).

\- PE resource type/name/language trees and static icon group references:

&#x20; `inspect\_pe\_resources` with an explicit path. This portable operation requires

&#x20; no active target or engine; opaque payloads retain file ranges and SHA-256.

\- PE export directories and ELF dynamic-symbol exports: `inspect\_native\_exports`

&#x20; with an explicit path; portable, no active target or engine.

\- Native executable, library, or analysis database: `open\_binary`, then

&#x20; use focused analysis tools directly; call `binary\_overview` when metadata or

&#x20; inventory context is useful and available from the selected provider.



For an existing IDA MCP configuration, use `open\_binary` with the original

input binary and `provider\_id: "ida"`; do not pass an `.idb` or `.i64` database.

The legacy attached profile binds the already-open GUI input by SHA-256 and

leaves its database open. The modern headless profile creates and closes an

owned private database without saving. Installation and registration reuse

\[mrexodia/ida-pro-mcp](https://github.com/mrexodia/ida-pro-mcp); see the

\[IDA provider guide](https://github.com/morluto/rea/blob/main/docs/ida-provider.md).

For migration to the separate native IDA MCP plugin, follow the guide's

\[legacy attached migration](https://github.com/morluto/rea/blob/main/docs/ida-provider.md#migrate-a-legacy-attached-registration).

Replace the upstream registration explicitly; modern mrexodia tools are not the

native profile. Native attachment preserves the database but does not provide

direct callers; check session availability and the guide's verification limits.

Proceed directly to `analyze\_function`, pseudocode, or function/string searches

after opening; IDA does not supply `binary\_overview`. Consult session availability

when composing broader workflows. Modern direct callers and unsupported dossier

facets remain unknown; live IDA results are not replayed from snapshots.



With `REA\_KUNA\_COMMAND` configured, `provider\_id: "kuna"` answers procedure

lists, lookups, pseudocode, assembly and `analyze\_function` from Kuna's cached

whole-program export. Callers/callees use a separate body-free graph shared by

the session; `xrefs` returns exact-address incoming sites and filters forwarding

aliases. These queries and inventory lookups do not trigger a C export. Indirect

targets remain unknown and can make relationship coverage partial. Reference

caches expire on close; a fresh CLI process repeats the native reference walk.

Typed references, CFG, raw-memory tools and annotations remain unsupported. A cold first

answer can come from a single-function run (its limitations say so); assembly

waits for the export. For pseudocode and assembly, `raw\_result.source` names the

cached `.c`/`.asm` file and byte span, which you may search directly instead of

calling per function. See the

\[Kuna provider guide](https://github.com/morluto/rea/blob/main/docs/kuna-provider.md).



If the user says only "my app" without a name, path, or target already selected

for this task, ask which app before searching for artifacts. Paths in unrelated

saved reports and nearby test fixtures do not select the user's target. Resolve

an actual human-readable app name to one clear installed artifact when possible;

ask when matches are ambiguous. Never choose an example app on the user's behalf.



In a target-free session, use `open\_binary` to bind any archive/package or

native target whose analysis tool operates on the active target. Do not call an

unadvertised tool; inspect `binary\_session` with `{}` and its

`result.tool\_availability` for availability reasons and remediation. Expand each

group's `names` into entries, inheriting the inventory-level client requirements

unless the group overrides them. The grouped name union is the complete tool

inventory; use `tools/list` for canonical tool schemas. The default summary keeps

each provider candidate's availability and target-support status, code and

reason; `omitted\_fields` lists what it leaves out. Request `{"detail": "complete"}`

only when you need provider diagnostics, candidate capability sets, or tool

surface, effect and annotation flags; in those closed flag vocabularies, listed

flags are true and omitted known flags are false. Provider candidate facts

describe possible providers, while top-level session capabilities describe the

current route and `analysis\_provider\_binding` names the selected provider. Do

not treat those scopes as interchangeable.



For a Ghidra startup timeout on a large native target, follow

\[deadline recovery](references/native-and-artifacts.md#ghidra-startup-deadline-recovery)

before abandoning deep analysis or repeating the same cold import. The default

330 seconds can be changed for new startup attempts through

`update\_analysis\_settings` using `ghidra.startup\_timeout\_ms`. Changing the

server's launch environment instead requires a new REA process.



\## Work summary-first



Start with the default result and use its inline Evidence and graph context.

For a JavaScript application or ELF layout whose size is unknown or large,

call `analyze\_javascript\_application` or `inspect\_binary\_layout` with

`"detail": "summary"`, then inspect the retained `parent\_evidence\_id` with

`inspect\_analysis\_view`. Over MCP, `analyze\_function` is summary-first without

a flag: its default reply is the summary, and the complete dossier stays

retained under `parent\_evidence\_id` for facet paging. Pass that ID as a

`retained-evidence` reference to `compare\_functions`, or inside

`match\_native\_functions` `left\_observations`/`right\_observations` or

`build\_call\_path.functions`; request `"detail": "complete"` only when the

dossier must travel inline.

Use the same exact-reference form in other advertised logical Evidence inputs,

including optional sources, arrays and nested sources across domains. Each

source can independently be inline or retained; preserve its source identity,

coverage, partial facts and unknowns. A summary or inspection page has its own

Evidence ID: use `normalized\_result.parent\_evidence\_id` when the consumer needs

the complete source. For portable CLI follow-ups, commands advertising

`--evidence-bundle` resolve references from that selected exported bundle.

See \[Evidence composition](references/evidence-workflows.md) for scope and

recovery; file selectors are supported only where explicitly advertised.

Page module identities and select the returned `node\_id` before requesting an

item; a single file path can represent several graph roles. Do not repeat an

identical analysis or rejected call without a changed input or resolved blocker.

Recheck session state when a lifecycle change makes that status useful.

Make a focused follow-up only when the

returned result leaves a specific question unanswered.



If MCP reports `resource\_constraint` with `details.resource: "transport"`, call

`inspect\_analysis\_view` with its reported same-session Evidence reference for a

summary, one section/module, or a stable page. Use `trace\_application\_feature`

once a seed is known, or call `export\_evidence\_bundle` to write the complete

session to a selected path. The analysis remains complete in the ledger; avoid

repeating it merely to request the same oversized response. Complete CLI JSON

output also streams without a single MCP frame.



When the retained parent ID is unknown, use `get\_evidence\_bundle` with

`detail: "summary"` and exact filters for operation, target SHA-256,

analysis-profile digest or canonical procedure address. Select its returned

Evidence ID for the next view instead of requesting the complete bundle.

`retention: "complete-record"` means the whole record is stored; native

value-flow truncation and unknown metadata remain separate. For an exported

portable bundle, use `rea inspect-evidence-bundle` with JSON containing `path`,

`detail: "summary"` and the same `filters`. Reading that file still requires

validating its full contents within the local memory budget.



A client can also truncate a successful tool result without a REA transport

error. Treat that preview as incomplete; use the retained parent ID for smaller

views or export when the full record is needed. Do not parse the preview as

canonical JSON or repeat the producer. For JavaScript, read the

\[module-view and trace recipes](references/javascript-applications.md) before

drilling into a large graph; a broad bidirectional trace can expand unrelated

containment relationships even after summary analysis.



Every conclusion must distinguish observations, inferences, and unknowns. Cite

Evidence IDs, preserve limitations and incomplete coverage, and never imply

that static analysis observed execution. Runtime requests execute the declared

target and lifecycle; do not broaden the target or action beyond those fields.



Within the user's requested investigation, call available analysis tools

directly. REA does not require permission grants or per-call approval flags.

The agent client can independently require approval. Follow the declared request

scope and the host's actual access requirements; use the recovery guide for an

observed client-policy rejection.



\## Plan broader investigations



For requests that span multiple features or subsystems, use a staged workflow:



1\. Turn the request into a checklist of questions and the evidence each answer

&#x20;  needs. Resolve target identity and constraints from the conversation and

&#x20;  workspace before asking for information again.

2\. Inspect the current REA session, artifact identity, saved analysis database,

&#x20;  bookmarks, and prior evidence. Reuse matching state; do not open duplicate

&#x20;  sessions or repeat identical analysis.

3\. Start with the smallest useful overview or inventory. Follow each question

&#x20;  from its entry point through relevant data and state changes to its result.

&#x20;  Batch related operations around a specific hypothesis, then expand only when

&#x20;  the returned evidence leaves a concrete gap.

4\. Inspect relevant packaged resources and configuration alongside code when

&#x20;  they affect the question. Use format-aware inventory and parsers; do not

&#x20;  infer behavior from filenames, strings, or layout alone.

5\. Corroborate a conclusion with the evidence type it requires. Use runtime

&#x20;  observation only when static evidence cannot answer the question and the

&#x20;  required host runtime and OS access are available. For JavaScript behavior,

&#x20;  run probes against the actual app through the available browser, Electron,

&#x20;  or process capture workflow.

6\. Decompose work into independent questions. When parallel workers are

&#x20;  available and the questions do not depend on one another, assign distinct

&#x20;  scopes, point workers to existing evidence, and ask them to return sources,

&#x20;  conclusions, and unresolved gaps. Otherwise, work sequentially.

7\. Keep a concise finding ledger linking each conclusion to Evidence IDs,

&#x20;  confidence/evidence type, search boundary, and remaining unknowns. Update

&#x20;  the shared index or investigation report so later passes can reuse results.



Before finishing, revisit the original checklist. Mark each question as

answered, partially answered, or unresolved based on its evidence; keep

bounded negative searches bounded, and do not describe a broad investigation

as complete while required questions remain open.



\## Read only the relevant guide



\- Native binaries, managed assemblies, archives, and extraction:

&#x20; \[references/native-and-artifacts.md](references/native-and-artifacts.md)

\- ASARs, extracted JavaScript, feature tracing, and version comparison:

&#x20; \[references/javascript-applications.md](references/javascript-applications.md)

\- Android APK declarations, classes, methods, and static references:

&#x20; \[references/android-applications.md](references/android-applications.md)

\- Native instrumentation, debugger and process capture; browser/Electron

&#x20; observation; static/runtime comparison:

&#x20; \[references/runtime-observation.md](references/runtime-observation.md)

\- Evidence paging, comparisons, residual unknowns, and verification:

&#x20; \[references/evidence-workflows.md](references/evidence-workflows.md)

\- Agent environments (AgentEnv bundles) of a web application, recorded from

&#x20; a live instance or a HAR:

&#x20; \[references/environments.md](references/environments.md)



\## Finish the task



Explain findings in plain language and tie them to returned evidence. When the

user asks to build something, use normal coding tools and separate observed

behavior from design choices. Close an opened native session with

`close\_binary` when the investigation is complete.

