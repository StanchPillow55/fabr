# PREFERENCES.md — project preferences for fabric

These bind below `RULES.md` (R-02) in this project and are cited by ID (`P-01` …). Set by the
owner on 2026-10-25.

- **P-01 No agents for small work.** If a change would take the main session less than one
  minute of its own work (a few lines in known files, a version bump, a path fix, a formatter
  run, a config tweak), do it directly and run the one relevant check. Launch a subagent only
  for multi-file implementation, exploration of unfamiliar code, or a phase-end verification.
- **P-02 One verifier per phase.** Verification runs once at the end of a phase (an F-wave or a
  milestone), with a brief limited to that phase's definition of done and one smoke run.
  Implementers self-check with the test gates inside a phase.
- **P-03 Commit between phases.** Each phase ends in a verified state and a commit, with the
  commands printed for the owner. The next phase's agents do not start until the owner says go.
