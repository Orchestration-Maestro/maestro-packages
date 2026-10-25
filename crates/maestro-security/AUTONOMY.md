# maestro-security autonomy — spec (issue #7)

## Owner decisions (grill, 2026-10-25)

1. Trigger: a command only. Nothing starts an assessment from message
   parsing or from the web UI.
2. Run style: a background run with live updates and a visible status;
   bidirectional control from the session chat — kill and steer, exactly
   like subagent workflows; modular; works in the TUI.
3. Scope: box tools plus evidence/report files in MAESTRO_SECURITY_HOME.
   No workspace writes.
4. Results: the same files and report page as a manual run, plus a
   notification, and the result event is emitted back to Maestro so larger
   workflows can consume it. Always bind to Maestro hooks and events.

## Design

### Trigger

`/assess <target>` registers a command (host.wit `register-command`). It
starts a background run and returns the run id at once.

### Live updates

- Progress events on the bus (`event-emit` on `maestro.security.run.<id>`),
  one event per phase boundary and per finding.
- Status-line and working-message updates through the UI context (TUI and
  browser neutral); no browser-only live surface.

### Bidirectional control

- `/security-stop <run_id>` aborts the run through its abort signal.
- `/security-steer <run_id> <message>` delivers steering input to the
  running phase; the run reads it at phase boundaries.

### Completion

Validate gate, then emit the result event (`maestro.security.assessment`
carrying the report JSON), write evidence and report files exactly like a
manual run, show the report page artifact, and notify with the findings
count.

### Modularity

One module per concern: trigger (commands), run (background task), progress
(emitter), control (stop/steer), completion (validate + emit). Reuses
tools.rs runners, router.rs, validate.rs, store.rs, doctor.rs. No new
backend, no polling; WASI 0.3 async for the background task.

## Non-goals

- No trigger from message parsing or the web UI.
- No workspace writes beyond the report files.

## Tests (red-first)

- `/assess` returns a run id and starts a run.
- Progress events fire in phase order.
- `/security-stop` aborts a mid-phase run; no result event after a kill.
- `/security-steer` reaches the running phase.
- Completion emits the result event and writes the same files as a manual
  run.

## Size

About 150–200 production lines reusing the nine shared runners. Each piece
maps to the extension contract (OD-354 + host.wit); nothing is a Pi-port
item.
