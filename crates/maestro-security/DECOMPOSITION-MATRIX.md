# DECOMPOSITION MATRIX — 13 frameworks → maestro-security

Evidence artifact for the unified security pipeline. Every behavioral
subsystem of every framework maps to exactly one maestro-security owner.

## Method (REA)

- Route: source-level targets (all 13 have full source; no binaries to decompile).
- Functions + classes: `graphify extract --code-only` (local AST, tree-sitter, no LLM).
- Behavioral subsystems: `graphify cluster-only` (community detection over call/reference edges).
- Architectural hubs: `graphify god-nodes` (highest-degree nodes per framework).
- Ownership sweep: every community >= 20 nodes classified behavioral vs non-behavioral
  (tests/UI/data/minified/build), then each behavioral core assigned to one of the 25
  maestro-security modules by distinctive-keyword scoring over its top-8 members.

## Corpus

- 13 frameworks, 75,764 functions/classes, 244,981 call/reference edges, 2,562 communities.

## Per-framework decomposition

### AIDA

2,149 nodes / 4,996 edges / 106 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | models/__init__.py[66]; assessments.py[51]; sqlalchemy_orm[45] | 80 | `store` |
| 2 | json[32]; approve_command()[13]; Session[13] | 68 | `store` |
| 3 | mcp_handlers.py[35]; handle_tool_call()[25]; TextContent[22] | 67 | `runner` |
| 4 | events.py[32]; create_event()[29]; credentials.py[28] | 65 | `store` |
| 5 | NotificationService[33]; notifications.py[24]; notification.py[11] | 64 | `report` |
| 6 | api/system.py[44]; pydantic[22]; get_tool_usage_stats()[12] | 64 | `store` |
| 7 | AssessmentDetail.jsx[59]; CardsTable.jsx[28]; index.jsx[27] | 64 | `webui` |
| 8 | typing[67]; main.py[54]; database.py[53] | 57 | `llm` |
| 9 | AidaMCPService[31]; Any[16]; .execute_container_command()[10] | 57 | `runner` |
| 10 | ExecResult[19]; ExecutionBackend[19]; HostAgentExecutionBackend[17] | 56 | `runner` |

### Dark-Moon

986 nodes / 2,083 edges / 73 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | live_push.py[39]; json_storage.py[31]; Any[21] | 84 | `campaign` |
| 2 | server.py[52]; os[37]; tool[13] | 66 | `pipeline` |
| 3 | growth.py[35]; test_growth.py[29]; record_milestone()[26] | 62 | `campaign` |
| 4 | gateway.py[26]; privacy/__init__.py[21]; GatewayPolicy[14] | 35 | `pipeline` |
| 5 | .tokenize()[26]; vault.py[21]; .register()[16] | 33 | `llm` |
| 6 | app.tsx[33]; tui()[4]; App()[2] | 31 | `webui` |
| 7 | index.tsx[71]; ToolProps[1]; context[1] | 28 | `webui` |
| 8 | index.ts[16]; ui.ts[11]; getBrewFormula()[3] | 28 | `webui` |
| 9 | BaseWorkflow[45]; typing[22]; list_workflows.py[20] | 26 | `pipeline` |
| 10 | DarkmoonDockerClient[28]; .execute_command()[14]; .health_check()[6] | 25 | `runner` |

### Pentest-Swarm-AI

4,421 nodes / 17,409 edges / 221 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | go_pkg_strings[241]; models.go[151]; cleanup_memory.go[138] | 108 | `pipeline` |
| 2 | context.Context[422]; net/http.Client[40]; LoggingBoard[17] | 96 | `pipeline` |
| 3 | go_pkg_fmt[205]; go_pkg_os[79]; chain.go[60] | 95 | `exploit` |
| 4 | exploit_api_test.go[111]; report/agent.go[102]; agents/report.go[88] | 81 | `llm` |
| 5 | Board[66]; ExploitAgent[63]; ProbeSpec[38] | 79 | `pipeline` |
| 6 | github.com/google/uuid.UUID[142]; time.Time[29]; MemoryBoard[28] | 72 | `memory` |
| 7 | swarm_runner.go[301]; go_pkg_time[179]; plugins/executor.go[176] | 69 | `pipeline` |
| 8 | censys.go[18]; github_search.go[15]; github_search_test.go[15] | 66 | `llm` |
| 9 | Provider[40]; ReportAgent[24]; Campaign[20] | 65 | `pipeline` |
| 10 | Playbook[19]; registry.go[17]; plugins/loader.go[17] | 64 | `tools` |

### PentestGPT

1,116 nodes / 3,940 edges / 52 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | ToolRegistry[29]; tools.py[26]; ToolRegistryError[24] | 63 | `tools` |
| 2 | unified_agent/__init__.py[62]; codex.py[38]; agent.py[36] | 52 | `llm` |
| 3 | Task[39]; test_agent.py[30]; AssistantText[27] | 49 | `agents` |
| 4 | test_skills.py[23]; SkillError[22]; skills.py[20] | 35 | `agents` |

### cai

16,758 nodes / 44,553 edges / 680 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | typing[280]; asyncio[135]; _run_impl.py[106] | 384 | `llm` |
| 2 | os[371]; function_tool()[136]; OpenAIChatCompletionsModel[120] | 379 | `llm` |
| 3 | time[136]; sys[116]; re[103] | 320 | `runner` |
| 4 | cli_headless.py[173]; run_cai_cli()[69]; config_utils.py[30] | 260 | `pipeline` |
| 5 | cai/util/__init__.py[173]; util/streaming.py[124]; terminal.py[54] | 230 | `llm` |
| 6 | user_prompts.py[85]; executor.py[64]; tools/common.py[50] | 216 | `runner` |
| 7 | openai_chatcompletions.py[192]; .get_response()[56]; wait_hints.py[47] | 210 | `llm` |
| 8 | Handoff[72]; .stream_response()[66]; AgentOutputSchema[49] | 172 | `llm` |
| 9 | unittest_mock[37]; mcp/server.py[28]; get_mcp_tools_for_agent()[20] | 151 | `runner` |
| 10 | cai_terminal.py[93]; main_view.py[31]; sidebar.py[28] | 136 | `webui` |

### clearwing

12,197 nodes / 33,695 edges / 486 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | json[247]; sourcehunt/runner.py[175]; typing[160] | 187 | `sourcehunt` |
| 2 | core/engine.py[30]; core/__init__.py[26]; CoreEngine[25] | 164 | `pipeline` |
| 3 | proof/__init__.py[150]; proof/engine.py[84]; Evidence[58] | 148 | `sourcehunt` |
| 4 | reveng.py[31]; RevengPipeline[29]; test_reveng.py[28] | 106 | `sourcehunt` |
| 5 | test_sandbox_backends.py[54]; SandboxInstance[40]; JsonRpcConnection[35] | 101 | `runner` |
| 6 | ProofStore[117]; Certificate[42]; proof_flow.py[38] | 99 | `workspace` |
| 7 | hunter.py[83]; test_sourcehunt_hunter.py[46]; .arun()[30] | 92 | `sourcehunt` |
| 8 | tools/__init__.py[121]; get_all_tools()[77]; Any[21] | 91 | `exploit` |
| 9 | openai_oauth.py[76]; test_openai_oauth.py[26]; AnthropicOAuthCredentials[20] | 88 | `llm` |
| 10 | ToolInputModel[45]; hunt/discovery.py[38]; hunt/__init__.py[38] | 87 | `llm` |

### hackingBuddyGPT

1,589 nodes / 3,962 edges / 86 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | configurable.py[63]; parameter()[21]; parse_args()[17] | 73 | `llm` |
| 2 | JsonlLogger[27]; log_storage.py[18]; Span[11] | 57 | `evidence` |
| 3 | Limits[96]; LiteLLM[32]; agents.py[28] | 52 | `llm` |
| 4 | SSHInteractiveConnection[32]; check_command_success()[12]; is_root_from_id()[9] | 45 | `runner` |
| 5 | typing[66]; Capability[58]; capability.py[44] | 44 | `llm` |
| 6 | target_surface.py[25]; load_surface()[12]; TargetSurface[11] | 44 | `surface` |
| 7 | advanced.py[24]; WebTestingAgent[21]; web/_base.py[20] | 43 | `agents` |
| 8 | strategies.py[33]; minimal_linux_privesc_tool_calling.py[28]; usecase.py[28] | 42 | `llm` |
| 9 | capabilities/__init__.py[30]; shell_root_detection.py[18]; SSHTestCredential[14] | 37 | `runner` |
| 10 | ad.py[28]; executor.py[26]; ADPlanner[17] | 36 | `runner` |

### pentagi

22,005 nodes / 91,307 edges / 365 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | .require()[329]; CreateGenerationBody[104]; ObservationBody[62] | 622 | `evidence` |
| 2 | context.Context[4109]; validatePermission()[78]; validatePermissionWithFlowID()[53] | 605 | `workspace` |
| 3 | executionContext[1690]; github.com/99designs/gqlgen/graphql.Marshaler[838]; github.com/vektah/gqlparser/v2/ast.SelectionSet[274] | 557 | `ctx` |
| 4 | types.ts[553]; flow-provider.tsx[73]; apollo.ts[51] | 524 | `llm` |
| 5 | github.com/99designs/gqlgen/graphql.CollectedField[1128]; github.com/99designs/gqlgen/graphql.FieldContext[564]; .fieldContext___Type_interfaces()[23] | 322 | `llm` |
| 6 | .require()[139]; ObservationsView[85]; Observation[57] | 306 | `llm` |
| 7 | go_pkg_context[344]; flow.go[220]; providers/providers.go[217] | 306 | `llm` |
| 8 | Config[150]; ProviderConfig[113]; Provider[81] | 303 | `llm` |
| 9 | ExtractExtraProperties()[197]; .require()[74]; ServiceProviderConfig[37] | 271 | `llm` |
| 10 | IngestionEvent[48]; IngestionEventEight[24]; IngestionEventEleven[24] | 258 | `llm` |

### shannon

3,897 nodes / 10,440 edges / 169 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | capella/temporal/activities.ts[116]; activity-types.ts[74]; runStageActivity()[31] | 88 | `pipeline` |
| 2 | src/temporal/activities.ts[175]; AuditSession[58]; deliverablesDir()[22] | 83 | `evidence` |
| 3 | reconcile-activities.ts[87]; reconcile-activity-types.ts[47]; ArtifactRef[19] | 83 | `workspace` |
| 4 | publish.ts[85]; prepare.ts[55]; readCommittedFile()[37] | 78 | `evidence` |
| 5 | verdicts.ts[72]; stages/shared.ts[55]; export.ts[34] | 70 | `webui` |
| 6 | renumber-core.ts[56]; compaction-core.ts[46]; ActivityLogger[41] | 65 | `report` |
| 7 | 173c9b9a1e7448a5884ab123f2b7e8b7cedca4dbfd4ad96650b2e712263f474c298f27e07ef5c1b0e6468cb80298a3f0aa7443609f750605df9b28b55290ab[18]; InteractiveOAuthClient[14]; InteractiveOAuthClient[14] | 64 | `store` |
| 8 | workflows.ts[94]; temporal/shared.ts[50]; temporal/pipeline.ts[19] | 60 | `pipeline` |
| 9 | run-state.ts[96]; appendPartialReasons()[15]; isPartialReason()[13] | 60 | `webui` |
| 10 | src/index.ts[66]; main()[34]; getMode()[23] | 58 | `pipeline` |

### strix

9,815 nodes / 30,875 edges / 238 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | $_[970]; has()[28]; h2()[16] | 456 | `webui` |
| 2 | r()[219]; .consume()[160]; t()[155] | 238 | `webui` |
| 3 | o()[105]; d()[101]; s()[98] | 238 | `webui` |
| 4 | .push()[131]; .slice()[87]; cj()[73] | 177 | `webui` |
| 5 | typing[189]; pathlib[100]; core/runner.py[92] | 156 | `pipeline` |
| 6 | codex.py[61]; threading[33]; test_codex_auth.py[32] | 124 | `llm` |
| 7 | models.py[110]; load_settings()[108]; config/__init__.py[59] | 117 | `report` |
| 8 | $s()[61]; _O()[49]; node()[38] | 112 | `webui` |
| 9 | Dim()[60]; StringValue()[57]; Col()[48] | 106 | `webui` |
| 10 | view.go[67]; main.go[58]; setup.go[48] | 105 | `webui` |

### cloudflare-check

145 nodes / 357 edges / 16 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|

### rea-check

5 nodes / 3 edges / 2 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|

### reverse-skill

681 nodes / 1,361 edges / 68 communities.

| # | Subsystem (top members) | Size | Owner |
|---|-------------------------|------|-------|
| 1 | McpHttpServer[104]; .dispatch()[87]; JsonObject[84] | 100 | `pipeline` |
| 2 | review_case.py[28]; render_diagram.py[15]; verify-repository-security.py[11] | 75 | `webui` |
| 3 | skills/scripts/bootstrap-reverse.sh[66]; has_cmd()[27]; ensure_capability()[25] | 69 | `llm` |
| 4 | bootstrap-reverse.ps1[42]; Ensure-Capability()[18]; test-bootstrap-supply-chain.ps1[15] | 66 | `runner` |
| 5 | McpHttpServer.java[43]; burp.api.montoya.core.Registration[2]; burp.api.montoya.http.message.HttpRequestResponse[2] | 42 | `llm` |
| 6 | kali/scripts/bootstrap-reverse.sh[20]; ensure_capability()[15]; tool-discovery.sh[13] | 35 | `tools` |
| 7 | ToolDiscovery.ps1[25]; Get-ReverseCapabilityState()[7]; Resolve-ReverseToolSpec()[7] | 28 | `llm` |
| 8 | mcp-bridge.js[22]; handleRequest()[6]; buildToolDefinitions()[4] | 23 | `llm` |

## Full ownership sweep (all communities >= 20 nodes)

- Total communities >= 20 nodes: **1089**
- Non-behavioral (tests/UI/data/minified): **362**
- Behavioral cores: **727**
- Mapped to a module owner: **724**
- Unmapped (install script, telemetry, payment proxy — non-pentest infra): **3**

### Owner distribution

| Owner module | Behavioral cores |
|--------------|------------------|
| `llm` | 214 |
| `webui` | 126 |
| `runner` | 81 |
| `pipeline` | 51 |
| `evidence` | 38 |
| `report` | 34 |
| `store` | 32 |
| `sourcehunt` | 31 |
| `workspace` | 31 |
| `bench` | 16 |
| `agents` | 15 |
| `tools` | 13 |
| `memory` | 11 |
| `exploit` | 8 |
| `models` | 4 |
| `campaign` | 3 |
| `phases` | 3 |
| `UNMAPPED (non-pentest infra)` | 3 |
| `router` | 3 |
| `validate` | 2 |
| `ctx` | 2 |
| `doctor` | 2 |
| `audit` | 1 |
| `extension` | 1 |
| `browser` | 1 |
| `surface` | 1 |

## Gap analysis

**0 behavioral gaps.** Every behavioral core maps to an existing owner. The 3 unmapped
communities are non-pentest infrastructure:

| Framework | Community | Top members | Disposition |
|-----------|-----------|-------------|-------------|
| Pentest-Swarm-AI | install CLI | install.js, cli.js, download() | Box tooling, not crate code |
| strix | telemetry | posthog.py, scarf.py | Product analytics, not ported |
| strix | payment proxy | payment_proxy.py, http_server | Billing infra, not ported |

## Not ported (documented, no feature loss)

| Item | Framework | Reason | Observable behaviour preserved by |
|------|-----------|--------|-----------------------------------|
| GraphQL codegen | pentagi | JS/Go runtime mechanic (OD-111) | Direct JSON tools (`tools.rs`) |
| Postgres | AIDA, pentagi | No new backend (owner) | JSON store (`store.rs`) |
| Temporal | shannon | Durable-workflow infra | Workspace resume + optimistic store (`workspace.rs`, `store.rs`) |
| Minified UI bundle | strix | Minified, no source value | Report page (`webui.rs`) |
| Embedding model calls | pentagi | External model dependency | Deterministic feature-hashing embedder (`memory.rs`) |
| Box bootstrap scripts | reverse-skill | Runner environment, not crate | Runner sandbox (`runner.rs`) |
| Telemetry | strix | Product analytics | Not ported |
| Payment proxy | strix | Billing infra | Not ported |

## Verification

- AST extraction: 13/13 frameworks, 75,764 nodes, 244,979 edges.
- Clustering: 13/13 frameworks, 2,481 communities.
- Ownership sweep: 1,089 communities >= 20 nodes; 362 non-behavioral; 727 behavioral; 724 owned; 3 non-pentest infra.
- God nodes: top 5 per framework identified and mapped.
- Cross-check: every top-10 subsystem per framework has a named owner module.

## Campaign target 1: the reverse-skill (issue #14, byte-per-byte)

Decomposed 2026-10-25 by the REA-only agent (mission
`rea-v2/reverse-skill/`): 93 capability rows, 77 proven, 16 likely, 0
guessed, 10 unknown; 13 REA evidence ids; sha256-identified target.

Shape: a router plus claim discipline, not a tool catalog — 60 of 93 rows
are selection or prohibition rules. Every analysis path collapses to three
primitives: a bounded result, a retained parent Evidence id, and a
page/item view.

Rulings: the primitives home to the evidence ledger (`evidence.rs` lineage,
`report.rs`/`webui.rs` view); the recurring prohibitions become one gate in
`validate.rs`. Graph agreement (ADR-0020): 43 nodes / 8 communities, every
community maps to a row group, 0 unmapped.

## Campaign target 2: the REA method + merged leanrea (issue #14)

Decomposed 2026-10-25 by the REA-only agent (mission `rea-v2/rea-method/`):
82 rows, 80 proven, 2 likely, 0 guessed, 11 unknown. Byte split confirmed:
lines 1–50 and 110–133 are the owner's REA method; lines 52–108 are the
adapted leanrea fast path — a probe router (19 target rows) plus six
economy rules, not a second methodology. Owner homes: prohibitions and
gates → validate.rs, ledger → evidence.rs, views → report.rs, target
selection → router.rs, recipes → tools.rs, the 8 steps →
pipeline.rs/phases.rs. Graph agreement: 11 nodes / 1 community, 0 unmapped.

## Campaign target 3, slice 1: the REA implementation surface (issue #14)

Decomposed 2026-10-26 by the REA-only agent (mission `rea-v2/rea/`): 29
rows, 28 proven, 7 unknowns. Target identity: rea-agents 6.7.0, tarball
sha256 20958a66…f51f90, 1,775 shipped files. Two faces, one registry: 153
CLI commands and 207 MCP tools generated from one declarative table, one
contract (closed-world schemas, four annotation hints, 11 signature
clusters covering 74 tools). Providers are external subprocesses (26
bridge sources + one Windows .node). Graph: 1,482 nodes / 62 communities.
Slice 2 order: the session/evidence family and the registry machinery
first — the foundation every family sits on.

## Campaign target 3, slice 2: the REA foundation (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 40 rows. The session family
is a coordinator (registerSessionTools delegates to lifecycle/evidence/
process registries, every call through one admission gate and one result
projection). One contract row of six fields; registration hard-fails on a
missing effect audit or output schema. The four MCP hints are derived from
the effect set, not authored; closed-world schemas come from strict zod
shapes. Evidence I/O canonicalises then writes atomically, re-parses then
trusts. Unknowns are an append-only hash chain (digest-addressed, CAS
revision tokens, duplicates as success). Agreement: 23 session-family
graph nodes ↔ provider-session seams; dist-internal rows ride byte
evidence. Slice 3 order: the provider-coordinate family (45 tools).

## Campaign target 3, slice 3: the provider-coordinate family (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 43 rows, all proven, 12
unknowns. The family is 44 tools (document/address/procedure coordinates,
overlapping); inspect_macho and demangle_swift are path tools, not
coordinate tools. Addresses are bare strings at the contract — a Rust hex
parser at the boundary would reject inputs the reference accepts;
normalisation happens after the provider's invalid_format, CLI-only.
Dispatch arity is (operation, parameters, options). Ruling: runner.rs owns
the six-decision pre-dispatch gate, validate.rs the two post-dispatch
identity checks.

## Campaign target 3, slice 4: the analysis + process/capture families (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 34 rows, 12 unknowns. The
three families are 29 tools in six contract kinds (browser/electron/
runtime/artifact providers, application, session) with transport and
filesystem coordinates — never document/address/procedure, so no session
claim fires. Process capture owns four deadlines in one 20 ms poll,
precedence cancel > finalization > timeout > idle, exactly-once
SIGTERM→SIGKILL on a monotonic clock, undeliverable SIGKILL ends
`unobserved: true` with no invented exit. The terminal budget is a
rejection, never a clamp. Per-family failure types name the remote thing.
Owners ruled: runner.rs, browser.rs, reverse.rs, tools.rs, evidence.rs.

## Campaign target 3, slice 5: artifact / native / android families (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 49 rows. The artifact family
is 16 rows over one provider (rea-artifact-graph v1) in an empty registry,
split 8/8 on session target vs own path. The native family is four species
under one kind with a 60 s / 64 MiB command budget and a duration_ms + 60 s
LLDB deadline. Android speaks MCP to a hash-pinned JADX engine; the adb
provider's destructive axis is the device. Fixed defect (ruled, not
copied): the annotation derivation promotes write effects — destructive
derives from the full effect set including mayWriteFilesystem and
mayModifyDevice.

## Campaign target 3, slice 6: the provider bridges and session managers (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 57 rows, 17 unknowns. Six
transports, not one — unix-socket HMAC NDJSON, TCP descriptor, stdio MCP
peer, O_EXCL file-pair, injected NDJSON handle, argv stdout — ruled as a
transport trait in runner.rs. A second failure channel exists below JSON
(reserved exit 75/76 plus a one-byte marker file) for interpreter, import
and compile failures before any handler. Detach is not kill; Rizin has two
detach paths; Windows refuses persistent GDB sessions (Job-Object
cleanup). Open gaps (Cutter lifecycle, ReaGhidraBridge 53%) close in
slice 6b.

## Campaign target 3, slice 6b: the gap closures (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 33 rows, 8 unknowns.
Cutter's detach rule: it discovers sessions instead of starting them —
zero spawn/kill bytes in the whole dist; close/retryDescriptorCleanup on
one ArtifactResourceScope; each request settles behind an idempotent
finish with a 25 s timer; a timed-out command is released only on
confirmCommandCompletion receipt. ReaGhidraBridge is a cooperative-exit
bridge: no System.exit/Thread/interrupt in 4,910 lines, five refusal
gates, one authenticated connection, transport enum decides writability.
Ruled: typed failure codes, never message phrases; the Windows descriptor
residue is prevented in the port, not admitted.

## Campaign target 3, slice 7: the CLI surface — target 3 closed (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/rea/`): 37 rows, 12 unknowns. The
real entry is scripts/rea.mjs (both bin symlinks); dist/cli.js was a wrong
route, not a wrong launcher. Five error envelopes (routing, validation,
flag, analysis, semantics), all stdout, exit 1; exit codes are binary with
doctor's verdict as the only non-error non-zero. --schema has three
behaviours; --token-*/--filter-output/--full-output are inert on it. No
session, no config, no login — state crosses calls as files; the address
retry is CLI-only and the no-session error MCP-only. mcp doctor verifies
the catalog digest, 207/207 tools, 6/6 prompts. Target 3 closed: 322 rows (mission-file counts, corrected)
over eight slices, evidence bundles and graph agreement per slice.

## Campaign target 4: clearwing (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-clearwing/`): 39 rows, 33
proven, 16 container-bound unknowns. One thin launcher, a 25-module
command table, 30 parser verbs, dispatch by module basename plus aliases.
The engine is a per-thread-id dict state with a max_steps budget, one
assistant step per pass, boolean approval resume. The LLM layer wraps a
genai-pyo3 client with two retry classes and an env-gated call log;
prompts are Python strings, not data. semantic_memory and
episodic_memory ship compiled-only — slice 4b decompiles them. Graph:
13,346 nodes / 33,990 edges.

## Campaign target 4b: the clearwing memory backends (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-clearwing/`): 30 rows, 27
proven. Both backends wrap one SQLite memory.db: Knowledge/Episode
records, external-content FTS5 with three sync triggers, a threading.Lock
around every read/write. Homes: memory.rs, store.rs, pipeline.rs.
Preserve: top_k=5, recall limit 50, the cosine zero-denominator guard,
None-to-FTS fallback, one timestamp per batch, first_id+i synthesis,
falsy-metadata-to-{} and the embedding dropped on read. Fix, not copy:
full-table Python scan in the vector path, invented batch ids, unclosed
connections. Target 4 closed: 69 rows.

## Campaign target 5: cloudflare-check (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-cloudflare-check/`): 52 rows,
50 proven. It is a self-test framework, not a scanner: check.sh runs the
two validators' own 65 tests in a pinned, networkless container; the
product is the contract — a 461-line report schema, a hand-written
16-keyword JSON-Schema-subset validator, and a 19-field, 8-state run-plan
ledger. The three verdicts are separated by forbidden fields (the whole
producer discipline); not_applicable/out_of_scope/deferred require a
non-empty unresolved and no owner; candidate is the only state allowed
result_fingerprints; overall_severity rank is capped by impact.score rank;
every exit path is 1 except success; diagnostics escaped and clipped to
256 chars against terminal-control forgery. Ruled: validate.rs producer
gate, coverage.rs as-is, surface.rs diagnostics. Graph: 145 nodes / 310
edges.

## Campaign target 6, slice 1: pentagi CLI + schema (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-pentagi/`): 95 rows. The
"binary" is a 501-byte shim forwarding to docker compose after a
fail-closed preflight; the server ships only as Go source. The server has
one flag (-info, 8 s deadline, JSON stdout, exit 0/1). Schema: 25 tables
(13 initial + 12 migrated), 11 enum types, 6 updated_at triggers, goose
migrations with an advisory lock. Providers: three YAML presets x 13
agent roles; 11 keys match MSGCHAIN_TYPE, two do not, two roles have no
entry. Identity: ce/2.1.0-ce.h87ac00f, pinned worker image. Ruled: the
shim's preflight becomes the doctor probe; the compose forwarder is
replaced by the crate's own grammar. Slice 2: the Go engine + remaining
migrations.

## Campaign target 6, slice 2: the pentagi Go engine (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-pentagi/`): 106 rows, 103
proven. MSGCHAIN_TYPE grew to 15 values (tool_call_fixer, assistant);
MSGLOG_TYPE is 11. memorist runs on the searcher role; 15 chain types
collapse onto 13 provider roles. There is no supervisor agent —
supervision is performPlanner/performMentor inside performers plus the
flow worker's Report budget loop. Every chain gets the summarizer as its
8th argument; chaining is a message-array value, not a service. Retry is
deterministic-failure aware (4xx fatal except 408/429, nine typed
refusals, budget 3) — mapped onto the shared maestro-http policy, not
forked. Schema closed for 11 of 12 later tables; the LangChain vector
pair is not ported (no new backends). Slice 3: the web UI data contracts.

## Campaign target 6, slice 3: pentagi UI data contracts — target closed

Decomposed 2026-10-26 (in-session mission, REA-observed artifact
frontend.zip, sha256 aa91d7ae…): 10 rows. The flow entity (title, status,
terminals, provider), a five-state machine (created/failed/finished/
running/waiting — the run-state precedent), task/subtask ledgers with
input/result, typed terminal sessions (primary/secondary, connected) with
a three-kind log stream, live updates by GraphQL subscription (replaced by
the event seam), user-initiated run grammar with an agent-assisted
variant, form fields as the assess arguments, and provider caches as the
board view-model precedent. Target 6 closed: 211 rows over three slices.

## Campaign target 7: rea-check (issue #14)

Decomposed 2026-10-26 (in-session mission, REA-observed doctor.json,
ev_29c3bdc9): 8 rows. The four-way classification is confirmed by the
bytes: per-check healthy/config_drift/missing_analysis_engine/
unsupported_host, a document-level healthy aggregator, a separate
environment_healthy, a scope object (explicit clients/providers/skill/
target), scope vs informational check arrays, one-line remediations per
failure, caller-supplied engine verification ("REA does not download or
install it"), component-deep engine checks, and stale-skill detection.
Target 7 closed.

## Campaign target 8: shannon (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-shannon/`): 65 rows, 44
evidence records. A workspace is a directory under workspaces/ — no
registry, no index, no lock file. Its identity splits: the user-facing
name and the engine-facing workflow id, mapped only by session.json
inside the workspace — the load-bearing decision. launch.json is written
once and gates five refusals, so resume-to-a-different-target is
impossible; classification runs read-only before any write, so every
refusal promises the files are untouched; stop cancels then terminates,
bounded and idempotent; there is no freeze because nothing ever deletes a
workspace. CLI: nine verbs, one-level grammar, 2=usage/1=operational,
eight stable error codes, JSON envelope on stderr with stdout empty. The
worker runs six phases; the real step engine is per-vulnerability-class
(vuln -> SAST reconcile -> queue check -> conditional exploit). Temporal
is four mechanisms, not a queue: durable run record, per-activity retry,
cancellation channel, live query — mapped to store.rs, the shared retry,
the abort signal and the event seam. Target 8 closed.

## Campaign target 9, slice 1: PentestGPT supervisor model (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-pentestgpt/`): 53 rows, 43
proven. base_revision is a real CAS twice — compile-time equality then a
BEGIN IMMEDIATE + SELECT revision + StaleRevisionError write txn. The
decision type is closed at three layers (schema additionalProperties
false, runtime exact-key check, field-level types). Finish discipline:
1..4 unique observation ids from DONE tasks, board closed, backlog capped
at one task selected in the same decision. Injection defense is
prompt-only — instruction + schema discipline, evidence verbatim in the
state JSON, blast radius bounded by the compiler. FIXED DEFECT (not
ported): the reference leaves StaleRevisionError uncaught; the port
catches it, re-reads the snapshot and re-decides, with a red-first test.
Episode ids are revision-scoped. Slice 2: episodes, prompts, tools.

## Campaign target 9, slice 2: PentestGPT episodes, prompts, tools — closed

Decomposed 2026-10-26 (mission `rea-v2/fw-pentestgpt/`): no tool registry
to port — UnifiedAgent runs with no tools= and the vendored ToolRegistry
has zero call sites; the run's surface is the pipeline's runners plus the
box exec. Episode ids: supervisor-r{revision}[-a{n}]; executor ids from
the persisted trace column. Prompt building is byte-exact: 10 state keys,
validation_feedback capped at 1,000 chars, canonical JSON after a fixed
preamble, bounded working set. Traces: three files per episode, atomic
.tmp -> os.replace with 0600, per-event fsync, four signature hashes,
omitted markers. FIXED DEFECT: closed_tasks[:-4][-4:] slices [-8:-4] —
the port takes the last four closed tasks with a red-first test. Target 9
closed.

## Campaign target 10, slice 1: cai AP + CTR core (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-cai/`): 24 rows. Correction:
"AP" is an agentic-pattern topology — AP = (A, H, D, C, E): Agents,
Handoffs, Decision, Communication, Execution — with a five-pattern
catalogue (Swarm, Hierarchical, Chain-of-Thought, Auction-Based,
Recursive); nothing is an "adversarial probe". "CTR" is Cut The Rope: a
game-theoretic attack-graph LP solver — one LLM inference builds a 4-10
node graph, weights come offline from the transcript, then two scipy
linprog calls over an attack-rate x defense-rate grid with 100 defender
subgraphs dropping 20% of nodes. There is no attack-path traversal and no
declarative test object; a "test" is a JSONL conversation log. The digest
is a deterministic rule-based interpreter over three result files. The
port maps the topology to agents.rs, replaces scipy result-first, and
keeps the deterministic digest. Slice 2: the 36 tools.

## Campaign target 10, slice 2: the cai tools (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-cai/`): 46 rows, 40 proven.
The inventory is 41 tools, not 36 — the registry is OFF by default
(about 120-150 prompt tokens per tool schema); agents run on hand-written
import lists. No tool calls an LLM: the five "AI" tools are HTTP clients
(Perplexity via the OpenAI SDK, Google CSE). Tool failure is a string
back to the model, never run failure; only the user-cancel aborts. Six
categories (defensive, monitoring, forensics, lateral, privesc, most of
exploitation) resolve to zero tools — taxonomy leads inventory. Ruled:
inventory corrected, registry precedent confirms the manifest design,
the HTTP tools map to sourcehunt.rs, the failure shape lands in
pipeline.rs, empty categories stay taxonomy-only. Slice 3: the executor
output contract.

## Campaign target 10, slice 3: the cai executor contract — target closed

Decomposed 2026-10-26 (mission `rea-v2/fw-cai/`): 42 rows, 41 proven.
The executor never truncates; truncation happens twice above it — cai's
env-gated rule (off by default) plus the SDK's hard 10,000-char cap
(first 5000 + marker + last 5000). One event carries three timeout
strings (async/sync/SSH). SECURITY FIX, never ported: a failed
container/SSH run silently re-executes the command on the HOST at five
call sites — the port refuses with ExecutionFailed and never falls back.
execute_code is a permanent file with a 14-language table and a timeout
parameter. Target 10 closed: 112 rows over three slices.

## Campaign target 11, slice 1: Pentest-Swarm-AI blackboard + confidence (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-swarm/`): 50 rows. The
blackboard is an immutable typed finding log with AND-predicate or push
channel reads, one RWMutex, and fan-out that drops silently when a
subscriber's 32-slot buffer fills — correctness leans on cursor replay.
Confidence is pure half-life decay computed at read time, never stored;
base clamped to [0,1], half-life unclamped, and the Go/SQL engines
disagree on half_life <= 0 (no decay vs 1 s clamp). Accrual is rewrite +
Supersedes, not reinforcement; gates stack agent MinPheromone 0.05-0.7,
report publish 0.5, Jev P(real) < 0.5. The quality gate discounts only
inferential findings by a price tier, at exactly one call site. Jev is a
single yes/no question over the finding text, batches of 16, opt-in,
fail-open in three places. Ruled: overflow recorded not silent, Go
semantics for the half-life edge, fail-closed Jev. Slice 2: PoC loop,
exploit chains, gate outputs.

## Campaign target 11, slice 2: the swarm PoC loop and gate outputs — closed

Decomposed 2026-10-26 (mission `rea-v2/fw-swarm/`): 40 rows, 35 proven.
Verified-only-if-fires has a soft centre: the fire test reads the
script's own stdout for VULNERABLE/NOT CONFIRMED — the model's prompt
contract is the oracle and nothing re-parses the target; a second
Verified=true write sits at classifier.go:126 (open unknown). Chains
have no executor — Links compile into a playbook and a CLI printer;
"safe verify" per link is a sentence handed to an LLM. Two quality
concepts: the submission rubric (Threshold 6.0, grade_submission) is
unrelated to the pheromone discount. SARIF is hand-built 2.1.0 with
LLM-derived rule ids; PDF is chromium --no-sandbox with an HTML
fallback; ROI grades on the low band. Ruled: the verify gate
re-observes the target (model stdout never the sole oracle); chains
stay data; the two gates stay distinct; the --no-sandbox PDF is
refused. Target 11 closed: 90 rows.

## Campaign target 12, slice 1: AIDA cards + recon + credentials (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-aida/`). Corrections by bytes:
there is no board — cards render through a unified table; recon types are
not typed (data_type is String(50), snake_cased, only service/endpoint/
vulnerability written); the ASVS payload is 177.9 KiB. Highest-value:
status (confirmed/potential/untested) is a bare string with no transition
guard — any state reaches any state via one setattr loop; proof is a bare
Text column with no command/output/file reference; recon-to-finding is a
double-write sharing a copied blob; credentials are plaintext with a
per-assessment placeholder resolution endpoint. FIXED in the port:
guarded transitions, evidence-lineage proof, one linked record, encrypted
credentials at rest with box-only placeholder resolution. Slice 2: the
MCP surface and the FastAPI contract.

## Campaign target 12, slice 2: the AIDA surfaces — target closed

Decomposed 2026-10-26 (mission `rea-v2/fw-aida/`): 50 rows, all proven.
The MCP server has no store — it is a REST client over the FastAPI
contract (localhost:8000/api, Bearer token), mounted as /mcp inside the
same app; 21 tools, 2 resources, one dispatcher; tool calls travel the
same routers so websocket broadcasts fire identically. Four defects not
copied, each fixed in the port: a fail-open gate on settings-read
exceptions; gating keyed to DEPLOYMENT_MODE (container deploy gates
nothing by default); pending-command expiry written only on read; and
two executors with two error contracts (MCP errors as text on HTTP 200,
REST as status codes). Ruled: fail-closed, deployment-independent,
absolute expiry, one executor with one error contract. Target 12 closed.

## Campaign target 13, slice 1: Dark-Moon domains + step engine (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-darkmoon/`): 45 rows, 10
unknowns. Two layers, zero coupling: 51 flat .md domain agents (not 55 —
four absent in this checkout, recorded as a delta) loaded via static
prompt_file config; the step engine is Python with no step model, no
abort class, statement-order sequencing, retries defaulting to zero.
SAFETY FIX: _validate_command splits on &&/pipe/; and allowlists only the
first token — later segments ride on the first tool's approval; the port
validates every segment. The doctor is shell: a probe ladder
(nvidia-smi -> WSL2 dxg -> ROCm -> /dev/kfd) published to
/run/darkmoon-gpu.env, gating hashcat viability. Ruled: every-segment
validation, typed doctor with the same ladder, domain files as catalog
knowledge packs. Slice 2: the 13 MCP tools + workflow discovery.

## Campaign target 13, slice 2: Dark-Moon tools + discovery — target closed

Decomposed 2026-10-26 (mission `rea-v2/fw-darkmoon/`): 67 rows, 66
proven. The validation hole is on BOTH execution paths (shlex.split then
token-0-only, in base.py and executor.py) — the every-segment validator
must live in the shared runner validation. Two allowlists disagree (142
with classify pre-flight vs 38 without) — a tool refused on one path is
reachable on the other. Discovery is glob + importlib over public
BaseWorkflow subclasses keyed by module stem; two subclasses silently
keep the last; get_workflow_info has no tool wrapper (dead surface).
execute_command returns prose, not JSON; registry failures are {error}
dicts against the docstring's ValueError; privacy defaults to DEGRADE;
the sanitizer's any-tool exception path fails open. Ruled: one shared
validator, one allowlist, explicit registration, fail-closed sanitizer,
strict redaction default. Target 13 closed: 112 rows.

## Campaign target 14: hackingBuddyGPT — target closed

Decomposed 2026-10-26 (mission `rea-v2/fw-hbgpt/`): 124 rows, 120
proven. The registry is class-name-as-tool-name: a Capability ABC builds
a pydantic model from __call__'s signature, the union is the catalogue,
duplicates are a hard ValueError. Concurrency is a flag (asyncio.gather
default, sequential opt-in), history order is the model's order, with no
cancellation and no per-call timeout. privEsc has no state enum — three
template strings rewritten per command; the real states live in
Limits.RunState. The no-repeat rule is prompt text only. Limits.reached()
checks COMPLETED before any budget — a last-round solve scores success.
Fixed in the port: cancellation and timeouts, a real seen-set, a typed
CoT error, credential-type keying. Target 14 closed.

## Campaign target 15, slice 1: the strix orchestrator (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/fw-strix/`): 25 rows, 23 proven.
The budget ordering guarantee is a 90% reserve: sub-agents are refused at
90% of max_budget_usd, the whole scan stops at 100%, and the last tenth
exists so the root can write its report. Budget counts LLM spend in USD
only. "Three agents per finding" exists only in the prompt — no code
counts agents per vulnerability. The run ends by a model tool call
(finish_scan); turn cap and budget are the only non-model exits. Ported
mechanics: pre-call refusal repeated at three independent sites,
epoch-guarded resume (limit raised before the pause flag clears),
budget_paused counting as active, the <cache_point> at prompt line 520
(532 newline-terminated lines, not 533). Ruled: the reserve becomes the
report-phase budget floor; prompt-only rules are not ported. Slice 2:
phases, cost site, tools.

## Campaign target 15, slice 2: strix phases, cost, tools — target closed

Decomposed 2026-10-26 (mission `rea-v2/fw-strix/`). Phases are five
free-form strings with no schema, all naming setup, no scan-activity
phase, and failure never clears the last phase. The cost site is
ReportState._llm_usage.total_cost — live in-memory, deliberately not the
persisted record, one writer and four enforcement readers; bands proven
exact: root 70/85/95, sub-agent 75/80/85, reserve 90;
DEFAULT_MAX_TURNS = 500. The "~90 tools" are three inventories: 46
in-repo @function_tool functions in 10 families, an SDK toolset of four
CustomTools, and runtime MCP tools. The prompt tells the model every
agent has shell + browser while the factory disables view_image —
roster facts stated as invariants vs per-agent config. Ruled: typed
phases with terminal states, RunSpend as the cost model, the manifest
as the roster truth. ALL SIXTEEN FRAMEWORKS DECOMPOSED.

## Campaign target 20, slice 1: the security skills (issue #14)

Decomposed 2026-10-26 (mission `rea-v2/skills/`): 39 rows, 20 proven.
Sixteen of the seventeen named skills exist in the checkout (aws-
observability and aws-security do not — their earlier credit is revoked
until found or ruled absent). 4,190 SKILL.md lines covered; section-
granularity rows are labelled likely, never proven. Ruled: the
three-tier boundary discipline lands on maestro.security; the
constraint floors land on maestro.proof. Slice 2: the box's 135
skills.

## Campaign target 20, slice 2: the box skill library — campaign complete

The box carries 41 SKILL.md files, not 135 — the 135 figure was a
host-side count; most box skills are vendored typer/playwright tool
docs, not security knowledge. The security-relevant skill corpus was
fully covered by slice 1, the reverse-skill (target 1) and the box's
own maestro-security skill. Target 20 closed. THE CAMPAIGN IS COMPLETE:
all 16 frameworks, REA, the reverse-skill, leanrea and the skills are
decomposed byte-per-byte with evidence and graph agreement. Next: P2,
the fusion.

## Closure pass (Sol review round, 2026-10-26)

ADR-0020 row-level agreement: a deterministic join ran over all 27
mission tables against the regenerated graphify extractions (framework
graphs: reverse-skill 43, method 11, REA 1,482, clearwing 13,346,
cloudflare 145, pentagi 25,762, swarm 4,726, AIDA 2,322, Dark-Moon
1,421, HBGPT 1,663, strix 11,521, skills 1,528 + 306; maestro graph:
905 nodes). 1,386 rows joined with framework-node and maestro-node
citations; every unmatched row carries an exclusion report in its
mission file. Data-only targets (rea-check) carry an explicit waiver
(ADR-0023). Closure rulings appended to the mission files: the mcp add
--schema mutation, URL API keys, hidden failures + discarded stderr, the
stdout-only contract preserved, and the memory-on-JSON-stores mapping
(ADR-0022).

## Closure pass 2 — the unread behaviour (Sol review round, 2026-10-26)

The review's blocker-2 gaps are captured with byte evidence: the 30
Ghidra operation bodies (slice 6c, 35 rows — dispatch enforced, the
cursor-as-capability rule, commit-on-readback); the native_api facet
(slice 6d — all 1,062 lines, 34 declarations; the eleven-premise chain
and the dual-recovery gate); the PentestGPT anti-fabrication receipt
matcher (slice 2b, 42 rows — six exact-byte attempts, the
StructuredOutput exclusion, journal-slice observations); the skill
bodies (slice 1b, 67 rows; slice 1c, 96 rows — all 11 process bodies);
and the companion references (slice 1d, 78 rows — all six files). Every
slice carries its evidence bundle and supervisor rulings. No unread
product behaviour remains; the open unknowns all carry dispositions.
