# Command reference

```text
gatefs run <name>
gatefs <name> destroy
gatefs <name> attach <mountpoint>
gatefs <name> detach <mountpoint>
gatefs <name> mount <local> <on_fs>
gatefs <name> mount
gatefs <name> umount <on_fs>
gatefs <name> hide <on_fs>
gatefs <name> protect-read <pattern>
gatefs <name> protect-write <pattern>
gatefs <name> protect-metadata <pattern>
gatefs <name> unprotect-read <pattern>
gatefs <name> unprotect-write <pattern>
gatefs <name> unprotect-metadata <pattern>
gatefs <name> bypass-read <pattern>
gatefs <name> bypass-write <pattern>
gatefs <name> bypass-metadata <pattern>
gatefs <name> unbypass-read <pattern>
gatefs <name> unbypass-write <pattern>
gatefs <name> unbypass-metadata <pattern>
gatefs <name> list-protection [--read] [--write] [--metadata]
gatefs <name> list-bypass [--read] [--write] [--metadata]
gatefs <name> chmod ...
gatefs <name> chown ...
gatefs <name> chattr ...
gatefs <name> allow [operation_id]
gatefs <name> allow <operation_id> [--path <sandbox-glob>] [--duration[=<duration>]] [--tree]
gatefs <name> allow --do-nothing <operation_id>
gatefs <name> deny <operation_id>
gatefs <name> cancel <operation_id>
gatefs <name> cancel-all [mountpoint]
gatefs <name> monitor [-f]
gatefs <name> metadata
gatefs-access-tui <name>
```

`mount` without arguments lists mappings and hide rules for the sandbox. `allow` without arguments lists pending authorization requests.

See also:

- [Concepts and lifecycle](concepts.md)
- [Policy, bypass rules, protection, and grants](policy.md)
- [Metadata operations](metadata.md)
- [Logs, runtime paths, and limitations](runtime-and-limits.md)
