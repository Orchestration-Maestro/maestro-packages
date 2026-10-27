# Domain docs

How the engineering skills consume this repo's domain documentation.

## Before exploring, read these

- `CONTEXT.md` at the repo root: the glossary. Workspace convention: the
  glossary lives in `CONTEXT.md`, not `GLOSSARY.md`.
- `docs/adr/`: read the ADRs that touch the area you're about to work in.

If these files don't exist, proceed silently. The domain-modeling skill
(reached via grill-with-docs) creates them lazily when terms or decisions
actually get resolved.

## File structure

Single-context repo:

```
/
├── CONTEXT.md
├── docs/
│   ├── adr/
│   │   ├── 0001-trigger-is-a-command.md
│   │   └── 0002-run-is-a-session-citizen.md
│   └── agents/
└── crates/
```
