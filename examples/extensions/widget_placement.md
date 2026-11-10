# Widget placement

On every `session_start`, `widget_placement` reads the host's current UI
availability and does nothing when there is none. With a UI it requests the
`widget-above` widget holding the line `Above editor widget`, then the
`widget-below` widget holding the line `Below editor widget` with the explicit
below-editor placement. The first request passes no options, so the host applies
its own above-editor default.

Each request acquires the UI context again, so a host that changes its binding
after the first request serves the second on the new one. A rejected request
ends the callback with that message; the example retries nothing and removes no
widget the host already accepted. Loading the component into an interactive
application is opt-in and needs that application's component runtime and UI
host; the controlled run only records the ordered requests.

## Controlled runs

The controlled runs for this example are not hosted here yet.
[#75](https://github.com/Orchestration-Maestro/maestro-packages/issues/75) adds
them.
