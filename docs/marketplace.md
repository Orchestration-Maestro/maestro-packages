# Maestro marketplace

## What you can build here

Anyone can author a Maestro extension, plugin or resource package. Start with
[the author guide](marketplace-author-guide.md); you do not need to know how
Maestro is implemented or contribute to its core.

An extension or plugin is a WebAssembly component using Maestro's existing
extension contract. These names do not select different runtimes. A package is
the distribution that carries components, resources or both. Keep one crate per
job; combine existing packages rather than copying their logic.

| Build | Use the existing contract |
| --- | --- |
| Tools, commands, providers, prompt changes and session behavior | Component factories, registration and event hooks |
| Dialogs, widgets and custom rendering | Extension UI capabilities, with a headless fallback |
| Skills, prompt templates, themes and browser contributions | Native resource files selected by `package.json` |
| Organization capabilities with roles, instructions and workflows | Existing manifest artifacts and their owning consumers |

Content-only packages need no Rust crate. Code packages ship built components;
users never compile them during installation. A browser contribution is not a
second extension runtime. See [resource packages][package-guide] and
[manifest artifacts][manifest-artifacts] for each kind's supported consumer.

This document specifies marketplace acceptance and catalog integration. It
does not deliver a marketplace service. Native package installation is
available. The [browser catalog][catalog] has no package installer today; its
metadata, trust display and install controls below are requirements for that
consumer. This design introduces no channel, API, protocol or file format.

## Publishing a package

1. Choose one job and build with the [extension contract](#extension-contract)
   or existing resource formats. Keep source, license, dependency lockfile and
   the tested Maestro/SDK revision available to reviewers.
2. Assemble an ordinary package. Its `package.json` selects resources in the
   existing `maestro` lists: `extensions`, `skills`, `prompts`, `themes` or
   `contributions`, as applicable. Include built `.wasm` files and runtime data;
   inspect `npm pack --dry-run`. Do not require an install-time Rust build.
3. Try the directory through `maestro -e ./your-package`. Test its behavior,
   failure cases and headless use before publishing through npm or a Git
   release. Git releases must include the artifacts, not just source. The
   [author guide](marketplace-author-guide.md) gives the complete local recipe.
4. To submit here, open an issue in `maestro-packages` with the `packages` label
   and an opening `Repository: maestro-packages` line. Submit a pull request
   linking that issue, the exact published source/version or commit, source and
   artifact locations, catalog metadata inputs and test results. Publishing to
   npm or Git alone does not mean Maestro has reviewed the package.
5. Maintainers apply the [review checklist](#review-checklist) before accepting
   the release. Resolve blocking findings before merging. Later releases need
   review of their changes; acceptance of one release does not cover a moving
   branch, a new dependency set or a different distribution.

### Catalog metadata has one owner

Every marketplace package must declare its catalog metadata through the
catalog's existing **package entity model**. The catalog owns the schema and
relations. This repository must not define a marketplace schema, an entity
sidecar file or a second package index.

Use the [catalog blueprints and relations][catalog-model] and
[ownership rules][catalog-ownership]. Authors supply existing package and
repository inputs; the catalog consumer maps those inputs to its entity model:

| Existing input | Catalog meaning |
| --- | --- |
| `package.json` name and description | Package name and purpose |
| Selected source and exact release | Package identity, provenance and release |
| Native resource entries and references | Provided artifacts and their relations |
| `.github/CODEOWNERS` at that release | Owner, using the last matching rule; otherwise unowned |
| Effective manifest policy and installed contents | Required, locked and locally modified state |

Identify these inputs in your submission so reviewers can check the package
entity and resource relations. A package used by a manifest is the
same package entity, not a duplicate listing. The Web UI catalog must list every
accepted package and correlate it with its release, owner, provided resources
and the entries that require it, using those existing relations.

The catalog contract describes these properties and relations, but does not yet
supply an author-facing serialized package-entity declaration or editor. Use
`package.json`, CODEOWNERS and native artifact references as today's authored
inputs. Do not invent JSON fields or a metadata command to fill that gap;
rendered cards and graph edges need validation in the catalog consumer.

## Trust model and third-party review

Installing code is a trust decision before the first callback runs. Git
preparation and npm installation can execute dependency lifecycle scripts.
Skills and prompts can influence model tool use even without a component.

### Runtime access and limits

Maestro extensions run with the user's access, **not in an untrusted-code
sandbox**. The [host operating environment][host-io] and
[guest operations][guest-operations] define the actual access:

- Components inherit environment variables, including secrets present there,
  and separate standard streams. Reading stdin or writing stdout can interfere
  with a protocol frontend.
- They can use the user's network without an added address filter, read and
  write through preopened filesystem roots, and launch host processes. Unix
  exposes `/`; Windows exposes accessible drives and the current share, with
  the platform limits documented by the host.
- Registered callbacks can observe and change prompts, handle tool calls, query
  session records and use supplied model/credential capabilities. Authors must
  document data sent to external services and any persistent writes.
- A component cannot import arbitrary host functions outside the supplied WIT
  world or gain operating-system privileges the user lacks. Host operations
  can reject a call; ordinary event contexts do not expose command-only session
  replacement operations. Filesystem and runtime limitations still apply.

Package filters select resources; they do not restrict the permissions of code
that is loaded. A manifest lock governs supported Maestro operations, not every
possible filesystem or process effect of malicious code. Review, pinning and
operating-system controls remain necessary. This design adds no permission
manifest, per-package sandbox or automatic security guarantee.

### Review checklist

A release is accepted only after maintainers check both behavior and repository
standards. The pull request records the results and unresolved limits:

- [ ] Source, license, publisher and CODEOWNERS attribution are inspectable;
  the proposed source pin identifies the reviewed release and built artifacts.
- [ ] Package contents match `package.json`; the component targets the existing
  WIT world and tested host/SDK revision. Dependencies, lifecycle scripts and
  distributed artifacts have been inspected for unexpected code and secrets.
- [ ] Tests cover the advertised behavior, failures and headless operation.
  Installation, discovery, enable/disable, update or pin change, and removal
  use the package manager. Evidence distinguishes executed tests from checks
  that were not run.
- [ ] Review accounts for filesystem, network, process and credential access,
  data disclosure, persistent writes and cleanup. Instructions and browser
  resources receive review too; a component build alone is insufficient.
- [ ] Catalog inputs resolve to the package entity, owner and resource
  relations without a second schema. In a catalog-enabled build, cards and
  relations match the release, and UI install/uninstall use the shared path.

### What the user sees

Trust means an accepted review of an identified release, not a claim that its
code is harmless. The catalog must show the source, release and owner alongside
links to that release's accepted pull-request review and validation evidence.
An author-supplied description or an ownership match is not review evidence.
Use existing catalog properties and links; do not add a marketplace trust field
or certificate format.

A release without matching review evidence must be presented as unreviewed.
A changed pin, a different source or locally modified contents cannot inherit
the reviewed release's trust claim. Locked or required describes organization
policy, not security review. If a defect is reported, maintainers record it in
the issue/review and correct or withdraw the recommendation; pin changes still
use ordinary package operations rather than a separate revocation service.

Today's `maestro list` shows configured sources and paths, not a trust verdict.
Until the catalog consumer presents review evidence, users inspect the linked
release pull request themselves. Installation does not perform marketplace
review or establish artifact authenticity automatically.

## Versioning and pinning

Give independently published packages a version and release notes. Use semantic
versioning: incompatible author-visible behavior changes the major version,
compatible additions the minor version, and fixes the patch version. Record
the tested Maestro revision alongside the SDK revision; the WIT package version
alone is not a promise that every host revision is compatible. In-tree manifest
capabilities can share their containing release's version.

The [package source grammar][sources] is the only pin format:

| Source | Pin example | Meaning |
| --- | --- | --- |
| npm | `npm:@your-team/maestro-hello@0.1.0` | Exact published package version |
| Git | `git:github.com/your-team/maestro-hello@<full-commit>` | Exact commit at that repository |
| Local directory | `./hello-extension` | Live files; no package-manager version pin |

The remote addresses above illustrate syntax; use a reviewed source you own or
can access. Git tags and branches can move. npm tags and ranges are explicit
pins for update selection, but are not reproducible exact versions. Prefer full
Git commits or exact npm versions when adopting a reviewed release.

[Update checks and execution][updates] skip local sources and every explicit
npm suffix or Git ref. Use `maestro update --extensions` for unpinned package
updates. To change a pin or roll back, install the selected exact version again;
the first matching registration keeps its other filter fields. Project
registrations override user registrations with the same source identity.
Installing via both npm and Git creates distinct identities and can load twice.

Ordinary installation is not an integrity check. The package manager's existing
[Git pin verification and restoration][pin-integrity] serve callers that need
those operations. Restoration and changed managed Git updates can discard edits
and untracked files. Develop against a local source instead of editing a managed
checkout. This design adds no lockfile or signature protocol.

## Installation and enabling

Use the native package manager to install a reviewed source. From the project
where you want it, choose **one** source form:

```sh
maestro install npm:@your-team/maestro-hello@0.1.0 --local
maestro install 'git:github.com/your-team/maestro-hello@<full-commit>' --local
maestro install ./hello-extension --local
```

Replace the remote examples before running them. Omit `--local` for user scope.
`maestro list` shows the registration and installed path. Local installation
records a path without copying files. Start Maestro again, or use `/reload` in
an interactive session after changing installed artifacts or filters.

Installation normally selects the package's resources; there is no separate
marketplace enable flag. Existing per-kind package filters control loading.
For example, `extensions: []` on an object-form `packages` entry disables that
package's components while leaving omitted resource kinds selected. To enable
its components again, remove that filter. Consult [resource selection][filters]
for the complete settings example. Edit the owning scope's settings to apply
these filters.
Themes still need selection and instructions still need their existing install
operation; package acquisition does not activate every artifact kind.

Remove the registration from the same scope with
`maestro remove <source> --local` (`uninstall` is an alias). Managed npm/Git
contents are removed; local source files remain. Removal does not reverse
external effects or erase data an extension wrote elsewhere. Required packages
and locked settings remain subject to manifest policy.

### One install path for the UI and manifest

The UI calls the existing package manager with the source, pin and scope:

1. UI install uses `PackageManager::install` and UI uninstall uses
   `PackageManager::remove`. For optional configured packages, the existing
   `install_and_persist` and `remove_and_persist` operations also update their
   settings registration, as the CLI does.
2. Manifest acquisition uses the same `install` operation for its selected
   source/scope. It keeps the manifest selection in its existing setting rather
   than creating an extra optional `packages` entry. Both callers retain the
   existing policy checks, progress, error and persistence behavior.
3. Updates use `PackageManager::update`; enabling uses existing resource filters
   through their settings owner. The UI must refuse removals or edits forbidden
   by manifest policy and refresh its catalog view from the resulting state.

These are catalog-consumer requirements, not currently available browser
buttons. They reuse [package acquisition and persistence][acquisition] and
[manifest acquisition][manifest-acquisition]. A UI must not copy components into
its own directory, run a different downloader, or report an operation successful
when the package manager or settings owner failed.

### Extension contract

Marketplace packages use the same contract as the [guest guide][guest]. These
are the entry points for implementation and review, not new marketplace APIs:

| Contract owner | What it defines |
| --- | --- |
| [Hooks][hooks] and [guest events][guest-events] | Lifecycle events, payloads and decisions |
| [Shared event bus][event-bus] | Existing extension-to-extension delivery and subscriptions |
| [Registration][registration] | Factories, tools, commands, flags, shortcuts and renderers |
| [Guest operations][guest-operations] | Host-backed tools, waits, processes, HTTP and watches |
| [Host IO][host-io] | Environment, filesystem, streams and network access |
| [Canonical WIT world][wit-world] | `maestro:extension@0.1.0`, world `extension`, importing `host` and exporting `guest` |

The [package specification][package-spec], [sources][sources] and
[updates][updates] own distribution and maintenance. Marketplace documentation
adds neither a second extension contract nor a second installation mechanism.

[package-guide]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-guide.md
[manifest-artifacts]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/specs/manifest-mode.md#m14--authored-artifacts
[catalog]: https://github.com/Orchestration-Maestro/maestro-manifests/blob/main/docs/catalog.md
[catalog-model]: https://github.com/Orchestration-Maestro/maestro-manifests/blob/main/docs/catalog.md#blueprints-and-relations
[catalog-ownership]: https://github.com/Orchestration-Maestro/maestro-manifests/blob/main/docs/capabilities-and-ownership.md#ownership
[guest]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md
[guest-events]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md#events-and-results
[hooks]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/hooks.md
[event-bus]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/event_bus.md
[registration]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/registration.md
[guest-operations]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest_operations.md
[host-io]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/host_io.md
[wit-world]: https://github.com/Orchestration-Maestro/maestro/blob/main/crates/maestro-extensions-wasm/wit/world.wit
[sources]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-sources.md
[updates]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-updates.md
[pin-integrity]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/packages.md#resolved-packages-and-pin-integrity
[filters]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-guide.md#select-resources
[acquisition]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/packages.md
[manifest-acquisition]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/specs/manifest-mode.md#m2--delivery-and-acquisition
[package-spec]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/specs/packages.md
