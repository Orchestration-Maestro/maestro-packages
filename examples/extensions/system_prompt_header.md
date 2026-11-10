# System prompt header

On `agent_start` the example reads the prompt the host supplies at that moment
and sets the `system-prompt` status to `System: N chars`. `N` counts UTF-16 code
units, so whitespace, a byte order mark and combining sequences count as the
string stores them; nothing is trimmed, rebuilt or cached. `session_shutdown`
removes the status under the same key, including when no start came before it.
A host rejection is returned to the host unchanged. Loading the component into
an application is opt-in.

## Controlled runs

The controlled runs for this example are not hosted here yet.
[#75](https://github.com/Orchestration-Maestro/maestro-packages/issues/75) adds
them.
