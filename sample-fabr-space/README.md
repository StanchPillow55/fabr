# sample-fabr-space

A synthetic demo space for the fabric desktop app: three live-looking projects, a nested archive
folder, a machine root with settings, a data bank policy, an audit log, notices and a registry.
Every value is made up. The shops use `.test` hosts and no file holds a bank value.

The fixed "now" is `2026-10-25T09:00:00Z`. Heartbeats are measured against it: two agents are
live, one is dead (3 minutes old).

Open `home/.fabric-space/settings.toml` for the machine root and `home/projects/` for the task
folders. Point the app at the projects: choose `sample-fabr-space/home/projects` as the default
folder in setup, or set `default_folder` in `settings.toml`.

Paths inside the files use the prefix `<SAMPLE>`. Replace it with the real absolute path, from
inside `sample-fabr-space` (macOS):

```sh
sed -i '' "s|<SAMPLE>|$(pwd)|g" $(grep -rl '<SAMPLE>' .)
```

This set is frozen. It changes only when the owner asks.
