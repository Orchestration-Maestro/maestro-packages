# Data-only targets waive code-graph extraction

ADR-0020 requires row-level graph nodes. Some decomposition targets are data
files, not code, such as doctor-probe fixtures: their agreement is byte
evidence and the extraction is recorded as zero nodes. Decided
(2026-10-26): such targets record the waiver explicitly and carry no
graph appendix; everything else must join row by row.
