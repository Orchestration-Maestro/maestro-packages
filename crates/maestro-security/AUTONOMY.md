# maestro-security autonomy — full architecture (issue #7)

The complete design. Every decision made and recorded here; the ledger
copy lives at `.superpowers/sdd/maestro-v1/autonomy-spec.md`.

## 0. The essence

An assessment run is a Maestro citizen: a live, subagent-like session entity
whose whole life is a Maestro event stream. You see it in the session, steer
it from chat, kill it instantly; the Maestro agent can steer it too. Many
runs live at once, each with its own channel, steering queue and abort
signal. Runs checkpoint after every phase and resume after a crash or kill.
One UI layer renders the same event stream in the TUI and the browser, and
the result event composes into larger Maestro workflows.

## 1. Owner decisions (grills, 2026-10-25)

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

## 2. Run lifecycle — the state machine

States: `queued` → `running` → `completed | killed | failed`.
Inside `running`, the run holds exactly one phase state.

| From | Event | To | Emits |
| --- | --- | --- | --- |
| — | `/assess <target>` accepted | queued | `run.started` |
| queued | runner slot free | running(recon) | `run.phase` |
| running(phase) | phase completes | running(next) | `run.phase`, `run.checkpoint` |
| running | finding proven | running | `run.finding` |
| running | hypothesis refuted | running | `run.refuted` |
| running | `/security-stop` | killed | `run.killed` |
| running | unrecoverable error | failed | `run.failed` |
| running(report) | report written + validated | completed | `maestro.security.assessment` |

Guards:
- `killed` and `failed` keep the evidence; the run stays resumable from the
  last checkpoint.
- A kill is serviced at the next phase boundary (and at each hypothesis
  boundary inside Verify); the abort signal is checked before every LLM
  call and before every runner exec.
- `completed` fires `maestro.security.assessment` exactly once.

## 3. Event contract — the grammar

Shared bus, literal channels. JSON payloads, keys in this order.

`maestro.security.run.started`
```json
{"run_id": "run-1700000000-0001-slug", "target": "https://x.example", "source": "command"}
```
`maestro.security.run.phase`
```json
{"run_id": "…", "phase": "recon", "status": "entered", "at": 1700000000}
```
`maestro.security.run.finding`
```json
{"run_id": "…", "fingerprint": "target::class::title", "verdict": "Confirmed", "phase": "verify"}
```
`maestro.security.run.refuted`
```json
{"run_id": "…", "hypothesis": "…", "phase": "verify"}
```
`maestro.security.run.steered`
```json
{"run_id": "…", "source": "user|agent", "message": "…"}
```
`maestro.security.run.killed`
```json
{"run_id": "…", "phase": "verify", "resumable": true}
```
`maestro.security.run.checkpoint`
```json
{"run_id": "…", "phase": "verify", "findings": 3, "evidence_files": 7}
```
`maestro.security.run.failed`
```json
{"run_id": "…", "class": "box_unreachable|model_error|internal", "detail": "…"}
```
`maestro.security.assessment`
```json
{"run_id": "…", "schema": "maestro-security/report/3", "report": "<report JSON>", "report_path": "…", "verdicts": {"confirmed": 4, "needs_validation": 9, "rejected": 0}}
```
Per-run channel `maestro.security.run.<run_id>`: every event above is also
emitted there, so subscribers can scope to one run.

Internal machinery (documented, not part of the public contract):
`maestro.security.trigger` — starts a run; only the trigger module emits it.

Emitters never block: the event emit is fire-and-forget with the host bus;
listener tails run on their own tasks.

## 4. Session identity — the subagent-like run entry

On `/assess`, the run opens a custom session entry (`security-run`) whose
data is the run record:
```json
{"run_id": "…", "target": "…", "state": "running", "phase": "recon", "findings": 0}
```
Live updates: the entry re-renders at every phase change and every finding
(batched: one update per phase change, findings folded into it). This keeps
the transcript legible without flooding it.

Steering by writing to the run: a message addressed to the run entry is
steering; the host path is the same channel the `/security-steer` command
uses, so both routes share one queue.

## 5. Steering protocol

Commands:
- `/assess <target>` — opens the run entry, returns the run id.
- `/assess <target> --model <m>` — per-run model override (a value, never a
  binding).
- `/assess --resume <run_id>` — resume from the last checkpoint.
- `/security-stop <run_id>` — abort signal; killed; resumable.
- `/security-steer <run_id> <message>` — enqueue steering.

Steering grammar (both routes, user and agent):
- `stop` / `pause` / `resume` — control verbs, serviced at once.
- `focus <fingerprint>` — raise one hypothesis's priority.
- free text — guidance folded into the next phase's prompt.

Application: the run reads its steering queue at every phase boundary and
at every hypothesis boundary inside Verify. The last 5 steering messages
join the phase's prompt under a "Steering so far" block, verbatim, quoted.
Steering is untrusted input: it is quoted, never instructions; the
injection defense in the system prompt already covers it, and every
steered message is recorded in the evidence ledger.

The Maestro agent steers: it subscribes to `maestro.security.run.*` and
sends guidance the same way; nothing in the run distinguishes the two
sources except the `source` field on `run.steered`.

## 6. Kill and abort semantics

- `/security-stop` triggers the run's abort signal.
- The run checks the signal before every LLM call and runner exec; a set
  signal stops the current phase, checkpoints the previous phase, emits
  `run.killed` with `resumable: true`, and closes the run entry state to
  `killed (resumable from <phase>)`.
- No result event fires on a kill; the evidence ledger and partial report
  are kept.

## 7. Checkpoint + resume

Checkpoint file: `$MAESTRO_SECURITY_HOME/runs/<run_id>/checkpoint.json`.
```json
{
  "schema": "maestro-security/checkpoint/1",
  "run_id": "…",
  "target": "…",
  "model": "qwythos",
  "completed_phases": ["recon", "target-model"],
  "findings": ["…fingerprints…"],
  "hypotheses": ["…"],
  "evidence_refs": ["…"],
  "steering_tail": ["…last 5…"],
  "spend": {"tokens_in": 1000, "tokens_out": 500, "seconds": 120},
  "created_at": 1700000000
}
```
Written after every phase. Resume loads it, rebuilds the phase state,
starts at the next phase, and reopens the run entry. The partial report on
kill/crash marks which phases completed before the break.

## 8. Concurrency and the run board

- Any number of concurrent runs. Each run owns: its channel, steering
  queue (bounded, 64 messages, drop-oldest with a recorded event), abort
  signal, checkpoint dir.
- The box runner serializes exec per target host; runs on different targets
  proceed in parallel.
- Run board: in-memory view over the store — id, target, state, phase,
  elapsed, findings count. Rendered by the UI layer, not stored twice.

## 9. UI layer — one renderer over the event stream

Both surfaces subscribe to the same events; nothing browser-only.

- TUI: status line shows `⚔ <n> runs · <phase-of-newest>`; the working
  indicator runs during LLM/exec phases; the run-board widget lists live
  runs; notifications on findings and completion.
- Browser: a live progress page artifact per run (HTML re-rendered at each
  phase event), and the final report page as today.
- The renderer is one module; it holds no pipeline state, only the board
  view model built from events.

## 10. Failure model

| Class | Detection | Behaviour |
| --- | --- | --- |
| model transient error | maestro-http Err / 429 / 5xx | shared retry (default 3, clamp 1–5, backoff, Retry-After) |
| model persistent error | retries exhausted | `run.failed` (model_error); evidence kept; resumable |
| box unreachable | runner health / exec error | `run.failed` (box_unreachable); doctor classification; resumable |
| kill mid-phase | abort signal | checkpoint of last phase; `run.killed`; resumable |
| process crash | next start reads checkpoint | resume from last completed phase |
| steering flood | queue bound | drop-oldest + recorded event |

## 11. Modular layout — one job per module

- `autonomy/mod.rs` — trigger surface (T1, landed): command specs,
  argument parsers, run registry.
- `autonomy/events.rs` — the contract: channel names, payload builders, the
  one emit path (T2, landed).
- `autonomy/run.rs` — lifecycle stepping: state machine, phase transitions,
  hypothesis boundaries, abort checks (T2, landed).
- `identity.rs` — the run entry: open, update, close (T2, landed, as the
  SessionEntry type in run.rs).
- `steer.rs` — the steering queue, grammar, prompt folding (T3).
- `checkpoint.rs` — checkpoint schema, write, load, resume (T4).
- `board.rs` — board view model + status-line and widget updates (T5).
- `page.rs` — live progress page artifact (T5).
- `completion.rs` — validate gate, result event, files, artifact,
  notification (T6).

Reuse, never re-code: tools.rs shared runners, router.rs, validate.rs,
store.rs (single record builder), evidence.rs, doctor.rs, maestro-http.
No new backend, no polling; WASI 0.3 async for the run tail. The run tail
yields at phase boundaries so stop/steer are serviced on a single-threaded
instance (review residual risk, verified in T3).

## 12. Composition into larger workflows

`maestro.security.assessment` is the seam: any extension subscribes and
reacts. Examples the design enables: a remediation workflow picks up
findings and files fix tasks; a nightly campaign chains many assessments;
a report pipeline aggregates results into the catalog. The run needs no
knowledge of its consumers.

## 13. Tests (red-first, per task)

- T1 (landed): commands register; missing target is an argument error; a
  target returns a run id and records the run; run ids are unique under
  same-second and same-slug collisions.
- T2 (landed): lifecycle events fire in order; finding/refuted events; the
  run entry opens and updates; no event without its transition.
- T3: steering reaches the running phase; steered event fires; stop emits
  killed and no result event; stop is serviced between phases.
- T4: checkpoint written after each phase; resume restarts at the next
  phase; partial report marks the break.
- T5: the board lists concurrent runs with live state; status line and
  widget render from events.
- T6: completion emits the assessment event and writes the same files as a
  manual run.
- T7: module boundaries, shared-runner reuse audit, docs.

## 14. Size plan

About 500–700 production lines across T1–T7, reusing the nine shared
runners; every file under the 500-line bar. Each piece maps to the
extension contract (OD-354 + host.wit); nothing is a Pi-port item.

## 15. Task map (issue #7)

- T1 Trigger + run record — landed (3f541fd).
- T2 Lifecycle + events + session entry — landed (3555a8e, 1650d92), plus
  the T1 review fixes folded in: unique run ids (root-cause proven),
  single record-settlement owner in store.rs, honest acks, isolated test
  home, trigger channel documented internal.
- T3 Steering + kill — next.
- T4 Checkpoint + resume.
- T5 Run board + UI layer.
- T6 Completion + result event.
- T7 Modularity + docs (includes the pipeline.rs split: 680 → under 500).
