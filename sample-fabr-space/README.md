# sample-fabr-space

The demo dataset for the CalHacks 13 judging. `home/` is the output of `crates/fabr-core/src/sample.rs`
with the clock pinned to 2026-10-25T09:00:00Z and paths written as `<SAMPLE>/home`.

Regenerate it:

    FABR_SAMPLE_NOW=2026-10-25T09:00:00Z cargo run -p fabr -- sample sample-fabr-space/home

Without `FABR_SAMPLE_NOW`, `cargo run -p fabr -- sample <dir>` writes the same tree relative to now with real paths.
