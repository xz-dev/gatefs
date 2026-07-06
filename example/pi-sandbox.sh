#!/usr/bin/env bash
set -euo pipefail

# Minimal demo wrapper: run `pi` inside a bubblewrap root backed by sandboxfs.
#
# Shape of the sandboxfs view:
#   - start from the host root (`sandboxfs mount / /`);
#   - hide /home, so other users' /home details are not visible;
#   - hide $HOME, then re-expose only $PWD and $HOME/.pi;
#   - re-expose every existing PATH directory and protect writes on each exposed
#     tree;
#   - use bwrap to make the sandboxfs attach point the process root.
#
# bwrap inherits the caller's environment by default. This wrapper only replaces
# PATH with the sanitized list of PATH directories that were mounted into the
# sandboxfs view. All arguments are passed through to pi unchanged.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
HOST_CWD=$(pwd -P)
HOST_HOME=${HOME:?HOME must be set}

require_executable() {
    local name=$1
    if ! command -v -- "$name" >/dev/null 2>&1; then
        printf 'pi-sandbox: required command not found: %s\n' "$name" >&2
        exit 127
    fi
}

resolve_sandboxfs() {
    if [[ -n ${SANDBOXFS_BIN:-} ]]; then
        printf '%s\n' "$SANDBOXFS_BIN"
    elif [[ -x "$SCRIPT_DIR/../target/debug/sandboxfs" ]]; then
        printf '%s\n' "$SCRIPT_DIR/../target/debug/sandboxfs"
    elif [[ -x "$SCRIPT_DIR/../target/release/sandboxfs" ]]; then
        printf '%s\n' "$SCRIPT_DIR/../target/release/sandboxfs"
    elif command -v sandboxfs >/dev/null 2>&1; then
        command -v sandboxfs
    else
        printf 'pi-sandbox: sandboxfs not found. Build it first or set SANDBOXFS_BIN.\n' >&2
        exit 127
    fi
}

clean_abs_path() {
    local path=$1
    if [[ $path != /* ]]; then
        path=$HOST_CWD/$path
    fi
    while [[ $path == *///* ]]; do
        path=${path//\/\//\/}
    done
    if [[ $path != / ]]; then
        path=${path%/}
    fi
    printf '%s\n' "$path"
}

canonical_dir() {
    local path=$1
    (cd -- "$path" && pwd -P)
}

canonical_path() {
    local path=$1
    readlink -f -- "$path" 2>/dev/null || printf '%s\n' "$path"
}

join_path() {
    local IFS=:
    printf '%s' "$*"
}

BWRAP_BIN=${BWRAP_BIN:-bwrap}
require_executable "$BWRAP_BIN"

SANDBOXFS_BIN=$(resolve_sandboxfs)
PI_BIN=${PI_BIN:-}
if [[ -z $PI_BIN ]]; then
    require_executable pi
    PI_BIN=$(command -v pi)
elif [[ $PI_BIN != /* ]]; then
    require_executable "$PI_BIN"
    PI_BIN=$(command -v "$PI_BIN")
fi

TMP_ROOT=$(mktemp -d -p /tmp pi-sandbox.XXXXXXXXXX)
RUNTIME_DIR=$TMP_ROOT/run
LOG_DIR=$TMP_ROOT/logs
ATTACH_DIR=$TMP_ROOT/root
SCAFFOLD_ROOT=$TMP_ROOT/scaffold
SESSION_NAME=pi-sandbox-$$
SESSION_PID=

mkdir -p -- "$RUNTIME_DIR" "$LOG_DIR" "$ATTACH_DIR" "$SCAFFOLD_ROOT"

sf() {
    SANDBOXFS_RUNTIME_DIR=$RUNTIME_DIR \
    SANDBOXFS_LOG_DIR=$LOG_DIR \
        "$SANDBOXFS_BIN" "$SESSION_NAME" "$@"
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ -n ${SESSION_PID:-} ]]; then
        sf destroy >/dev/null 2>&1 || true
        wait "$SESSION_PID" 2>/dev/null || true
    fi
    if [[ ${PI_SANDBOX_KEEP:-0} == 1 ]]; then
        printf 'pi-sandbox: kept temporary directory: %s\n' "$TMP_ROOT" >&2
    else
        rm -rf -- "$TMP_ROOT"
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

SANDBOXFS_RUNTIME_DIR=$RUNTIME_DIR \
SANDBOXFS_LOG_DIR=$LOG_DIR \
    "$SANDBOXFS_BIN" run "$SESSION_NAME" \
    >"$TMP_ROOT/sandboxfs-run.stdout" \
    2>"$TMP_ROOT/sandboxfs-run.stderr" &
SESSION_PID=$!

SOCKET=$RUNTIME_DIR/$SESSION_NAME.sock
for _ in {1..100}; do
    if [[ -S $SOCKET ]]; then
        break
    fi
    if ! kill -0 "$SESSION_PID" 2>/dev/null; then
        printf 'pi-sandbox: sandboxfs run exited early\n' >&2
        sed -n '1,120p' "$TMP_ROOT/sandboxfs-run.stderr" >&2 || true
        exit 1
    fi
    sleep 0.05
done
if [[ ! -S $SOCKET ]]; then
    printf 'pi-sandbox: timed out waiting for sandboxfs socket: %s\n' "$SOCKET" >&2
    sed -n '1,120p' "$TMP_ROOT/sandboxfs-run.stderr" >&2 || true
    exit 1
fi

declare -A SCAFFOLD_MOUNTS=()
declare -A ADDED_TREES=()
declare -A ADDED_FILES=()
BWRAP_RO_BINDS=()

protect_tree_pattern() {
    local path=$1
    if [[ $path == / ]]; then
        printf '/**\n'
    else
        printf '%s/**\n' "$path"
    fi
}

add_scaffold_mount() {
    local sandbox_path=$1
    [[ -n ${SCAFFOLD_MOUNTS[$sandbox_path]+set} ]] && return 0
    local local_path=$SCAFFOLD_ROOT${sandbox_path}
    mkdir -p -- "$local_path"
    sf mount "$local_path" "$sandbox_path"
    SCAFFOLD_MOUNTS[$sandbox_path]=1
}

ensure_hidden_path_ancestors() {
    local target=$1
    local needs_scaffold=0
    local accum=
    local -a parts=()

    case "$target" in
        /home|/home/*) needs_scaffold=1 ;;
    esac
    if [[ $target == "$HOST_HOME" || $target == "$HOST_HOME"/* ]]; then
        needs_scaffold=1
    fi
    [[ $needs_scaffold == 1 ]] || return 0

    IFS=/ read -r -a parts <<< "${target#/}"
    for ((i = 0; i + 1 < ${#parts[@]}; i++)); do
        [[ -z ${parts[$i]} ]] && continue
        accum=$accum/${parts[$i]}
        add_scaffold_mount "$accum"
    done
}

add_write_protected_tree() {
    local local_path=$1
    local sandbox_path=$2
    sandbox_path=$(clean_abs_path "$sandbox_path")

    if [[ ! -d $local_path ]]; then
        printf 'pi-sandbox: warning: skipping missing directory: %s\n' "$local_path" >&2
        return 0
    fi
    [[ -n ${ADDED_TREES[$sandbox_path]+set} ]] && return 0

    ensure_hidden_path_ancestors "$sandbox_path"
    sf mount "$local_path" "$sandbox_path"
    sf protect-write "$(protect_tree_pattern "$sandbox_path")"
    ADDED_TREES[$sandbox_path]=1
}

add_write_protected_file() {
    local local_path=$1
    local sandbox_path=$2
    sandbox_path=$(clean_abs_path "$sandbox_path")

    if [[ ! -f $local_path ]]; then
        return 0
    fi
    [[ -n ${ADDED_FILES[$sandbox_path]+set} ]] && return 0

    sf mount "$local_path" "$sandbox_path"
    sf protect-write "$sandbox_path"
    ADDED_FILES[$sandbox_path]=1
}

add_symlink_dir_mount() {
    local sandbox_path=$1
    if [[ -L $sandbox_path && -d $sandbox_path ]]; then
        add_write_protected_tree "$(canonical_dir "$sandbox_path")" "$sandbox_path"
    fi
}

add_bwrap_ro_bind_dir() {
    local source_path=$1
    local sandbox_path=$2
    if [[ -d $source_path ]]; then
        BWRAP_RO_BINDS+=(--ro-bind "$source_path" "$sandbox_path")
    fi
}

# Base view: root redirect, with home details hidden until explicitly re-added.
sf mount / /
sf hide /home
sf hide "$HOST_HOME"

add_write_protected_tree "$HOST_CWD" "$HOST_CWD"

PI_HOME_DIR=$HOST_HOME/.pi
if [[ -d $PI_HOME_DIR ]]; then
    add_write_protected_tree "$(canonical_dir "$PI_HOME_DIR")" "$PI_HOME_DIR"
else
    printf 'pi-sandbox: warning: %s does not exist; pi config may be unavailable\n' "$PI_HOME_DIR" >&2
fi

# sandboxfs currently lacks readlink support. Mount common symlinked executable
# and loader paths as real directories so shebangs and dynamic executables can
# start inside the sandboxfs root.
for compat_dir in /bin /sbin /lib /lib64 /usr/lib64; do
    add_symlink_dir_mount "$compat_dir"
done

# Dynamic library sonames are often symlinks. These read-only bwrap overlays are
# execution support, not part of the sandboxfs write-authorization surface.
for lib_dir in /usr/lib /usr/lib64 /lib /lib64 /lib/x86_64-linux-gnu /usr/lib/x86_64-linux-gnu; do
    if [[ -d $lib_dir ]]; then
        add_bwrap_ro_bind_dir "$(canonical_dir "$lib_dir")" "$lib_dir"
    fi
done

# Re-expose every existing PATH directory and use a sanitized absolute PATH
# inside bwrap. All other environment variables are inherited from the caller.
declare -A PATH_SEEN=()
SANDBOX_PATH_PARTS=()
IFS=: read -r -a HOST_PATH_PARTS <<< "${PATH:-}"
for entry in "${HOST_PATH_PARTS[@]}"; do
    if [[ -z $entry ]]; then
        entry=$HOST_CWD
    elif [[ $entry != /* ]]; then
        entry=$HOST_CWD/$entry
    fi
    entry=$(clean_abs_path "$entry")
    if [[ ! -d $entry ]]; then
        continue
    fi
    [[ -n ${PATH_SEEN[$entry]+set} ]] && continue
    PATH_SEEN[$entry]=1
    SANDBOX_PATH_PARTS+=("$entry")
    add_write_protected_tree "$(canonical_dir "$entry")" "$entry"
done
SANDBOXED_PATH=$(join_path "${SANDBOX_PATH_PARTS[@]}")

PI_BIN_PARENT=$(clean_abs_path "$(dirname -- "$PI_BIN")")
if [[ -d $PI_BIN_PARENT ]]; then
    add_write_protected_tree "$(canonical_dir "$PI_BIN_PARENT")" "$PI_BIN_PARENT"
fi

# On this host the user's pi wrapper defaults to /bin/pi, and /bin/pi is a
# symlink. Overlay the resolved target at /bin/pi so the wrapper can keep its
# normal default without needing an environment override.
if [[ -e /bin/pi ]]; then
    PI_REAL_TARGET=$(canonical_path /bin/pi)
    add_write_protected_file "$PI_REAL_TARGET" /bin/pi
fi

sf attach "$ATTACH_DIR"

cat >&2 <<EOF
pi-sandbox: sandboxfs session is running
  session: $SESSION_NAME
  runtime: $RUNTIME_DIR
  logs:    $LOG_DIR
  mount:   $ATTACH_DIR

If pi blocks on a protected write, resolve it from another terminal with:
  SANDBOXFS_RUNTIME_DIR=$RUNTIME_DIR SANDBOXFS_LOG_DIR=$LOG_DIR sandboxfs-access-tui $SESSION_NAME

Set PI_SANDBOX_KEEP=1 to keep the temporary directory after exit.
EOF

"$BWRAP_BIN" \
    --die-with-parent \
    --bind "$ATTACH_DIR" / \
    "${BWRAP_RO_BINDS[@]}" \
    --dev-bind /dev /dev \
    --proc /proc \
    --chdir "$HOST_CWD" \
    --setenv PATH "$SANDBOXED_PATH" \
    "$PI_BIN" "$@"
