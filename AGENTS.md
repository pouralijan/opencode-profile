# Working on opencode-profile

- Profile roots are independently resolved under each XDG home at `opencode-profile/profiles/NAME`; preserve distinct config, data, state, and cache roots. Runtime OpenCode CLI preferences use `config/NAME/.xdg/opencode/cli.json`.
- `run` reuses a native service per profile: set `OPENCODE_CONFIG_DIR` and all four XDG homes, then inject only the internally verified `--server` endpoint and environment-based password. Reject caller routing overrides and unknown routes; `debug paths` and single-flag root help/version remain local.
- Keep ownership checks and lifecycle coordination in `src/service.rs`: lock the state namespace outside removable roots, validate both four-root manifests, and prepare authenticated clients under the lock. No discovery fallback after unlock; removal must stop the verified service before deleting directories.
- Autostart uses owned systemd user units in `src/autostart.rs`, with a separate stable lock acquired before lifecycle locks. Remove enabled units before profile data; never persist credentials in units or change lingering automatically. Test with fake `systemctl`, not the host manager.
- Create starts fresh; clone copies only `$schema`, `theme`, `model`, and `small_model` string settings. Never read/import `opencode-multi` profiles or copy credentials/session data.
- For CLI integration tests, use temporary XDG homes and a fake `opencode` executable; never inspect real auth/session files. Routing and path changes require updates to `tests/cli.rs`.
- Verify changes with `cargo fmt --check`, `cargo test`, and `cargo check`. Keep `README.md`'s usage and isolation limitations aligned with behavior changes.
