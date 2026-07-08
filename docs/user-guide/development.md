# Development checks

Run formatting, tests, and lint checks before submitting changes:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

FUSE behavior tests require `/dev/fuse` and `fusermount3` support:

```sh
GATEFS_RUN_FUSE_TESTS=1 cargo test --test fuse_behavior -- --ignored
```

Stress tests are opt-in:

```sh
GATEFS_RUN_FUSE_TESTS=1 GATEFS_RUN_STRESS_TESTS=1 cargo test --test fuse_behavior stress_multiple_pending_viewers_do_not_consume_request -- --ignored
```
