# gatefs documentation

`gatefs` is an observable filesystem protection shim. It is meant to be initialized explicitly by the caller for each use case, usually from a small script that combines an existing sandboxing tool with `gatefs` policy commands.

## User guide

- [Concepts and lifecycle](user-guide/concepts.md)
- [Command reference](user-guide/commands.md)
- [Policy, bypass rules, protection, and grants](user-guide/policy.md)
- [Metadata operations](user-guide/metadata.md)
- [Logs, runtime paths, and limitations](user-guide/runtime-and-limits.md)
- [AI agent wrapper notes](user-guide/ai-agent-wrapper.md)
- [Development checks](user-guide/development.md)

## Architecture decisions

Architecture decision records live under [adr/](adr/).
