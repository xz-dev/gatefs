# sandboxfs

`sandboxfs` is an experimental, observable filesystem protection shim built on [`fuser`](https://github.com/cberner/fuser). It gives a process a FUSE-backed filesystem view whose read, write, and metadata permissions can be inspected, protected, granted, denied, and adjusted at runtime.

It is designed to complement existing sandboxing tools such as Bubblewrap, containers, or VM-based runners, not to replace them. Those tools still provide the process boundary; `sandboxfs` adds the dynamic filesystem policy layer that static bind mounts and read-only mounts do not provide.

The initial design target is AI agents: they are unusually dynamic, tool-heavy, and hard to predict ahead of time, so static filesystem permissions are often either too broad to be useful or too narrow to let the agent finish the task. `sandboxfs` focuses on observability and controllability for that workflow: host paths are presented through sandbox paths, sensitive operations can become pending authorization requests, and decisions are logged against the sandbox view rather than leaking host-specific details into ordinary prompts.

The important lifecycle rule is explicit foreground ownership: a sandbox exists only while a visible `sandboxfs run <name>` process is running. There is no hidden `sandboxfsd`, no automatic daemon startup, and no global `list` command.

## Basic usage

Start a foreground sandbox session in one terminal:

```sh
sandboxfs run demo
```

In another terminal, map local data into the sandbox and expose it through FUSE:

```sh
DEMO_MNT="$(mktemp -d)"
sandboxfs demo mount /some/local/dir /
sandboxfs demo attach "$DEMO_MNT"
ls "$DEMO_MNT"
cat "$DEMO_MNT/file.txt"
```

Add a protection rule when an operation should be observable and adjustable at runtime:

```sh
sandboxfs demo protect-write '/**'
echo updated > "$DEMO_MNT/file.txt" # blocks and creates a pending request
sandboxfs demo allow
sandboxfs demo allow <operation_id>
```

Unmount one attach point:

```sh
sandboxfs demo detach "$DEMO_MNT"
rmdir "$DEMO_MNT"
```

Stop the foreground session and drop all in-memory state:

```sh
sandboxfs demo destroy
```

Ctrl-C in the `sandboxfs run demo` terminal also stops the session.

## Commands

```text
sandboxfs run <name>
sandboxfs <name> destroy
sandboxfs <name> attach <mountpoint>
sandboxfs <name> detach <mountpoint>
sandboxfs <name> mount <local> <on_fs>
sandboxfs <name> mount
sandboxfs <name> umount <on_fs>
sandboxfs <name> hide <on_fs>
sandboxfs <name> protect-read <pattern>
sandboxfs <name> protect-write <pattern>
sandboxfs <name> protect-metadata <pattern>
sandboxfs <name> unprotect-read <pattern>
sandboxfs <name> unprotect-write <pattern>
sandboxfs <name> unprotect-metadata <pattern>
sandboxfs <name> passthrough-read <pattern>
sandboxfs <name> passthrough-write <pattern>
sandboxfs <name> passthrough-metadata <pattern>
sandboxfs <name> unpassthrough-read <pattern>
sandboxfs <name> unpassthrough-write <pattern>
sandboxfs <name> unpassthrough-metadata <pattern>
sandboxfs <name> list-protection [--read] [--write] [--metadata]
sandboxfs <name> list-passthrough [--read] [--write] [--metadata]
sandboxfs <name> chmod ...
sandboxfs <name> chown ...
sandboxfs <name> chattr ...
sandboxfs <name> allow [operation_id]
sandboxfs <name> allow <operation_id> [--path <sandbox-glob>] [--duration[=<duration>]] [--tree]
sandboxfs <name> allow --do-nothing <operation_id>
sandboxfs <name> deny <operation_id>
sandboxfs <name> cancel <operation_id>
sandboxfs <name> cancel-all [mountpoint]
sandboxfs <name> monitor [-f]
sandboxfs <name> metadata
sandboxfs-access-tui <name>
```

`mount` without arguments lists mappings and hide rules for the sandbox. `allow` without arguments lists pending authorization requests.

## Overlay and hide behavior

Mappings are added with:

```sh
sandboxfs demo mount <local_path> <sandbox_path>
```

Later mappings overlay earlier mappings, similar to mounts. Intermediate sandbox directories that do not exist in the underlying local filesystems are virtual, in-memory directories.

Hide a sandbox subtree with:

```sh
sandboxfs demo hide /path/in/sandbox
```

A hide rule removes that path and descendants from visibility until a newer mapping covers that path again.

## Metadata operations

`sandboxfs demo chmod/chown/chattr ...` creates a temporary FUSE mountpoint under the runtime directory, runs the command name found through `PATH`, and then immediately detaches and removes the temporary mountpoint.

These trusted CLI-initiated operations skip the pending authorization flow, but they can still fail normally if the command fails, the path does not exist, or the FUSE operation is unsupported. They only update sandbox-local metadata overrides; they do not chmod/chown/chattr the underlying files.

Direct metadata changes through an attached FUSE mountpoint are untrusted but not protected by default. Unless a path matches `protect-metadata`, metadata operations update sandbox-local metadata overrides where sandboxfs manages that metadata surface, without mutating the underlying files and without creating a pending request.

Protect metadata explicitly when direct metadata changes should require approval:

```sh
sandboxfs demo protect-metadata '/data/**'
chmod 444 "$DEMO_MNT/file.txt"
```

That protected request becomes pending. Inspect or resolve it with:

```sh
sandboxfs demo allow
sandboxfs demo allow <operation_id>
sandboxfs demo allow --do-nothing <operation_id>
sandboxfs demo deny <operation_id>
sandboxfs demo cancel <operation_id>
sandboxfs demo cancel-all [mountpoint]
sandboxfs-access-tui demo
```

Inspecting pending requests is read-only. Multiple CLI tools or Access TUI instances may view the same foreground session socket concurrently; `allow`, `allow --do-nothing`, `deny`, or lifecycle `cancel` resolves and removes a pending request. `cancel-all` cancels all pending requests in the sandbox, or only pending requests from the attached view identified by `<mountpoint>` when a mountpoint is provided.

`allow --do-nothing` lets the blocked FUSE request return success without changing sandbox metadata or underlying files.

Read/write protection rules are configured separately with `protect-read`, `protect-write`, `unprotect-read`, `unprotect-write`, and `list-protection`. Metadata protection uses `protect-metadata` and `unprotect-metadata`; `list-protection --metadata` filters to metadata rules. Passthrough rules are also layer-specific: `passthrough-read` and `passthrough-write` apply only to read/write operations, while `passthrough-metadata` applies only to metadata operations. A passthrough or protection pattern is a sandbox namespace glob: `/a/b` matches that exact file or directory, `/a/b/` is directory-only, `/a/*` matches one path segment below `/a`, and `/a/**` matches a recursive subtree below `/a`. For protected read/write requests, bare `allow <operation_id>` only releases the current blocked request. Add grant options to create a future-matching read/write grant: `--path <sandbox-glob>` chooses the grant path pattern, `--duration` or `--duration=<duration>` creates a duration grant (default 30 minutes), and `--tree` snapshots the requester's current process tree instead of the exact requester process. If grant options are present without `--duration`, the grant is one-shot.

The TUI displays pending requests and supports allow, deny, do-nothing, and edit-command. Edit-command reruns a user-edited `chmod`, `chown`, or `chattr` through the trusted `sandboxfs` CLI path, then releases the original pending request with do-nothing. Read/write TUI allow/deny/do-nothing resolves only the selected pending request and does not create broader grants.

## AI agent wrapper example

`example/pi-sandbox.sh` shows the intended integration shape for an AI coding agent: use Bubblewrap for the process/container boundary, then put `sandboxfs` inside that boundary as the observable filesystem policy layer. The wrapper keeps the agent-facing view simple while allowing selected paths, PATH tools, lock directories, and protected operations to be managed through `sandboxfs` policy instead of static bind mounts alone.

## Logs and monitoring

Show the operation log:

```sh
sandboxfs demo monitor
sandboxfs demo monitor -f
```

`monitor` prints the recent log tail; `monitor -f` starts at the same tail and follows new log entries. Logs are reset when `sandboxfs run <name>` starts and are removed when the sandbox is destroyed.

Audit log entries use filesystem-operation vocabulary rather than shell command reconstruction. Every entry has a UTC microsecond timestamp and its own event ID, for example:

```text
[2026-06-29T13:55:12.123456Z] id=3 pending path=/data/file.txt SETATTR mode=0600
[2026-06-29T13:55:13.000042Z] id=4 decision request=3 ALLOW
[2026-06-29T13:55:14.999999Z] id=5 trusted path=/data/file.txt SETATTR mode=0444
```

The log writer is a serialized event loop. FUSE and control paths publish events to it instead of appending directly from concurrent call sites.

## Runtime paths

- `SANDBOXFS_RUNTIME_DIR` overrides the runtime directory.
- Without an override, `sandboxfs` asks `directories-rs` (`directories::ProjectDirs`) for the project runtime directory.
- If the platform has no project runtime directory, it falls back to the project cache directory with a `run` child.
- Runtime directories are created with mode `0700`.
- Socket path defaults to `<runtime>/<name>.sock`.
- `SANDBOXFS_SOCKET` overrides the socket path for special cases and tests.
- Log path defaults to `<runtime>/<name>.log`.
- `SANDBOXFS_LOG_DIR` overrides the log directory.
- Temporary trusted-operation mountpoints live under `<runtime>/tmp/`.

## Current limitations

- `sandboxfs` is not a complete process sandbox or security boundary by itself; use it with an existing sandboxing or runtime isolation tool when process isolation is required.
- File content and directory structure writes are read-only unless a path matches an explicit passthrough rule for a supported operation. In this version, `passthrough-write` enables lock-directory mkdir/rmdir passthrough, and `passthrough-metadata` enables timestamp and xattr metadata passthrough for matching visible paths. Other create/write/truncate/unlink/rename operations still return read-only or unsupported errors and never modify underlying files.
- Real FUSE behavior depends on `/dev/fuse` and `fusermount3` availability and permissions.
- The project is experimental.

## Development checks

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
SANDBOXFS_RUN_FUSE_TESTS=1 cargo test --test fuse_behavior -- --ignored
SANDBOXFS_RUN_FUSE_TESTS=1 SANDBOXFS_RUN_STRESS_TESTS=1 cargo test --test fuse_behavior stress_multiple_pending_viewers_do_not_consume_request -- --ignored
```
