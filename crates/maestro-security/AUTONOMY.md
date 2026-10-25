# maestro-security autonomy — spec (issue #7)

## The essence

An assessment run is a Maestro citizen: a live, subagent-like session
entity whose whole life is a Maestro event stream. You see it in the
session, steer it from chat, kill it instantly; the Maestro agent can steer
it too. Many runs live at once, each with its own channel, steering queue
and abort signal. Runs checkpoint after every phase and resume after a
crash or kill. One UI layer renders the same event stream in the TUI and
the browser, and the result event composes into larger Maestro workflows.

## Owner decisions (grills, 2026-10-25)

1. Trigger: a command only — never message parsing, never the web UI.
2. Run style: background run, live updates, a real visual, bidirectional
   control from the session chat, modular, works in the TUI.
3. Scope: box tools + evidence/report files only; no workspace writes.
4. Results: files + report page + notification, and the result event is
   emitted back to Maestro; always bound to Maestro hooks and events.
5. Identity: every run is subagent-like — its own live entry in the
   session; you steer by writing to it.
6. Steering: you (free text) AND the Maestro agent (it sees run events and
   sends guidance mid-run).
7. Concurrency: many runs at once, with a run board listing live state.
8. Durability: checkpoint after every phase; resume from the last completed
   phase; partial evidence kept.

## Architecture

### 1. Run lifecycle

`started → phase (recon → target-model → hypotheses → verify → deepen →
threat → report) → completed | killed | failed`. Every transition emits its
event. Kill uses the abort signal, keeps the evidence and stays resumable.
A crash resumes from the last checkpoint.

### 2. Event contract (shared bus, literal channels)

| Channel | Payload |
| --- | --- |
| `maestro.security.run.started` | run id, target |
| `maestro.security.run.phase` | run id, phase, status |
| `maestro.security.run.finding` | run id, finding |
| `maestro.security.run.refuted` | run id, hypothesis |
| `maestro.security.run.steered` | run id, source, message |
| `maestro.security.run.killed` | run id |
| `maestro.security.run.checkpoint` | run id, phase |
| `maestro.security.assessment` | run id, report — the result event larger workflows consume |
| `maestro.security.run.<id>` | per-run channel for scoped subscriptions |

### 3. Session identity

Each run opens a session entry (custom entry `security-run`) with its live
thread. Progress updates append to it; steering is a message addressed to
it, from you or the agent. The run entry is the subagent-like surface.

### 4. Steering protocol

- `/assess <target>` — opens the run entry, starts the background run,
  returns the run id.
- `/security-stop <run_id>` — abort signal; Killed event; evidence kept;
  resumable.
- `/security-steer <run_id> <message>` — queued; read at the next phase
  boundary and folded into that phase's prompt.
- Agent steering: the agent sees run events and sends guidance the same way.

### 5. Concurrency and the run board

Many runs at once, each with its own channel, steering queue and abort
signal. The run board lists id, target, phase, elapsed and findings so far;
it renders from the same events in the TUI (status line, working indicator,
widget) and the browser (live progress page).

### 6. Durability

One checkpoint JSON per run after each phase under
`$MAESTRO_SECURITY_HOME/runs/<id>/checkpoint.json`. A killed or crashed run
resumes at the next phase with evidence and findings intact; the partial
report marks what was proven before the break.

### 7. UI layer

One layer over the event stream: TUI status line and working indicator
during phases, the run-board widget, notifications; browser live progress
page plus the final report artifact. No browser-only live surface.

### 8. Modular layout

One module per concern, single-purpose functions:

- `autonomy.rs` — trigger surface (T1, landed): the three command specs,
  argument parsers, the run registry.
- `run.rs` — lifecycle stepping and checkpoint/resume.
- `events.rs` — the event contract: channel names and payload shapes.
- `steer.rs` — the steering queue and prompt folding.
- `board.rs` — run-board state and status updates.
- `completion.rs` — validate gate, result event, files, artifact,
  notification.

Reuse, never re-code: tools.rs shared runners, router.rs, validate.rs,
store.rs, evidence.rs, doctor.rs, maestro-http. No new backend, no polling;
WASI 0.3 async for the background task.

### 9. Failure modes

| Failure | Behaviour |
| --- | --- |
| Model transient error | shared retry (default 3, clamped 1–5) |
| Box unreachable | run fails with the doctor classification; evidence kept |
| Kill mid-phase | last completed phase is the checkpoint; Killed event |
| Crash | resume reads the checkpoint; partial evidence kept |

## Tasks (work blockers-first)

- T1 Trigger + run record — LANDED (3f541fd): the three commands, run
  registry.
- T2 Run lifecycle + events + session entry — phase stepping emits the
  event contract; the run entry updates live. (blocked by T1)
- T3 Steering + kill — steering queue (user + agent), abort signal, Killed
  and steered events. (blocked by T2)
- T4 Checkpoint + resume — checkpoint JSON per phase, resume path, partial
  report on kill/crash. (blocked by T2)
- T5 Run board + UI layer — run board state, TUI status line/widget,
  browser live page over the same events. (blocked by T2)
- T6 Completion + result event — validate gate, maestro.security.assessment,
  files + artifact + notification. (blocked by T2)
- T7 Modularity + docs — module layout, shared-runner reuse audit, SKILL.md
  section, humanizer. (blocked by T1–T6)

## Tests (red-first)

- trigger: the three commands register; missing target is an argument
  error; a target returns a run id and records the run (T1, landed).
- lifecycle events fire in order on the channels above (T2).
- steering reaches the running phase; steered event fires (T3).
- stop emits Killed and no result event (T3).
- a killed or crashed run resumes at the next phase (T4).
- the run board lists concurrent runs with live state (T5).
- completion emits maestro.security.assessment and writes the same files as
  a manual run (T6).

## Size

About 500–700 production lines across T1–T7, reusing the nine shared
runners; every file under the 500-line bar. Each piece maps to the
extension contract (OD-354 + host.wit); nothing is a Pi-port item.
