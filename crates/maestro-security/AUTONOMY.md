# maestro-security autonomy — spec (issue #7)

The spec written from the grill (2026-10-25, five rounds, owner-confirmed).
The domain language lives in CONTEXT.md; the decisions live in
docs/adr/0001-0012. This document is the build plan.

## 1. The essence

A run is an isolated background process inside Maestro: its own workflow
(the seven-phase pipeline), its own log, its own state. The session shows
only a curated timeline of it. You and the Maestro agent reach it through
one command grammar — `/maestro-security <verb>` — on four transports:
session commands, the maestro CLI, MCP, and the RPC surface. Its findings
and results leave the run only through the event seam. It outlives the
session, checkpoints after every phase, and resumes after a kill or crash.

## 2. The decisions

All decisions are recorded as ADRs and are binding:

- ADR-0001 trigger is a command, never message parsing or the web UI
- ADR-0002 the run is a session citizen (curated presence, steerable)
- ADR-0003 runs are extension-owned; the native-hosting gap is deferred
  until maestro is fully set up
- ADR-0004 a run is an isolated background process with its own workflow,
  logs and state
- ADR-0005 one grammar, four transports: session commands, CLI, MCP, RPC
- ADR-0006 a run outlives the session
- ADR-0007 every run starts with the doctor probe (fail fast)
- ADR-0008 no hard cap on a run's life
- ADR-0009 local-first authorization (user + agent; hosted auth later)
- ADR-0010 evidence lives forever (manual cleanup only)
- ADR-0011 no run steers another run
- ADR-0012 two runs may share one target (the box serializes exec)

Owner decisions from the earlier grills: steering shapes priorities and
guidance, never the evidence (verdicts are sacred); the curated timeline
carries phase transitions and findings only; campaigns compose assessments
and open as a follow-up ticket.

## 3. The run

- States: `queued → running → completed | killed | failed | paused`.
- The run is a background task with its own state; nothing of it enters
  the session's context.
- It survives the session ending and Maestro restarting; the board shows
  it on the next start and steer/stop/logs keep working.
- Every run starts with the doctor probe; an unhealthy result refuses the
  start with the classification before any phase runs.
- No duration cap: steering, pause, stop and the abort signal are the
  controls; per-phase timeouts stay where the pipeline already has them.
- Runs are independent: no run steers another; two runs may share a target
  and the box serializes tool exec per target.
- Kill: abort signal, evidence kept, resumable from the last checkpoint,
  no result event. Pause: hold at the next phase boundary, checkpointed,
  visible, resumed by the resume verb.

## 4. The event seam

Channels on the shared bus (literal names; JSON payloads; per-run channel
`maestro.security.run.<run_id>` mirrors every event):

| Channel | Carries |
| --- | --- |
| `maestro.security.run.started` | run id, target, source |
| `maestro.security.run.phase` | run id, phase, status |
| `maestro.security.run.finding` | run id, fingerprint, verdict |
| `maestro.security.run.refuted` | run id, hypothesis |
| `maestro.security.run.steered` | run id, source, message |
| `maestro.security.run.paused` / `.resumed` | run id, phase |
| `maestro.security.run.killed` | run id, phase, resumable |
| `maestro.security.run.checkpoint` | run id, phase |
| `maestro.security.run.failed` | run id, class, detail |
| `maestro.security.assessment` | run id, report, verdict counts |

`maestro.security.trigger` stays internal machinery. The seam is the only
route findings and results leave a run; evidence and reports are immutable
and larger workflows consume them as events.

## 5. The command surface — one grammar, four transports

```
/maestro-security assess <target> [--model <m>] [--resume <run_id>]
/maestro-security stop <run_id>
/maestro-security pause <run_id>
/maestro-security resume <run_id>
/maestro-security steer <run_id> <message>
/maestro-security list
/maestro-security board
/maestro-security logs <run_id> [--follow]
/maestro-security report <run_id>
```

The same grammar on all four transports: session slash-commands, the
maestro CLI, the MCP server tools, and the RPC methods. One core behind
them; no transport-specific grammar.

## 6. The curated timeline

Each run opens a curated session entry: phase transitions and findings
only, updated as they happen, closing with the summary (verdict counts,
spend, report link) when the run completes. The run's raw events, prompts,
tool outputs and spend never enter the transcript.

## 7. Steering

- Verbs: `stop`, `pause`, `resume`, `focus <fingerprint>`, free text.
- Steering shapes priorities and guidance only: the target, the evidence
  and the verdict rules stay fixed.
- Read at phase boundaries and at hypothesis boundaries inside Verify; the
  last five messages fold into the next phase's prompt, quoted, as
  untrusted input.
- You and the Maestro agent steer through the same surface; the agent sees
  the curated timeline and the board, not the run's raw events.

## 8. Logs, evidence, checkpoints

- `$MAESTRO_SECURITY_HOME/runs/<run_id>/log.jsonl`: every event, phase
  prompt, tool output, spend and steering message, appended as it happens;
  readable via `/maestro-security logs`.
- Evidence files per run, as today; reports as HTML artifacts.
- Checkpoint after every phase; resume continues at the next phase with
  evidence and findings intact; the partial report marks the break.
- Everything is kept until the owner deletes it (ADR-0010).

## 9. The run board and notifications

- Board: all runs with id, target, state, phase, elapsed, findings —
  rendered from the one event stream in the TUI (status line, board
  widget) and the browser (board page, live run page).
- Notifications on findings and completion, on both surfaces.
- Authorization: the session's user and the agent (ADR-0009).

## 10. Failure model

| Class | Behaviour |
| --- | --- |
| model transient error | shared retry (default 3, clamp 1–5) |
| model persistent error | run failed (model_error), evidence kept, resumable |
| box unreachable | run failed (box_unreachable), doctor classification, resumable |
| kill mid-phase | checkpoint of last phase, killed event, resumable |
| pause | hold at boundary, checkpointed, resumed later |
| crash / restart | run keeps living or resumes from checkpoint (ADR-0006) |
| steering flood | bounded queue, drop-oldest recorded |

## 11. Modular layout — one job per module

Core (the run machinery, reusing the nine shared runners — no duplicates):

- `autonomy/mod.rs` — the grammar: verb parsing, one parser per verb.
- `autonomy/run.rs` — lifecycle: state machine, phase boundaries, abort
  checks, doctor probe at start.
- `autonomy/log.rs` — the run log (log.jsonl writer + reader).
- `autonomy/events.rs` — the seam: channel names and payload builders.
- `autonomy/steer.rs` — steering queue, verbs, prompt folding.
- `autonomy/checkpoint.rs` — checkpoint write/load/resume.
- `autonomy/board.rs` — board view model + status updates.
- `autonomy/completion.rs` — validate gate, result event, files, artifact,
  summary close, notification.

Surfaces (each transport = one module over the core):

- `autonomy/session.rs` — the session slash-commands (register-command).
- `autonomy/cli.rs` — the maestro CLI verbs.
- `autonomy/mcp_surface.rs` — the MCP tools (extends mcp.rs).
- `autonomy/rpc_surface.rs` — the RPC methods.

The four transports are thin adapters; no logic outside the core.

## 12. Tests (red-first)

- grammar: each verb parses; bad arguments error per verb.
- doctor-at-start: unhealthy refuses the run with the classification.
- run: lifecycle events in order; the curated entry opens, updates, closes
  with the summary.
- steering: reaches the running phase; steered event; focus reprioritizes;
  pause holds at the boundary; resume continues; stop emits killed, no
  result event.
- logs: log.jsonl appends every event and steering message.
- checkpoint: written per phase; resume restarts at the next phase;
  partial report marks the break.
- board: lists concurrent runs with live state; two runs, one target,
  serialized exec.
- completion: assessment event, same files as a manual run, immutable.
- surfaces: all four transports dispatch the same core verbs (transport
  tests, not re-tests of core logic).

## 13. Task map (issue #7, revised after the grill)

- T1 Rework the grammar: `/maestro-security` verbs replace the three
  standalone commands; run registry unchanged in shape.
- T2 Rework the run: isolated background process + log.jsonl + curated
  timeline (the landed session-event wiring becomes the timeline writer).
- T3 Steering + pause/stop/kill.
- T4 Checkpoint + resume.
- T5 Board + notifications (TUI + browser).
- T6 Completion + result event + summary close.
- T7 CLI surface.
- T8 MCP surface.
- T9 RPC surface.
- T10 Modularity + docs + SKILL.md section + humanizer.

## 14. Size plan

The planned run layer is about 900–1,200 production lines across the ten
tasks. The four transports remain thin adapters over one core. The current
extension provides assessment triggering and lifecycle events; controls,
isolated process hosting and recovery remain open implementation tasks.
