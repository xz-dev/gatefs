# ADR 0007: Bypass rules and effect-based authorization

## Status

Accepted.

## Context

ADR 0006 introduced per-layer passthrough rules as a narrow compatibility mechanism, primarily for AI-agent lock-directory workflows. That naming and behavior made the policy model harder to reason about: `passthrough-write` sounded like a partial host-filesystem write implementation, while the intended user-facing need is simpler.

The policy question users need to answer is not "which internal write operations has sandboxfs implemented as passthrough?" It is:

- which filesystem effects should require authorization;
- which effects should be automatically allowed; and
- which effects should remain visible in logs and pending requests when they are not automatically allowed.

FUSE exposes filesystem operations, not the originating shell command. A single operation may have multiple filesystem effects. For example, a hard link adds a destination directory entry and changes the source inode's link count/ctime. Treating the whole operation as one coarse `WRITE` loses the distinction between the destination write effect and the source metadata effect.

## Decision

sandboxfs renames `passthrough-*` to `bypass-*` and defines bypass rules as automatic-allow exclusions from protection rules.

The command surface is:

```text
sandboxfs <name> bypass-read <glob>
sandboxfs <name> bypass-write <glob>
sandboxfs <name> bypass-metadata <glob>
sandboxfs <name> unbypass-read <glob>
sandboxfs <name> unbypass-write <glob>
sandboxfs <name> unbypass-metadata <glob>
sandboxfs <name> list-bypass [--read] [--write] [--metadata]
```

`bypass-*` rules are layer-specific. A matching `bypass-write` automatically allows the matching write effect; it does not bypass metadata effects. A matching `bypass-metadata` automatically allows the matching metadata effect; it does not bypass write effects.

Authorization is evaluated per filesystem effect, not per command name. Each effect has a policy layer (`READ`, `WRITE`, or `METADATA`) and a sandbox path. For each effect:

1. if a matching `bypass-*` rule exists for that effect's layer and path, that effect is automatically allowed;
2. otherwise, if a matching `protect-*` rule exists for that effect's layer and path, that effect requires pending authorization;
3. otherwise, that effect is allowed by default.

The operation may execute only when all effects are allowed. If any effect requires pending authorization, the operation blocks until it is allowed, denied, canceled, or released with do-nothing according to the pending authorization semantics. If any effect is denied or canceled, the whole operation fails.

## Metadata precedence

Metadata protection is independent and has the highest precedence for metadata effects.

Operations that change file contents or directory entries often also update metadata such as mtime, ctime, link count, or parent-directory timestamps. `protect-write` protects the content or namespace write effect, but it does not implicitly cover the metadata effect. Conversely, `bypass-write` only automatically allows the write effect; it does not bypass `protect-metadata`.

Therefore an operation such as truncate is a `WRITE` effect because it changes file size/content semantics, but it also has a metadata effect. If `protect-metadata` matches that path and `bypass-metadata` does not, the truncate must not automatically succeed even if the write effect is otherwise unprotected or matched by `bypass-write`.

This same rule applies to other coupled write+metadata operations. The implementation may initially model a conservative subset of metadata side effects, but the policy rule is that metadata effects are evaluated separately from write effects.

## Multi-path effects

Multi-path operations are judged by the filesystem objects they affect, not by the command that caused them.

For hard link:

- the source path has a `METADATA` effect because its inode link count and ctime change;
- the destination path has a `WRITE` effect because a new directory entry is created.

For rename:

- the source path has a `WRITE` effect because its directory entry is removed or moved;
- the destination path has a `WRITE` effect because its directory entry is created or replaced.

If any affected effect is protected and not bypassed, the operation must pending.

## Consequences

The user-facing model becomes simpler: `protect-*` means "ask before this effect" and `bypass-*` means "automatically allow this effect." There is no separate "passthrough" vocabulary in commands, logs, IPC output, examples, or documentation.

ADR 0006 remains historical context for the split between read/write and metadata policy layers and for glob pattern semantics, but this ADR supersedes ADR 0006's `passthrough-*` naming and its narrow `passthrough-write` behavior. Write authorization should no longer mean "allow the request and then still return EROFS" for write operations that sandboxfs exposes as protectable. If a protected write effect is allowed, sandboxfs should perform the corresponding filesystem operation unless another protected effect, such as metadata, blocks the operation.
