# Metadata operations

`gatefs` separates metadata operations from read/write operations.

## Trusted CLI metadata operations

`gatefs demo chmod/chown/chattr ...` creates a temporary FUSE mountpoint under the runtime directory, runs the command name found through `PATH`, and then immediately detaches and removes the temporary mountpoint.

These trusted CLI-initiated operations skip the pending authorization flow, but they can still fail normally if the command fails, the path does not exist, or the FUSE operation is unsupported. They only update sandbox-local metadata overrides; they do not chmod/chown/chattr the underlying files.

## Direct FUSE metadata operations

Direct metadata changes through an attached FUSE mountpoint are untrusted but not protected by default. Unless a path matches `protect-metadata`, metadata operations update sandbox-local metadata overrides where gatefs manages that metadata surface, without mutating the underlying files and without creating a pending request.

Protect metadata explicitly when direct metadata changes should require approval:

```sh
gatefs demo protect-metadata '/data/**'
chmod 444 "$DEMO_MNT/file.txt"
```

That protected request becomes pending and can be resolved through the normal pending authorization flow.

## Metadata bypass

Use `bypass-metadata` when matching metadata effects should be automatically allowed without pending authorization:

```sh
gatefs demo bypass-metadata '/path/to/file.lock'
```

`bypass-metadata` is layer-specific. It does not bypass read or write protection, and `bypass-write` does not bypass metadata protection.

See [Policy, bypass rules, protection, and grants](policy.md) for the full policy model.
