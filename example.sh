#!/bin/bash

wsm="./sqlite2cel2jsons.wasm"

hdir="$PWD"
gdir="/guest.d"
iname="example.sqlite.db3"

hpath="${hdir}/${iname}"
ipath="${gdir}/${iname}"

prog_limit=65536

sql='SELECT rowid, timestamp, severity, message FROM log ORDER BY rowid'

run_wasmtime() {
	cat /dev/stdin |
		wasmtime \
			run \
			--env ENV_SQL="${sql}" \
			--env ENV_SQLITE_DB_FILE_PATH="${ipath}" \
			--env ENV_MAX_PROG_SIZE=${prog_limit} \
			--dir "${hdir}::${gdir}" \
			"${wsm}"
}

run_wasi() {
	run_wasmtime
}

input1() {
	echo '{
    "doubled": 2*id,
    "timestam_str": timestamp,
    "is_world": message.matches("^.*world.*$"),
    "severity_no": (
      (severity.startsWith("trace")) ? 1
      : (severity.startsWith("debug")) ? 5
      : (severity.startsWith("info")) ? 9
      : (severity.startsWith("warn")) ? 13
      : (severity.startsWith("error")) ? 17
      : (severity.startsWith("fatal")) ? 21
      : 0
    )
  }'
}

inst(){
  inst1="INSERT INTO log(timestamp, severity, message)"
  inst2=$1

  echo "${inst1} ${inst2}" | sqlite3 "${hpath}"
}

geninput() {
	echo generating input sqlite file...

	echo "
    CREATE TABLE IF NOT EXISTS log(
      id INTEGER PRIMARY KEY,
      timestamp TEXT NOT NULL,
      severity TEXT NOT NULL,
      message TEXT NOT NULL
    )
  " | sqlite3 "${hpath}"

  inst "VALUES('2026-10-27T01:23:45Z', 'trace1', 'helo wrld')"
  inst "VALUES('2026-10-27T01:23:45Z', 'debug2', 'hellowrld')"
  inst "VALUES('2026-10-27T01:23:45Z', 'warn3',  'heloworld')"
  inst "VALUES('2026-10-27T01:23:45Z', 'info4',  'hello    ')"
  inst "VALUES('2026-10-27T01:23:45Z', 'error5', '     wrld')"
  inst "VALUES('2026-10-27T01:23:45Z', 'fatal6', '    world')"
}

test -f "${hpath}" || geninput

echo dump sqlite
echo "${sql}" | sqlite3 -markdown "${hpath}"
echo

input1 | run_wasi | jq -c
