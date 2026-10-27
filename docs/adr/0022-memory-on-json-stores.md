# Memory behaviour maps onto the JSON stores, no database

The memory backends use a relational database with full-text search and
sync triggers, but the program forbids new backends of any kind. Decided
(2026-10-26): the observable behaviour maps onto the existing JSON stores,
with the same record shapes and sync discipline. An in-memory word index
over the JSON store replaces full-text search and triggers. No database
ships in Maestro.
