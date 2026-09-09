#!/usr/bin/env bash
set -u

mkdir -p /var/log/zhiqiaoapi

ZHIQIAOAPI_BIN="${ZHIQIAOAPI_BIN:-./target/release/zhiqiaoapi}"
ZHIQIAOAPI_SCHEDULER_BIN="${ZHIQIAOAPI_SCHEDULER_BIN:-./target/release/zhiqiaoapi-scheduler}"

if [ "${ZHIQIAOAPI_SCHEDULER_LOG_STDOUT:-0}" = "1" ]; then
  "$ZHIQIAOAPI_SCHEDULER_BIN" &
else
  "$ZHIQIAOAPI_SCHEDULER_BIN" >>/var/log/zhiqiaoapi/scheduler.log 2>>/var/log/zhiqiaoapi/scheduler-error.log &
fi
scheduler_pid=$!

"$ZHIQIAOAPI_BIN" &
backend_pid=$!

shutdown() {
  kill "$backend_pid" "$scheduler_pid" 2>/dev/null || true
}

trap shutdown TERM INT

wait -n "$backend_pid" "$scheduler_pid"
status=$?

shutdown
wait || true
exit "$status"
