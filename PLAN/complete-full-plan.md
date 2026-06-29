# Complete sandboxfs full planned software

## Goal

Finish the foreground `sandboxfs run <name>` software to match the approved plan:

- no hidden daemon, no `list`, foreground session owns in-memory state;
- mapping/overlay/hide/metadata auth/log/monitor/TUI behavior works without modifying underlying files;
- CLI/TUI/IPC/BDD/FUSE success and error tests exist and use isolated temp dirs.

## Small implementation slices

1. **TUI edit-command + metadata logging**
   - Log trusted metadata overrides as chmod/chown/chattr/setattr entries.
   - Implement TUI edit-command path in a testable way without invoking an editor in tests.
   - Keep edit-command sandbox-local and route through trusted CLI command execution semantics.

2. **Integration and behavior tests**
   - Add isolated IPC tests.
   - Add TUI test-backend tests.
   - Add BDD-style behavior scenarios in Rust integration tests.
   - Add non-mounted filesystem/state error tests.

3. **Gated real FUSE tests**
   - Add ignored/gated tests for attach/read, detach errors, trusted chmod, direct untrusted chmod allow/deny/do-nothing, and read-only write errors.
   - Ensure each test uses unique temp runtime/socket/sandbox/mountpoint and cleanup.

4. **Docs and verification**
   - Update PLAN.md/README so no items are marked partial.
   - Run fmt, clippy, all non-gated tests, and gated FUSE tests if the environment supports FUSE.
