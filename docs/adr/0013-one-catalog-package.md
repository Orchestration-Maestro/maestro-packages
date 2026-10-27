# Everything ships as one catalog package

Decided (2026-10-25): the maestro-security extension installs one package
into Maestro's catalog: the runner, the source tools, the pipeline and the
skills, agents, instructions and workflow all defined in the catalog
manifest. The package is solely user-invocable — through
`/maestro-security` and the other transports.
