# The graph and the matrix must agree, row by row

Decided (2026-10-25): every decomposition row cites its code-graph node —
from the per-framework graphs and the Maestro master graph — and the
coverage check runs the graph blast radius against the matrix before any
build step. No node unmapped, no behaviour unowned.
