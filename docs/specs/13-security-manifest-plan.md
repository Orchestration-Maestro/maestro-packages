# Security manifest delivery plan

Part of #13. This is a planning contract, not an installable manifest or
permission to start the runner build. No repository creation, resource migration,
component build or release happens in this change.

## Contents

1. [Sources and release gates](#sources-and-release-gates)
2. [Repository layout](#repository-layout)
3. [Native package and resource conventions](#native-package-and-resource-conventions)
4. [Complete security content](#complete-security-content)
5. [Delivery order and acceptance](#delivery-order-and-acceptance)

## Sources and release gates

Read [the unified design](../../crates/maestro-security/UNIFIED-DESIGN.md),
[the coverage summary](../../crates/maestro-security/DECOMPOSITION-MATRIX.md),
[the master spec](../../crates/maestro-security/SPEC.md) and
[the autonomy spec](../../crates/maestro-security/AUTONOMY.md) together. The
unified design supplies task/workflow selection and the five agent duties;
the autonomy spec supplies the run lifecycle and transport contract.
[Marketplace acceptance](../marketplace.md) supplies the package-manager,
extension and catalog boundaries. ADRs [0013](../adr/0013-one-catalog-package.md),
[0014](../adr/0014-agents-are-catalog-items.md),
[0015](../adr/0015-full-fusion.md), [0018](../adr/0018-manifests-repo.md) and
[0021](../adr/0021-user-initiated-only.md) govern delivery.

The external baselines inspected for this plan are:

| Repository | Revision | Contract files |
| --- | --- | --- |
| `maestro` | `d3f7aa6a9906d23350f4b28a88a41e0fde8195a0` | `docs/specs/manifest-mode.md`, M3, M6, M14 through M19 |
| `maestro-manifests` | `70dba4121b9b27d16e2e2b65cf1ab0da28395353` | `README.md`, `docs/catalog.md`, `docs/capabilities-and-ownership.md`, `docs/schemas-and-checks.md` |

Both repositories are under the Orchestration-Maestro GitHub organization.
These revisions identify source contracts, not approved security releases.

Issue #14 must finish its full coverage and graph/matrix review. Issue #21's
source-identity flag remains unresolved and is not proof. The coverage summary
does not authorize builds. Review must accept every
mapping and evidence disposition, then accept the complete fused design before
any security manifest is assembled or issue #15 implementation begins. This
plan does not declare those gates passed or close #13.

## Repository layout

`Orchestration-Maestro/maestro-manifests` already exists. Reuse it rather than
creating a second repository. Keep the existing core and evidence-review
capability. The target layout follows manifest-mode M3:

```text
maestro-manifests/
  .github/CODEOWNERS
  README.md
  docs/
  version.txt
  manifests/
    package.json
    CHANGELOG.md
    settings/
    core/
    capabilities/
      practice/evidence-review/
      security/maestro-security/
        package.json
        extensions/maestro-security/
        agents/
        skills/
        instructions/
        prompts/
        workflows/
```

The inspected manifests revision still keeps runtime areas at repository root.
The migration tracked in `maestro-manifests#28` must move the existing runtime
content under `manifests/`, update CODEOWNERS patterns, documentation, content
tests and release tooling (including the changelog path), and pass the manifest
consumer's root-resolution checks before security delivery. The consumer is
tracked in `maestro#1318`. This planning PR moves nothing. Do not add a fallback
reader for both roots. Repository tooling, docs, generated schemas and ownership
stay outside the manifest root. Create standards/language areas only for real
content.

Select the release through the existing `manifest` setting with a published
Git tag or full commit. Select security through
`manifestAreas: ["capabilities/security/maestro-security"]` in user/project
settings; the path is relative to `manifests/`. Common, standards and core load
first, followed by selected languages and capabilities. The security group is
navigation, not a settings area. Security remains opt-in and starts no run on
install, selection, reload, session start or background discovery.

## Native package and resource conventions

The `package.toml` and `*.maestro.toml` entity-sidecar layout is superseded.
The first draft in maestro-manifests PR #18 is closed; do not revive its
schema or copy its metadata. Use `package.json`, native artifact files and
CODEOWNERS. There is no authored entity sidecar, ownership array, qualified-ID
index, maturity field or new marketplace schema.

The planned capability package record is:

```json
{
  "name": "maestro-security",
  "private": true,
  "description": "Evidence-led security assessments using one shared runner.",
  "license": "MIT",
  "maestro": {
    "extensions": ["./extensions/maestro-security/index.wasm"],
    "skills": [
      "./skills/security-assessment",
      "./skills/security-probe",
      "./skills/security-proof",
      "./skills/security-rebuild",
      "./skills/security-cyber"
    ],
    "prompts": [
      "./prompts/recon.md",
      "./prompts/target-model.md",
      "./prompts/hypotheses.md",
      "./prompts/verify.md",
      "./prompts/deepen.md",
      "./prompts/threat.md",
      "./prompts/report.md"
    ]
  }
}
```

All paths are package-relative and must exist in the release. This in-tree
package shares the manifest's Git release version; it has no separate npm
version or install-time build. The extension directory holds the built entry
component and the tested native runner/runtime dependencies it needs. The
native runner is a package payload, not another manifest entry point or catalog
package. Issue #15 must establish the supported-host artifact set and prove
launch through the existing extension host operations before release.

Roles use `agents/<name>.md` with native `name`, `description`, `tools`, optional
`model` and `thinking` frontmatter. Skills use `skills/<name>/SKILL.md` with
native skill frontmatter. The shared instruction is
`instructions/security-assessment.instructions.md` with `applyTo` frontmatter.
Instructions are offered for explicit installation through the rules consumer;
area selection does not silently append them. Mandatory scope, evidence and
execution checks belong to the runner and cannot depend on installing guidance.

Roles, instructions and workflows use their owning consumers' conventional
folders, not invented `maestro.agents`, `maestro.instructions` or
`maestro.workflows` arrays. Omit empty settings, policies, themes, model files,
MCP configurations and a separate `knowledge/` format. Credentials, endpoints,
run logs, evidence and checkpoints stay local, outside the manifest release.

After the root migration, retain the approved principal and last-match order:

```text
* @FrancoisLDaigneault
/manifests/capabilities/security/ @FrancoisLDaigneault
/manifests/capabilities/security/maestro-security/ @FrancoisLDaigneault
```

These are the security-related CODEOWNERS rules, not a replacement for the
repository's other rules. Ownership is review attribution, not execution trust.

## Complete security content

### Seven selectable tasks

A task is a runner operation exposed as an existing prompt-template resource.
It is not a new catalog kind. Each `prompts/<task>.md` has native description
frontmatter and a body requesting that one task through the shared run surface,
with run arguments supplied using existing prompt-template expansion. It names
its retained-input requirements, output and blocked behavior. Target text and
other substituted content remain untrusted data.

| Task / prompt stem | Required retained inputs | Output responsibility |
| --- | --- | --- |
| `recon` | Authorized target and scope | Observations and execution receipts |
| `target-model` | Recon observations | Evidence-linked architecture, boundaries and gaps |
| `hypotheses` | Target model | Testable claims and coverage obligations |
| `verify` | Hypotheses and their evidence | Independent observations, counterevidence and verdict support |
| `deepen` | Verified findings and receipts | Observed impact and supported attack paths |
| `threat` | Target model and retained observations | Threat model and standard hardening |
| `report` | Retained evidence, coverage and findings | Validated immutable report and remaining gaps |

Every task can run alone against sufficient retained inputs. Missing inputs
produce a blocked result; the runner never silently executes omitted tasks.
`--tasks` selects an explicit ordered subset. Catalog cards use the existing
prompt kind; there is no `tasks/` directory, `Task` blueprint or task sidecar.

### Three workflows

Each workflow is one native `workflows/<name>.yaml` with
`apiVersion: maestro.workflow/v1`, `kind: Workflow`, matching `name`,
`description` and a `chain`. The runner owns the assessment task plan, budgets,
checkpoints and validation. A catalog workflow delegates one assessment to the
`security` role; it does not implement a second seven-step engine in YAML.

| Workflow / file stem | Ordered runner tasks | Execution breadth |
| --- | --- | --- |
| `standard` | recon, target-model, hypotheses, verify, deepen, threat, report | Default budgets; default selection |
| `deep` | recon, target-model, hypotheses, verify, deepen, threat, report | Finer coverage and additional critic waves |
| `quick` | recon, target-model, hypotheses, verify, report | Coarser coverage; omitted work remains visible |

For example, the planned `workflows/standard.yaml` uses only native members:

```yaml
apiVersion: maestro.workflow/v1
kind: Workflow
name: standard
description: Run the full seven-task security assessment with default budgets
chain:
  - agent: security
    task: "Run one authorized security assessment using standard: $@"
```

Deep and quick use the same shape with their own name and selection. The role
passes that selection to the runner's `--workflow` contract. `--workflow`
selects a named workflow; it does not change the evidence methodology or lower
the confirmation threshold. Issue #15 still says `--phases` / `--profile`;
reconcile that ticket to the fused design's `--tasks` / `--workflow` before
implementation. This plan adds no profile mode or workflow-invocation syntax.

### Five agents and their knowledge

| Native role file stem | Shared-runner role identity | Knowledge pack / duty |
| --- | --- | --- |
| `security` | `maestro.security` | `security-assessment`: coordinate the assessment and validated publication |
| `security-probe` | `maestro.probe` | `security-probe`: collect observations and preserve receipts |
| `security-proof` | `maestro.proof` | `security-proof`: challenge claims with independent checks and counterevidence |
| `security-rebuild` | `maestro.rebuild` | `security-rebuild`: reconstruct architecture from verified evidence |
| `security-cyber` | `maestro.cyber` | `security-cyber`: model threats and propose standard hardening |

These are five native Maestro roles using one engine, not five components or
copies of assessment logic. Each role prompt points to its skill pack and the
shared assessment instruction. The accepted matrix assigns every knowledge
behavior to its runner module and agent pack. Full tools, documentation and
concepts must be covered; a role description alone is not a knowledge pack.
Package assembly waits for that accepted mapping and complete resource bodies.

Role tool ceilings use the actual registered `security_*` names, not the older
MCP-qualified spelling. The shared surface retains `security_doctor`,
`security_route`, `security_assess`, `security_validate`, `security_evidence`,
`security_report_page`, `security_evidence_page`, `security_disclose` and
`security_workspace`. Check each role's needed subset against the built host;
do not grant shell access merely to bypass missing runner support.

Resolve a model once per run: explicit override, configured preferred model if
served, otherwise the first available model. Record that identity. Do not hardcode
a provider, model endpoint or key in any resource. Role preferences stay within
ordinary policy. The shared instruction and packs cover authorization, data-only
targets, deterministic admission, retained receipts, independent confirmation,
coverage gaps and validated publication. Verdicts remain Confirmed,
NeedsValidation and Rejected; Refuted is a hypothesis disposition.

### Existing entry points and catalog relations

The package manager alone acquires, pins, updates and removes the package. The
built component uses the existing extension factory, registration, event and
host-operation contract described in [the author guide](../marketplace-author-guide.md).
Session commands, CLI, MCP and RPC delegate to the same runner grammar; they
are not package entry-point fields or separate installers. Register lifecycle
controls only when implemented. Startup and discovery never assess a target.

The runner owns isolated run processes, JSON state, logs, checkpoint/resume,
scope and admission, evidence validation and immutable reports. A doctor failure
refuses execution. Failed remote/box execution never falls back to the host.
Destructive actions require authorization. Findings/results leave through the
existing event seam; the session receives only the curated timeline.

The catalog owns entity types, properties and relations. Package name/purpose
come from `package.json`; source and release come from the selected pin;
resources and native references supply the provides/uses relations; CODEOWNERS
supplies the owner; effective policy supplies required/locked state. Reuse
release, capability group, capability, package, role, skill, prompt, instruction
and workflow entities. List security once as a package, with its resources, not
once per transport, agent or task. Do not hand-maintain a second relation index.
The catalog consumer must verify rendered cards and graph edges; authored text
alone does not prove them.

## Delivery order and acceptance

1. Land this single planning spec in maestro-packages. Link #13 to this file.
   Keep #13 open and the security manifest unpublished. Obtain standards and
   spec reviews; merging a plan does not accept fusion evidence.
2. Finish #14 and resolve #21's disposition in review. Accept the full fused
   design and resource inventory. In parallel, land `maestro-manifests#28` with
   its tests and reviews. Both gates precede assembly.
3. Prepare the complete native security resource change in maestro-manifests
   from the accepted fusion, replacing the obsolete draft wholesale. Update its
   README artifact index and per-kind docs. Review the package shape against
   this plan before #15; keep the manifest PR held until every payload exists.
4. Implement #15 file by file from accepted matrix rows, using this package
   contract. Red-first slices cover task selection and missing inputs, workflow
   breadth, five role/pack bindings, doctor refusal, authorization, evidence and
   run recovery. Build/test on the AWS host only. Copy tested built artifacts
   into the held manifest change with source and host/SDK provenance.
5. Validate and review the assembled release, then publish its pinned Git
   release. Run manifests `just check` and `just test`; use generated schemas
   from the owning consumer and load the fetchable commit through
   `maestro -p "/manifest status"`. Exercise opt-in discovery, task/workflow
   selection, roles, catalog ownership/relations, disable/reload, pin change and
   removal through existing consumers. Only then record #13 completion and
   proceed to box strip-down and later integration stages in the master spec.

The inspected manifest repository has content parsers but no generated schemas
or schema-validation hook. A parser pass is not schema validation. Delivery
requires the real consumer-generated schemas and root/composition integration;
do not handwrite substitutes or invent a validator command. Record valid and
invalid artifact cases, unresolved consumers and checks not run separately.

This planning PR runs public-text, document and commit hygiene checks only.
It does not run Cargo, a component build, a schema validator, a manifest load,
a model request or a catalog UI test. Those remain delivery gates, not claimed
results of this document.
