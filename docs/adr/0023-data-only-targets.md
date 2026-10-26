# Data-only targets waive graphify

ADR-0020 requires row-level graph nodes. Some campaign targets are data
files, not code (rea-check's doctor.json fixtures): their agreement is
byte evidence and the extraction is recorded as zero nodes. Decided
(2026-10-26): such targets record the waiver explicitly and carry no
graph appendix; everything else must join row by row.
