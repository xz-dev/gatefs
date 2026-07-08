# Policy, bypass rules, protection, and grants

`sandboxfs` evaluates policy per filesystem effect. An effect has a policy layer (`READ`, `WRITE`, or `METADATA`) and a sandbox path.

For each effect:

1. a matching `bypass-*` rule automatically allows the effect without creating a pending request;
2. otherwise, a matching `protect-*` rule creates a pending authorization request;
3. otherwise, the effect is allowed by default.

The operation may execute only when all of its effects are allowed. If any effect is denied or canceled, the whole operation fails.

## Protection rules

Read/write protection rules are configured separately:

```sh
sandboxfs demo protect-read '/data/**'
sandboxfs demo protect-write '/data/**'
sandboxfs demo unprotect-read '/data/**'
sandboxfs demo unprotect-write '/data/**'
sandboxfs demo list-protection [--read] [--write]
```

Metadata protection is separate:

```sh
sandboxfs demo protect-metadata '/data/**'
sandboxfs demo unprotect-metadata '/data/**'
sandboxfs demo list-protection --metadata
```

A matching protected effect becomes a pending authorization request. Inspect or resolve it with:

```sh
sandboxfs demo allow
sandboxfs demo allow <operation_id>
sandboxfs demo allow --do-nothing <operation_id>
sandboxfs demo deny <operation_id>
sandboxfs demo cancel <operation_id>
sandboxfs demo cancel-all [mountpoint]
sandboxfs-access-tui demo
```

Inspecting pending requests is read-only. Multiple CLI tools or Access TUI instances may view the same foreground session socket concurrently. `allow`, `allow --do-nothing`, `deny`, or lifecycle `cancel` resolves and removes a pending request. `cancel-all` cancels all pending requests in the sandbox, or only pending requests from the attached view identified by `<mountpoint>` when a mountpoint is provided.

`allow --do-nothing` releases the blocked FUSE request according to the normal do-nothing semantics for that request.

## Bypass rules

Bypass rules are automatic-allow exclusions from protection rules:

```sh
sandboxfs demo bypass-read '/data/**'
sandboxfs demo bypass-write '/data/**'
sandboxfs demo bypass-metadata '/data/**'
sandboxfs demo unbypass-read '/data/**'
sandboxfs demo unbypass-write '/data/**'
sandboxfs demo unbypass-metadata '/data/**'
sandboxfs demo list-bypass [--read] [--write] [--metadata]
```

`bypass-*` rules are layer-specific. `bypass-write` automatically allows matching write effects, but it does not bypass metadata protection. `bypass-metadata` automatically allows matching metadata effects, but it does not bypass write protection.

This matters because a single FUSE operation can have multiple effects. For example, truncate changes file size/content semantics, so it has a `WRITE` effect, but it also updates metadata. If `protect-metadata` matches and `bypass-metadata` does not, truncate must not automatically succeed even when its write effect is otherwise allowed or covered by `bypass-write`.

Hard link is another multi-effect operation: the source path has a `METADATA` effect because the source inode's link count and ctime change, while the destination path has a `WRITE` effect because a new directory entry is created.

## Grants

For protected read/write requests, bare `allow <operation_id>` only releases the current blocked request.

Add grant options to create a future-matching read/write grant:

```sh
sandboxfs demo allow <operation_id> --path <sandbox-glob> --duration
sandboxfs demo allow <operation_id> --path <sandbox-glob> --duration=30m
sandboxfs demo allow <operation_id> --path <sandbox-glob> --tree
```

`--path <sandbox-glob>` chooses the grant path pattern. `--duration` or `--duration=<duration>` creates a duration grant; the default is 30 minutes. `--tree` snapshots the requester's current process tree instead of the exact requester process. If grant options are present without `--duration`, the grant is one-shot.

## Pattern semantics

A bypass or protection pattern is a sandbox namespace glob:

- `/a/b` matches that exact file or directory.
- `/a/b/` is directory-only.
- `/a/*` matches one path segment below `/a`.
- `/a/**` matches a recursive subtree below `/a`; it does not match `/a` itself.
- `/*/` matches one directory level below `/`, but not `/` itself.
- `/**/` matches non-root directories recursively, but not regular files and not `/` itself.

Patterns use Rust [`glob`](https://docs.rs/glob/) crate semantics with sandboxfs' directory-only handling for trailing `/`.

## Access TUI

The TUI displays pending requests and supports allow, deny, do-nothing, and edit-command. Edit-command reruns a user-edited `chmod`, `chown`, or `chattr` through the trusted `sandboxfs` CLI path, then releases the original pending request with do-nothing. Read/write TUI allow/deny/do-nothing resolves only the selected pending request and does not create broader grants.
