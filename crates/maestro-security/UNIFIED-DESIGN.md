# maestro-security: unified design

This is a design contract, not a claim that the runner is implemented.
The build order is decomposition, fusion, catalog, runner, then box integration.
Later stages wait for their review gates.

## Architecture

One runner library owns the assessment engine, contracts, state and capabilities.
The native runner executes the work; the extension supplies the session adapter.
Session commands, CLI, MCP and RPC delegate to the same grammar and core logic.
The box contains raw tools, analysis engines and models, not another orchestrator.

The data path is:

```text
explicit request -> transport adapter -> parsed run contract
  -> isolated runner -> selected tasks -> action proposals and admission gates
  -> box execution -> retained receipts -> coverage and findings
  -> validated report -> events -> curated session timeline
```

State uses the existing JSON stores. Each schema, parser, state transition and
validation rule has one owner. Transports do not duplicate assessment logic.
Findings, evidence, coverage and run state have distinct typed records.

## Tasks and workflows

The seven tasks, in canonical order, are recon, target-model, hypotheses, verify,
deepen, threat and report. Each task can run alone. A workflow chains tasks in
one file; `--tasks` selects an explicit ordered subset and `--workflow` selects
a named workflow. Missing retained inputs produce a blocked result rather than
silently running omitted tasks.

The standard workflow uses all seven tasks with default budgets. Deep keeps the
same chain with finer coverage and additional critic waves. Quick selects recon,
target-model, hypotheses, verify and report with coarser coverage. All use the
same evidence and validation rules. Selection changes execution breadth, not the
methodology or confirmation threshold.

## Agents and model selection

Five catalog agents share the engine: security coordinates, probe gathers
observations, proof challenges findings, rebuild reconstructs architecture and
cyber models threats. Their instructions and knowledge are catalog resources,
not copies of engine logic.

Resolve the model once per run: an explicit override wins, then the configured
preferred model if served, otherwise the first available model. Record the
resolved identity. Credentials come from runtime settings, never embedded keys.

## Run lifecycle

A run has an immutable identity, target and selected task plan. Its states are
queued, running, completed, killed, failed and paused. The planned isolated
process owns its log and checkpoints; the session shows only phase transitions
and findings. Evidence survives failure or kill, and recovery resumes from the
last completed checkpoint.

Steering changes priorities and guidance, never evidence or verdicts. Planned
pause, resume and stop controls act through the run machinery. Until that
machinery exists, inactive controls are not registered or advertised.

## Evidence and execution contracts

Target text, diagnostics, fetched documents and steering text are untrusted data.
Scope and admission checks remain deterministic. Destructive operations and
workspace changes are proposals requiring separate authorization; execution is
restricted to authorized box-owned scratch space.

A receipt retains command, output, source identity and exact quoted bytes.
Views keep their parent identity. Failed execution remains diagnostic evidence,
not proof of success. Confirmation requires independent observations; a script
printing a success label cannot confirm itself. Coverage records owned attempts,
refutations and gaps without turning unknowns into successful results.

Finding verdicts are Confirmed, NeedsValidation and Rejected. Refuted describes
a hypothesis disposition, not a finding verdict. Publication validates verdict
shape, evidence lineage, scope, coverage and severity against observed impact.
Reports are immutable projections of retained records.

Failed container or remote execution never falls back to host execution.
Timeouts preserve partial output and distinguish causes. Shared HTTP retries use
bounded attempts; non-idempotent actions are not blindly replayed. Event overflow
must be recorded and recovered by cursor rather than silently losing findings.

## Coverage status

The design maps 2,168 qualified entries, including 42 correction overlays.
Graph accounting records 798 two-sided witnesses and 1,913 missing-side
exclusions. One source-identity flag remains open in issue #21 and is not
accepted as proof. These counts describe evidence records, not implemented
features; they do not authorize the next build stage.

See [the master spec](SPEC.md) for program order and
[the autonomy spec](AUTONOMY.md) for lifecycle and transport requirements.
