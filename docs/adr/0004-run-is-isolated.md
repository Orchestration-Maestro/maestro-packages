# A run is an isolated background process

Decided (2026-10-25): a run is not an event stream inside the session's
own context — it is a background process with its own workflow, its own
logs and its own isolated state. Maestro (and the user) access it through a
command surface; the session thread carries a curated timeline of the run
(phase transitions and findings), never the run's raw events or logs.
