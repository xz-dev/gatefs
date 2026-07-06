# TODO

## Design xattr handling for sandboxfs

Extended attributes must not be blanket-passthrough. Some xattrs describe the backing host filesystem or host security context rather than the sandboxfs view, so exposing them unchanged can be inaccurate or unsafe. Examples include SELinux/security labels, capabilities, ACL-related attributes, and other `security.*`, `system.*`, or `trusted.*` namespaces.

Before implementing xattr support, design an explicit policy for:

- which xattr namespaces, if any, are visible through sandboxfs;
- whether visible xattrs are raw passthrough, filtered, or synthetic;
- whether xattr reads are metadata probes or protected READ operations;
- whether xattr writes are host passthrough, sandbox-local overrides, or rejected;
- how host filesystem errno values should be preserved when an allowed xattr operation reaches the backing filesystem.

Do this after the basic non-xattr metadata passthrough/override work is complete.
