# maestro-security — master spec

One image, one pipeline, one brain. The master document for the whole
maestro-security system. Issue-scoped specs (AUTONOMY.md, …) are chapters
of this document; this is the complete picture.

## 1. What this is

Maestro's autonomous AI pentester, the "massive secret weapon": a Kali box
carrying raw tools, skills and models, driven by ONE unified Rust pipeline
fused from 13 decomposed AI-pentest frameworks. The ten box frameworks are
retired — maestro-security is the sole assessment path. The image ships in
two versions: abliterated (default, no guardrails) and standard
(git-pristine). One methodology, no profiles; the model is a runtime value
resolved from the bridge, never a binding.

## 2. What exists today (landed, PR #6)

- **Crates**: `maestro-security` (35 modules) + `maestro-http` (shared
  client with retry).
- **Pipeline**: 7 phases (Recon → Model → Hypotheses → Verify → Deepen →
  Threat → Report), 5 Maestro-branded agents, 6 pattern classes.
- **9 tools on 2 surfaces** (WASM extension + MCP stdio via rmcp):
  doctor, route, assess, validate, evidence, report_page, evidence_page,
  disclose (brief + SARIF), workspace.
- **Models from the decomposition**: 3-verdict findings (CONFIRMED /
  NeedsValidation / Rejected), 8-state coverage ledger, 4-way doctor,
  optimistic-concurrency workspaces, cards + credentials stores.
- **Evidence**: one JSON file per run under `$MAESTRO_SECURITY_HOME`;
  reports render as HTML artifacts (Maestro's live-page mechanism).
- **maestro-http**: TLS, chunked decoding, retry (default 3, clamped 1–5,
  exponential backoff, Retry-After).
- **SKILL.md**: pi-bar knowledge — the nine tools, the canonical workflow,
  knowledge sources (CVE/OWASP/ATT&CK), rules.
- **Fusion proof**: DECOMPOSITION-MATRIX.md — 1,089 communities, 727
  behavioural, 724 owned, 0 gaps; UNIFIED-DESIGN.md holds the design.
- **Box**: the `maestro-security:local` image carrying the raw tools
  (nmap, sqlmap, nuclei, …), the runner on 8100 (NDJSON) and the box
  bridge for LLM routing; two-version mode machinery.
- **Live proof**: `tests/live_assess.rs` plus a full 7-phase run executed
  on qwythos; the run record lives in the workspace ledger.

## 3. The contract with Maestro

- **Extension**: cdylib for `wasm32-wasip2` (WASI 0.3 async), 9 registered
  tools; MCP stdio server on rmcp 3.7.0 for host use.
- **Events/hooks**: `on`/`event-on`/`event-emit`/`send-message` plus the UI
  context (status, working message, widgets) — the binding for autonomy
  (chapter R1). Always bind to Maestro hooks and events.
- **No new backends**: state lives in JSON files on top of Maestro's
  existing stores. No GraphDB, no vector DB, no database of any kind.
- **Webpages**: reports are HTML artifacts; Maestro renders them live.

## 4. The program (roadmap, in order — settled by the grill)

The end state: maestro-security assimilates everything into ONE Maestro-branded
Rust framework — the 16 frameworks, REA (tools, docs, concepts), the
reverse-skill, leanrea and every skill — shipped as one catalog package with
its manifest in the maestro-manifests repository. The box keeps raw tools and
models only.

### P1 — The decomposition campaign

Byte-per-byte decomposition of all 16 frameworks + REA + reverse-skill +
leanrea + every skill, run by an agent whose only tools are REA's
(ADR-0016). Every row cites its graphify node; the graph blast radius and
the matrix must agree row by row before any build (ADR-0020); a review
accepts only with evidence. Decompose first, then build (ADR-0019).

### P2 — The fusion

Nothing is built or manifested incrementally. When the full campaign is
done (every target decomposed, graph + matrix agreeing), ONE unified
design (UNIFIED-DESIGN.md v3) fuses every decomposed row into the
maestro-security architecture: modules, agents, knowledge packs, the
workflow. The fusion is the single source the manifest and the build read
from.

### P3 — maestro-manifests

New repository Orchestration-Maestro/maestro-manifests, following the
manifest schema Maestro already documents (manifest mode names one pinned
release, e.g. `git:github.com/org/maestro-manifests@v1`). The security
package manifest is written from the fused design, complete — nothing
partial (ADR-0013, ADR-0018). The first draft (PR #18) is held until then.

### P4 — The runner build

`maestro-security-runner`: one Rust crate — the unified framework. Agents
(defined in the catalog, living in Maestro, ADR-0014), knowledge packs per
agent, the modular pipeline (phases as catalog items, `--phases` and
`--profile`, default the full seven), the REA modules, the exec client.
Includes the autonomy run layer (issue #7, spec AUTONOMY.md): the run as
an isolated background process with its own log, the curated timeline, the
`/maestro-security` grammar on four transports, checkpoint + resume.
Built file-by-file from the matrix, red-first, AWS-only.

### P5 — The box strip-down

Box = raw tools + models only. All 16 frameworks are removed as code; the
box daemon is reduced to command exec. The runner embeds the brain.

### P6 — Catalog integration

Installing the extension adds the security package to Maestro's catalog;
the agents register in Maestro; modular workflows are selectable; the
pipeline is user-initiated only (ADR-0021).

### P7 — The four transports

One grammar `/maestro-security <verb>` (assess, stop, pause, resume,
steer, list, board, logs, report) on session commands, the maestro CLI,
MCP and RPC (ADR-0005); the 9 tools stay the on-demand capability surface
behind the same core.

### P8 — Campaigns (follow-up)

Missions compose into campaigns; opens as its own ticket after the
mission is right.

### P9 — Larger-workflow binding + live-visual polish (follow-up)

`maestro.security.assessment` consumed by other Maestro workflows;
Forge-branded board and pages, TUI status and widgets.

### Tracked elsewhere

- `module_name_repetitions` violations across the maestro core repo
  (separate lane, tracked in the maestro workspace).
- Box image backup cadence (local D: drive only, no GHCR).

## 5. Owner decisions (register)

- Sole assessment path; the ten box frameworks are retired (2026-10-25).
- Subagents and all lanes: deepseek/deepseek-flash only.
- Default model qwythos, resolved at runtime from the bridge/router.
- Retry: default 3, overridable 1–5 per caller.
- No new backends of any kind; schemas on existing stores.
- Single-purpose functions; query the code graph before every plan (owner
  OD-133); no duplicated code.
- No local builds: every check through `maestro-remote-build run`.
- Brand: Forge palette; Maestro names only in code and docs.
- Autonomy grill (2026-10-25): the four R1 decisions.
- Repo mechanics: `packages` label + Repository stamp on every issue; main
  is ruleset-protected, so everything lands via squash PRs.

## 6. Non-goals

- No profile selection (one methodology, no feature loss).
- No new backend, no polling loops.
- No browser-only live surfaces (TUI first).
- No re-coding of the retired frameworks — fusion only, 0 gaps proven.

## 7. Workflow binding

AGENTS.md governs: issue-first with the ask-matt pipeline (grill-with-docs
→ to-spec → to-tickets → implement per ticket with tdd → pr → retro),
two-axis review per PR, AWS-only verification, squash merges, humanizer on
every documentation change.
