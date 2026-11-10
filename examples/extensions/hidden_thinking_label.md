# Hidden thinking label

`/thinking-label <text>` sets the collapsed-thinking label the host shows, and
`/thinking-label` without arguments clears it and restores `Pondering...` for the
next session start. The example also applies the label it retains at every
`session_start`, so the next session shows the label the session before set.

A command that sets a label trims the surrounding whitespace and keeps the interior
text as it is. When the host rejects a request the command ends with the host's
message, and the label the example retains stays the one the command chose.
Loading the component into an application is opt-in and needs a host that enables
the component runtime.

## Controlled runs

The controlled runs for this example are not hosted here yet.
[#75](https://github.com/Orchestration-Maestro/maestro-packages/issues/75) adds
them.
