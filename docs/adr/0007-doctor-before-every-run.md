# Every run starts with the doctor probe

Decided (2026-10-25): every run begins with an automatic, fast doctor
probe — runner reachability plus a one-token LLM probe. An unhealthy
result refuses the start with the doctor classification before any phase
runs, so no run spends itself on a dead box.
