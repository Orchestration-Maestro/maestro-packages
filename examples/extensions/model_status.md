# Model status

`model_select` reads the selection the host reports. On an explicit selection or
a cycle it notifies `Model: {provider}/{id}` with info severity; restoring a
saved selection skips the notification. Every selection then sets the `model`
status to `🤖 {id}` and writes the previous and selected identifiers and
the selection source to standard output. An absent previous model reads `none`.
Repeated and unchanged selections are reported too.

The example makes no model or provider call and paints no footer; a host that
shows the status decides where it goes. `model_status_component_runs_selection_hook`
instantiates the built component and checks its host requests and its standard
output against the recorded selections. Loading the component into an application
is opt-in and needs that application's component runtime.

## Controlled runs

The controlled runs for this example are not hosted here yet.
[#75](https://github.com/Orchestration-Maestro/maestro-packages/issues/75) adds
them.
