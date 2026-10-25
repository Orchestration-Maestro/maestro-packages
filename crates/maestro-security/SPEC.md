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

## 4. Everything we want to do (roadmap, in order)

### R1 — Autonomy (issue #7; chapter spec: AUTONOMY.md)

Owner decisions (grill, 2026-10-25):

1. Trigger: a command only — never message parsing, never the web UI.
2. Run style: a background run with live updates and a visible status;
   bidirectional control (kill/steer) from the session chat, exactly like
   subagent workflows; modular; works in the TUI.
3. Scope: box tools + evidence/report files only; no workspace writes.
4. Results: files + report page + notification, and the result event is
   emitted back to Maestro so larger workflows can consume it.

Surface: `/assess <target>` (returns a run id), `/security-stop <run_id>`,
`/security-steer <run_id> <message>`; progress events per phase boundary
and per finding; result event `maestro.security.assessment`. One module per
concern, reusing the nine shared runners. Estimate: 150–200 production
lines. Next: to-tickets → implement (red-first) → two-axis review → PR.

### R2 — Box bake (task #15, box lane)

REA framework + all security skills baked into the box image; two-version
mode verified end to end (abliterated default, standard git-pristine);
runner and bridge health wired into the doctor probe.

### R3 — Live visuals and TUI polish

Forge-branded live surfaces: status line, working indicator and progress
widget in the TUI; report page + attack graph refinement in the browser.
Traces to owner grill decision 2 (the run must have a real visual and work
in the TUI).

### R4 — Larger-workflow binding

Consume `maestro.security.assessment` in Maestro workflows so an autonomous
run is a step in bigger pipelines (hooks/events chain), per the owner's
"always bind to Maestro hooks and events".

### R5 — Elsewhere (tracked, not this repo)

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
