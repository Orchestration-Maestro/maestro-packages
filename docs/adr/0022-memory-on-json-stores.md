# Memory behaviour maps onto the JSON stores, no database

The clearwing memory backends are SQLite with FTS5 and sync triggers, but
the program forbids new backends of any kind. Decided (2026-10-26): the
observable behaviour maps onto the existing JSON stores — the same record
shapes and sync discipline, the FTS5+trigger pattern as an in-memory
word index over the JSON store. No database ships in the port.
