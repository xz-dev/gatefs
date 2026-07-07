# TODO

## Design xattr handling for sandboxfs

Extended attributes must not be blanket-passthrough. Some xattrs describe the backing host filesystem or host security context rather than the sandboxfs view, so exposing them unchanged can be inaccurate or unsafe. Examples include SELinux/security labels, capabilities, ACL-related attributes, and other `security.*`, `system.*`, or `trusted.*` namespaces.

Policy for the first xattr slice:

- xattr operations are thin passthrough to the backing filesystem;
- `getxattr`/`listxattr` are metadata probes, not protected READ operations;
- `setxattr`/`removexattr` are backing-host metadata mutations, because sandboxfs does not manage xattr overlays yet;
- preserve the host filesystem's support and errno behavior once an xattr operation reaches the backing filesystem;
- do not partially filter or synthesize security/SELinux/capability/ACL xattrs in the first slice. Those values can look odd from a sandboxfs view, but host passthrough is at least reproducible. Partial sandboxfs emulation is more likely to produce unstable and surprising behavior.

Future work: revisit whether any xattr namespace should become a sandboxfs-managed override. If that happens, it must be designed as an explicit managed surface like mode/uid/gid/flags/timestamps, not as an ad-hoc filter layered on passthrough.
