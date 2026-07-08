# Logs, runtime paths, and limitations

## Logs and monitoring

Show the operation log:

```sh
gatefs demo monitor
gatefs demo monitor -f
```

`monitor` prints the recent log tail. `monitor -f` starts at the same tail and follows new log entries. Logs are reset when `gatefs run <name>` starts and are removed when the sandbox is destroyed.

Audit log entries use filesystem-operation vocabulary rather than shell command reconstruction. Every entry has a UTC microsecond timestamp and its own event ID, for example:

```text
[2026-06-29T13:55:12.123456Z] id=3 pending path=/data/file.txt SETATTR mode=0600
[2026-06-29T13:55:13.000042Z] id=4 decision request=3 ALLOW
[2026-06-29T13:55:14.999999Z] id=5 trusted path=/data/file.txt SETATTR mode=0444
```

The log writer is a serialized event loop. FUSE and control paths publish events to it instead of appending directly from concurrent call sites.

## Runtime paths

- `GATEFS_RUNTIME_DIR` overrides the runtime directory.
- Without an override, `gatefs` asks `directories-rs` (`directories::ProjectDirs`) for the project runtime directory.
- If the platform has no project runtime directory, it falls back to the project cache directory with a `run` child.
- Runtime directories are created with mode `0700`.
- Socket path defaults to `<runtime>/<name>.sock`.
- `GATEFS_SOCKET` overrides the socket path for special cases and tests.
- Log path defaults to `<runtime>/<name>.log`.
- `GATEFS_LOG_DIR` overrides the log directory.
- Temporary trusted-operation mountpoints live under `<runtime>/tmp/`.

## Current limitations

- `gatefs` is not a complete process sandbox or security boundary by itself; use it with an existing sandboxing or runtime isolation tool when process isolation is required.
- Protection and bypass are evaluated per filesystem effect. A matching `bypass-write` automatically allows write effects but does not bypass `protect-metadata` for metadata side effects. Write operations, including `mknod`, are forwarded to the backing filesystem after policy allows them; backing filesystem, mount, process, or kernel restrictions may still return their native errno.
- Real FUSE behavior depends on `/dev/fuse` and `fusermount3` availability and permissions.
- The project is experimental.
