# Concepts and lifecycle

`gatefs` provides an observable FUSE filesystem view with runtime-adjustable protection policy. It is intended to complement process isolation tools such as Bubblewrap, containers, or VM-based runners. Those tools define the process boundary; `gatefs` adds the dynamic filesystem policy layer that static bind mounts and read-only mounts do not provide.

The initial design target is AI agents because their filesystem needs are unusually dynamic and hard to predict ahead of time. A static policy is often either too permissive to be useful or too narrow to let the agent finish the task. `gatefs` lets sensitive operations become visible pending requests that can be allowed, denied, or logged against the sandbox namespace.

## Explicit foreground ownership

A sandbox exists only while a visible `gatefs run <name>` process is running. There is no hidden `gatefsd`, no automatic daemon startup, and no global `list` command.

This keeps ownership local and obvious:

```sh
gatefs run demo
```

Stopping that foreground process, or running `gatefs demo destroy`, drops all in-memory state.

## No configuration file by design

`gatefs` intentionally does not use a persistent configuration file format. Each integration should initialize the filesystem view and policy explicitly, usually from a small project-specific wrapper script.

This is a design choice. The expected policy shape depends heavily on the surrounding tool, target workflow, visible paths, writable compatibility shims, and protected operations. A general configuration format would tend to grow new sections and compatibility flags as the project gains features. Instead, `gatefs` keeps the durable interface as commands and lets each caller compose those commands in ordinary scripts.

For example, an integration script can:

1. start `gatefs run <name>`;
2. mount selected host paths into the sandbox namespace;
3. hide broad areas that should not be visible;
4. add `protect-*` and `bypass-*` rules;
5. attach the FUSE view;
6. run the target process inside an existing sandbox/container boundary.

## Overlay and hide behavior

Mappings are added with:

```sh
gatefs demo mount <local_path> <sandbox_path>
```

Later mappings overlay earlier mappings, similar to mounts. Intermediate sandbox directories that do not exist in the underlying local filesystems are virtual, in-memory directories.

Hide a sandbox subtree with:

```sh
gatefs demo hide /path/in/sandbox
```

A hide rule removes that path and descendants from visibility until a newer mapping covers that path again.
