# Reload runtime

`/reload-runtime` awaits the reload request its command context supplies and
returns nothing else. The `reload_runtime` tool runs with the ordinary context,
which has no reload method, so it queues `/reload-runtime` as a follow-up user
message and answers that it queued it. The controlled run reloads nothing in a
running application; loading the component is opt-in and needs that
application's component runtime.

## Controlled runs

The controlled runs for this example are not hosted here yet.
[#75](https://github.com/Orchestration-Maestro/maestro-packages/issues/75) adds
them.
