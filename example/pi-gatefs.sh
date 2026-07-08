#!/usr/bin/env bash
set -euo pipefail

# Minimal demo wrapper: run pi inside a bubblewrap root backed by gatefs.
#
# gatefs is used here to make the agent's filesystem view and operations
# observable, not to provide a strong isolation boundary. The view starts from
# host /, hides /home and $HOME, then re-exposes $HOME/.pi, $HOME/.agents,
# the caller's PATH directories, and the current working directory. Re-exposed
# PATH directories and $HOME/.agents are protected against write and metadata
# changes. The wrapped process inherits the caller's environment; this script
# only sets PATH.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
HOST_CWD=$(pwd -P)
HOST_HOME=${HOME:?HOME must be set}
HOST_TMPDIR=${TMPDIR:-/tmp}
HOST_PATH=${PATH:-}

require_executable() {
    local name=$1
    if ! command -v -- "$name" >/dev/null 2>&1; then
        printf 'pi-gatefs: required command not found: %s\n' "$name" >&2
        exit 127
    fi
}

resolve_gatefs() {
    if [[ -n ${GATEFS_BIN:-} ]]; then
        printf '%s\n' "$GATEFS_BIN"
    elif [[ -x "$SCRIPT_DIR/../target/debug/gatefs" ]]; then
        printf '%s\n' "$SCRIPT_DIR/../target/debug/gatefs"
    elif [[ -x "$SCRIPT_DIR/../target/release/gatefs" ]]; then
        printf '%s\n' "$SCRIPT_DIR/../target/release/gatefs"
    elif command -v gatefs >/dev/null 2>&1; then
        command -v gatefs
    else
        printf 'pi-gatefs: gatefs not found. Build it first or set GATEFS_BIN.\n' >&2
        exit 127
    fi
}

resolve_pi() {
    if [[ -n ${PI_BIN:-} ]]; then
        printf '%s\n' "$PI_BIN"
    elif command -v pi >/dev/null 2>&1; then
        command -v pi
    elif [[ -e /bin/pi ]]; then
        printf '/bin/pi\n'
    elif [[ -e /usr/bin/pi ]]; then
        printf '/usr/bin/pi\n'
    else
        printf 'pi-gatefs: pi not found. Set PI_BIN or install pi.\n' >&2
        exit 127
    fi
}

BWRAP_BIN=${BWRAP_BIN:-bwrap}
require_executable "$BWRAP_BIN"
GATEFS_BIN=$(resolve_gatefs)
PI_BIN=$(resolve_pi)

TMP_ROOT=$(mktemp -d -p "$HOST_TMPDIR" pi-gatefs.XXXXXXXXXX)
RUNTIME_DIR=$TMP_ROOT/run
LOG_DIR=$TMP_ROOT/logs
ATTACH_DIR=$TMP_ROOT/root
SESSION_NAME=pi-gatefs-$$
SESSION_PID=

mkdir -p -- "$RUNTIME_DIR" "$LOG_DIR" "$ATTACH_DIR"

sf() {
    GATEFS_RUNTIME_DIR=$RUNTIME_DIR \
    GATEFS_LOG_DIR=$LOG_DIR \
        "$GATEFS_BIN" "$SESSION_NAME" "$@"
}

protect_readonly_tree() {
    local path=$1
    sf protect-write "$path/"
    sf protect-write "$path/**"
    sf protect-metadata "$path/"
    sf protect-metadata "$path/**"
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ -n ${SESSION_PID:-} ]]; then
        sf destroy >/dev/null 2>&1 || true
        wait "$SESSION_PID" 2>/dev/null || true
    fi
    if [[ ${PI_GATEFS_KEEP:-0} == 1 ]]; then
        printf 'pi-gatefs: kept temporary directory: %s\n' "$TMP_ROOT" >&2
    else
        rm -rf -- "$TMP_ROOT"
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

GATEFS_RUNTIME_DIR=$RUNTIME_DIR \
GATEFS_LOG_DIR=$LOG_DIR \
    "$GATEFS_BIN" run "$SESSION_NAME" \
    >"$TMP_ROOT/gatefs-run.stdout" \
    2>"$TMP_ROOT/gatefs-run.stderr" &
SESSION_PID=$!

SOCKET=$RUNTIME_DIR/$SESSION_NAME.sock
for _ in {1..100}; do
    if [[ -S $SOCKET ]]; then
        break
    fi
    if ! kill -0 "$SESSION_PID" 2>/dev/null; then
        printf 'pi-gatefs: gatefs run exited early\n' >&2
        sed -n '1,120p' "$TMP_ROOT/gatefs-run.stderr" >&2 || true
        exit 1
    fi
    sleep 0.05
done
if [[ ! -S $SOCKET ]]; then
    printf 'pi-gatefs: timed out waiting for gatefs socket: %s\n' "$SOCKET" >&2
    sed -n '1,120p' "$TMP_ROOT/gatefs-run.stderr" >&2 || true
    exit 1
fi

# Base view: root redirect for observability, with home details hidden and only
# the workflow's required user paths re-exposed.
sf mount / /
sf hide /home
sf hide "$HOST_HOME"
sf mount "$HOST_HOME/.pi" "$HOST_HOME/.pi"
if [[ -d "$HOST_HOME/.agents" ]]; then
    sf mount "$HOST_HOME/.agents" "$HOST_HOME/.agents"
    protect_readonly_tree "$HOST_HOME/.agents"
fi
IFS=: read -r -a HOST_PATH_DIRS <<< "$HOST_PATH"
for path_dir in "${HOST_PATH_DIRS[@]}"; do
    [[ -n $path_dir && -d $path_dir && $path_dir = /* ]] || continue
    while [[ $path_dir != / && $path_dir == */ ]]; do
        path_dir=${path_dir%/}
    done
    sf mount "$path_dir" "$path_dir"
    protect_readonly_tree "$path_dir"
done
sf mount "$HOST_CWD" "$HOST_CWD"
sf bypass-write "$HOST_HOME/.pi/agent/settings.json.lock"
sf bypass-metadata "$HOST_HOME/.pi/agent/settings.json.lock"
sf bypass-write "$HOST_HOME/.pi/agent/trust.json.lock"
sf bypass-metadata "$HOST_HOME/.pi/agent/trust.json.lock"

sf attach "$ATTACH_DIR"

cat >&2 <<EOF
pi-gatefs: gatefs session is running
  session: $SESSION_NAME
  runtime: $RUNTIME_DIR
  logs:    $LOG_DIR
  mount:   $ATTACH_DIR

Inspect the gatefs session from another terminal with:
  GATEFS_RUNTIME_DIR=$RUNTIME_DIR GATEFS_LOG_DIR=$LOG_DIR gatefs-access-tui $SESSION_NAME

Set PI_GATEFS_KEEP=1 to keep the temporary directory after exit.
EOF

"$BWRAP_BIN" \
    --die-with-parent \
    --bind "$ATTACH_DIR" / \
    --dev /dev \
    --proc /proc \
    --chdir "$HOST_CWD" \
    --setenv PATH "$HOST_PATH" \
    "$PI_BIN" "$@"
