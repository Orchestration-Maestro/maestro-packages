# Write your first Maestro package

Start with an empty directory and install an extension that adds a `hello` tool.
You will build one Rust crate, package its component and register it with
Maestro's package manager. Consumers of a published package need only the built
files, not Rust or knowledge of Maestro's internals.

Extensions and plugins use the same component contract. A package can also
contain only skills, prompts, themes or other supported resources. See
[what you can build](marketplace.md#what-you-can-build-here) and the
[trust model](marketplace.md#trust-model-and-third-party-review) before loading
someone else's package. Components run with your user access.

## The extension contract

A **host** is the Maestro application running your extension. A **guest** is your
component. Its factory receives `ExtensionAPI`, registers callbacks and returns
a result. A callback is a function Maestro calls when its registered tool,
command or event is used. You do not write a server or connect a new protocol.

The [guest guide][guest] is the contract for this guide too:

| Need | Use | Read |
| --- | --- | --- |
| Add a tool, command, shortcut, flag or renderer | `register_tool`, `register_command` and the other registration methods | [Guest registration][guest-register], [registration rules][registration] |
| React to prompts, tools or session changes | `api.on(event_name, handler)` and typed event results | [Guest events][guest-events], [hooks][hooks] |
| Communicate between extensions | `api.events()`, `on`, `emit` and explicit `unsubscribe()` | [Guest event channels][guest-channels], [event bus][event-bus] |
| Use host-backed tools, processes, waits, HTTP or watches | Existing factories and `api.native_io()` | [Guest operations][guest-operations] |
| Access environment, files, network and byte streams | The supplied operating-system interfaces, with the documented platform limits | [Host IO][host-io] |

The canonical [WIT world][wit-world] is `maestro:extension@0.1.0`, world
`extension`. WIT describes the functions and resources exchanged across the
component boundary. The world imports `host` and exports `guest`.
`maestro_extensions_wasm::export_extension!(factory)` supplies your guest entry
point; do not copy WIT or maintain your own bindings for this Rust example.
Rust's build target is `wasm32-wasip2`; the SDK adds Maestro's native asynchronous
component bindings. An arbitrary WASI command is not a Maestro extension.

## Build and install the greeting extension

Use a POSIX shell with Git, rustup, npm and a native `maestro` executable on
`PATH`. Use a host built from the same revision as the SDK below, with component
loading enabled. This recipe pins a published source revision rather than a
moving branch. For another host revision, use its matching SDK and authoring
guide. Host installation is covered by [your first session][first-session].

A model and credentials are needed only for the interactive greeting, which
can incur provider usage. The commands below create a new workspace; do not run
them over an existing project.

### 1. Create the project

```sh
mkdir maestro-author-workshop
cd maestro-author-workshop
git clone --no-checkout https://github.com/Orchestration-Maestro/maestro.git maestro
git -C maestro checkout --detach 58787fdc77242aaac2f74c7b2ca6e3a2748d904d
mkdir -p hello-extension/src
cd hello-extension
cp ../maestro/rust-toolchain.toml .
cp ../maestro/examples/extensions/hello.rs src/lib.rs
cp ../maestro/LICENSE .
cat > Cargo.toml <<'EOF'
[package]
name = "hello-extension"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
maestro-extensions-wasm = { path = "../maestro/crates/maestro-extensions-wasm" }
serde = { version = "=1.0.229", features = ["derive"] }
serde_json = { version = "=1.0.151", features = ["float_roundtrip"] }

[workspace]
EOF
```

The recipe copies the maintained [shipped greeting example][hello-source]
unchanged. The standalone manifest follows
[component authoring][component-authoring]. The empty `[workspace]` keeps this
crate independent of any parent Cargo workspace.

The source's `factory` calls `api.register_tool(hello_tool())`. Its tool schema
requires a string named `name`; execution returns `Hello, <name>!` and structured
`greeted` details. The `wasm32`-guarded export macro names that factory. There is
no `main` function and no slash command named `/hello`.

### 2. Build and assemble the distribution

```sh
rustup target add wasm32-wasip2
cargo generate-lockfile
cargo build --locked --target wasm32-wasip2 --release
mkdir -p dist
cp target/wasm32-wasip2/release/hello_extension.wasm dist/index.wasm
cat > package.json <<'EOF'
{
  "name": "maestro-hello-workshop",
  "version": "0.1.0",
  "description": "A Maestro tool that greets a supplied name",
  "license": "MIT",
  "files": ["dist/index.wasm", "LICENSE"],
  "maestro": {
    "extensions": ["dist/index.wasm"]
  }
}
EOF
npm pack --dry-run
```

The hyphen in the crate name becomes an underscore in the build output.
`dist/index.wasm` is the built component. Consumers load it without a separate
component-conversion step. The SDK dependency is linked into it.
Keep `Cargo.lock` with your source and rebuild intentionally after dependency
changes. The consumer does not need the adjacent SDK checkout.

`package.json` is the existing distribution manifest, not a marketplace format.
Its `maestro.extensions` paths are relative to the package root. Check the pack
listing includes `package.json`, `dist/index.wasm` and the license. Runtime data
must be included too if your extension needs it. The greeting needs none.

### 3. Install into a separate project

From `hello-extension`, run:

```sh
cd ..
mkdir greeting-project
cd greeting-project
maestro install ../hello-extension --local
maestro list
maestro
```

The installer records the local source in this project's
`.maestro/settings.json`. It calculates the stored path relative to that settings
file, so do not replace it with the input spelling by hand. Local installation
does not copy your files. `maestro list` must show the source and its existing
path; that listing proves registration, not successful component loading.

In the interactive session, ask:

```text
Call the hello tool with name set to Ada.
```

The expected tool text is `Hello, Ada!`, with details `{"greeted":"Ada"}`.
Check the tool result, not just the model's prose. If no tool is available,
inspect load diagnostics, the manifest path and whether host and SDK revisions
match. A second greeting extension can conflict with the same tool name. Remove
or disable that duplicate.

### 4. Change, enable or remove the extension

Exit the session before running the following maintenance commands. After a
source change, rebuild and replace the packaged artifact:

```sh
cd ../hello-extension
cargo build --locked --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/hello_extension.wasm dist/index.wasm
cd ../greeting-project
maestro
```

A fresh session loads the new artifact. In an already running interactive
session, `/reload` reloads the artifact and filters; it does not build Rust or
copy your build output.

Installation selects the component unless existing filters disable it. To
disable it without uninstalling, change only its `packages` entry in
`.maestro/settings.json` to the following object, keeping the installer's actual
stored `source` value and every unrelated setting:

```json
{
  "source": "../../hello-extension",
  "extensions": []
}
```

For this directory layout the source is `../../hello-extension`. Remove the
`extensions` key to enable it again, then restart or `/reload`. Resource filters
are not a sandbox. See [resource filters][filters] for other kinds and patterns.

After exiting the session, remove the project registration:

```sh
maestro remove ../hello-extension --local
maestro list
```

The registration disappears and your source directory remains intact. Restart
or reload any session that already loaded the component. Uninstall does not
undo files, processes or external effects an extension created.

### 5. Prepare a marketplace submission

Keep your source, `Cargo.toml`, `Cargo.lock`, toolchain, license and tested SDK
revision in version control. For Git distribution, commit `package.json` and
`dist/index.wasm` too; ignored build output alone cannot be installed. For npm,
choose a package name you own, inspect the tarball and publish the built files.
From `greeting-project`, publish to your own npm account only when you intend
to release the package publicly:

```sh
cd ../hello-extension
npm login
npm pkg set "name=@$(npm whoami)/maestro-hello-workshop"
npm pack --dry-run
npm publish --access public
```

Consumers can then install the exact published version from their own project,
substituting the publisher's real npm username for `your-name`:

```sh
maestro install npm:@your-name/maestro-hello-workshop@0.1.0 --local
```

The [package source guide][sources] also supports pinned Git sources. Local
sources and explicitly pinned remote sources are skipped by ordinary updates;
install a new exact pin to change a version. Use one source form per package to
avoid loading both the local and published copies.

For acceptance here, follow [publishing a package](marketplace.md#publishing-a-package)
and attach the [review checklist](marketplace.md#review-checklist) to your pull
request. The greeting has these catalog inputs:

| Input | This example supplies |
| --- | --- |
| Package name and purpose | `package.json` name and description |
| Provided resource | `maestro.extensions` names `dist/index.wasm` |
| Release | `0.1.0` for npm; the selected commit for Git; local files are unpinned |
| Owner | No CODEOWNERS rule in this local example, so unowned; add a real repository principal in `.github/CODEOWNERS` for publication |

Declare the package through the catalog's existing package entity model using
these inputs. Follow its existing relations to the provided resources; do not
add a metadata sidecar or a second schema. The catalog consumer owns the actual
entity declaration surface, which is not an available author command today.
The Web UI must list and correlate accepted packages and use the same
package-manager install/remove operations as a manifest. It has no installer
today. [Catalog metadata](marketplace.md#catalog-metadata-has-one-owner) and
[the shared install path](marketplace.md#one-install-path-for-the-ui-and-manifest)
state the design requirements without introducing a new API.

The existing [clean-folder component check][clean-folder-check] covers the
copied greeting source's standalone build, native discovery and tool result.
It does not prove this package-manager registration recipe, npm publication or
catalog rendering. Those are separate checks in a release review; this
documentation change does not claim a new build or runtime test run.

[guest]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md
[guest-register]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md#register
[guest-events]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md#events-and-results
[guest-channels]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest.md#event-channels
[registration]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/registration.md
[hooks]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/hooks.md
[event-bus]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/event_bus.md
[guest-operations]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/guest_operations.md
[host-io]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/extensions/host_io.md
[wit-world]: https://github.com/Orchestration-Maestro/maestro/blob/58787fdc77242aaac2f74c7b2ca6e3a2748d904d/crates/maestro-extensions-wasm/wit/world.wit
[first-session]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/first-session.md
[hello-source]: https://github.com/Orchestration-Maestro/maestro/blob/58787fdc77242aaac2f74c7b2ca6e3a2748d904d/examples/extensions/hello.rs
[component-authoring]: https://github.com/Orchestration-Maestro/maestro/blob/58787fdc77242aaac2f74c7b2ca6e3a2748d904d/docs/extensions.md
[filters]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-guide.md#select-resources
[sources]: https://github.com/Orchestration-Maestro/maestro/blob/main/docs/package-sources.md
[clean-folder-check]: https://github.com/Orchestration-Maestro/maestro/blob/58787fdc77242aaac2f74c7b2ca6e3a2748d904d/docs/extensions.md#check-the-recipe-without-a-model
