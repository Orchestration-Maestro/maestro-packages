# maestro-packages

The context for the maestro-packages repository: extension packages for
Maestro, governed like the maestro repo. The language below is the domain
model for maestro-security; the grill settles terms here, the spec reads
from here. Implementation detail never lives in this file.

## Language

**Assessment** (or **run**):
One autonomous security assessment of one target, driven by the seven-phase
pipeline. "Run" names the live, steerable execution of an assessment.
_Avoid_: Scan, audit, mission

**Run id**:
The unique identifier of one run; never reused, never guessed.
_Avoid_: Job id, task id

**Phase**:
One of the seven pipeline stages: recon, target-model, hypotheses, verify,
deepen, threat, report.
_Avoid_: Stage, step

**Finding**:
A vulnerability claim carrying a verdict.
_Avoid_: Vuln, issue, bug

**Verdict**:
One of Confirmed, NeedsValidation, Rejected.
_Avoid_: Severity, rating

**Refuted**:
A hypothesis disproven with counterevidence; an evidence disposition, never
a verdict.
_Avoid_: Rejected (for hypotheses), dismissed

**Steering**:
A message sent into a live run: free text, or the verbs stop, pause,
resume, focus <fingerprint>.
_Avoid_: Prompt, instruction, control message

**Kill**:
Aborting a run: evidence kept, resumable from the last checkpoint, no
result event.
_Avoid_: Cancel, stop (reserved as the steering verb)

**Pause**:
Holding a run at the next phase boundary: checkpointed, still visible,
resumed by the resume verb. Distinct from kill, which ends the run.
_Avoid_: Suspend, freeze

**Checkpoint**:
The durable per-run state written after each phase.
_Avoid_: Snapshot, save state

**Run board**:
The live view over all runs: id, target, state, phase, elapsed, findings.
_Avoid_: Dashboard, task list

**Box**:
The Kali image carrying raw tools, skills and models — the pipeline's
hands, never the brain.
_Avoid_: Sandbox, VM, container

**Run entry**:
The curated timeline of one run in the session: phase transitions and
findings only; the run itself lives in its own isolated process, never in
the session's context.
_Avoid_: Run thread, run panel

**Run log**:
The per-run log the run writes for itself: events, phase prompts, tool
outputs, spend — readable through the command surface, not the transcript.
_Avoid_: Transcript, console output

**Command surface**:
The one grammar `/maestro-security <verb>` (assess, stop, steer, list,
board, logs, report) exposed on all four Maestro transports: session
commands, CLI, MCP, RPC.
_Avoid_: CLI, tool, endpoint (each names one transport, not the surface)

**Campaign**:
A queued sequence of assessments worked through one after another; the
mission is the core, campaigns compose missions.
_Avoid_: Batch, queue

**Event seam**:
The shared-bus events a run's life emits; the only route findings and
results leave a run — larger workflows consume them as events, nothing
queries a run's files directly.
_Avoid_: API, hook, callback

## Decisions

Decisions live in `docs/adr/`. Settled so far: ADR-0001 (the trigger),
ADR-0002 (the run as a session citizen).
