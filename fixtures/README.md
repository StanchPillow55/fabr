# fixtures

Ten fixtures for the F0 exit test. Each is a small copy of what the agent backend and the GUI
share on disk, built from `.agents/skills/doc/dev2/agent_surface_spec.md`, section 10. The
backend's reader must load every record, and the GUI must render every one.

Fixtures 02 to 09 follow one task, "order 2 kg of black PLA", from question to re-delegation.
Fixture 01 is a different, trivial reminder. Fixture 10 has no records, only registry entries.

Fixed "now" for every liveness check: `2026-10-24T19:00:00Z`. An agent is dead when its
`heartbeat_at` is older than three intervals (30 s). Fixture 10 has two live entries and one dead
one; fixtures 05 and 06 are alternate views of the same run with heartbeats within 10 s of now.
Fixtures 07 to 09 end after now, so treat them as later snapshots and never check their liveness.

Mapping to a real machine: `<fixture>/project/` stands for a user's project folder, whose real
path inside every record is `/home/me/3d`. `<fixture>/home/.fabric-space/` is the machine root,
that is `~/.fabric-space` (registry, events log, data bank audit log). Copy either one to the
real path to try it.

All values are made up. The shop host `shop.example-filament.test` does not exist, and no file
holds a bank value.

Fixtures change in the same commit as the record format (spec, section 10).
