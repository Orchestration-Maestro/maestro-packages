# The trigger is a command, never message parsing

Autonomous assessments could start from any mention of a target in a
message. Decided (owner, 2026-10-25): only an explicit command (`/assess`)
starts a run — never message parsing, never the web UI — so a run never
starts by accident on a target mentioned in passing.
