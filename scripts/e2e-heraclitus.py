#!/usr/bin/env python3
"""ALLY end to end against a real, throwaway Heraclitus server (Linux only).

Starts the Heraclitus build given on the command line with its own data
directory, loopback ports and token, never the operator's service, then drives
MachiavelliDB through: authorized ALLY, idempotent replay, conflicting reuse
of an operation ID, Heraclitus outage with fenced writes, MachiavelliDB crash
with a pending operation, and recovery after both processes restart.
"""
import json
import os
import pathlib
import re
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

release = pathlib.Path(sys.argv[1]).resolve()
heraclitus_binary = pathlib.Path(sys.argv[2]).resolve()
root = pathlib.Path(tempfile.mkdtemp(prefix="machiavelli-e2e-", dir=os.environ.get("TMPDIR", "/mnt/d/tmp")))
machiavelli_token = "e2e-machiavelli-" + uuid.uuid4().hex + uuid.uuid4().hex
heraclitus_token = "e2e-heraclitus-" + uuid.uuid4().hex + uuid.uuid4().hex


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


grpc_port, rest_port, console_port = free_port(), free_port(), free_port()
# Inherited HERACLITUS_* variables would silently retarget the test server.
base_env = {k: v for k, v in os.environ.items() if not k.startswith("HERACLITUS_")}
heraclitus_env = dict(
    base_env,
    HERACLITUS_DATA_DIR=str(root / "heraclitus"),
    HERACLITUS_GRPC_ADDR=f"127.0.0.1:{grpc_port}",
    HERACLITUS_REST_ADDR=f"127.0.0.1:{rest_port}",
    # The Platform Console defaults to 0.0.0.0:8080, where the operator's service listens.
    HERACLITUS_AGENT_CONSOLE_ADDR=f"127.0.0.1:{console_port}",
    HERACLITUS_AUTH_TOKEN=heraclitus_token,
)
machiavelli_env = dict(
    base_env,
    MACHIAVELLI_TOKEN=machiavelli_token,
    HERACLITUS_ENDPOINT=f"http://127.0.0.1:{grpc_port}",
    HERACLITUS_TOKEN=heraclitus_token,
)


class Process:
    starts = 0

    def __init__(self, name, command, env):
        # One log per start, so a restart never matches an older line.
        Process.starts += 1
        self.log = root / f"{name}-{Process.starts}.log"
        self.output = self.log.open("w")
        self.process = subprocess.Popen(command, env=env, stdout=self.output, stderr=self.output)

    def read(self):
        # drvfs (/mnt/d) can briefly answer ENODATA while the writer appends.
        try:
            return self.log.read_text()
        except OSError:
            return ""

    def alive(self):
        return self.process.poll() is None

    def stop(self, sig):
        if self.alive():
            self.process.send_signal(sig)
            self.process.wait(timeout=60)
        self.output.close()


def start_heraclitus():
    process = Process("heraclitus", [str(heraclitus_binary)], heraclitus_env)
    deadline = time.time() + 120
    while time.time() < deadline:
        if not process.alive():
            raise AssertionError("Heraclitus exited: " + process.read()[-4000:])
        try:
            socket.create_connection(("127.0.0.1", grpc_port), timeout=1).close()
            return process
        except OSError:
            time.sleep(0.2)
    process.stop(signal.SIGTERM)
    raise AssertionError("Heraclitus startup timed out")


def start_machiavelli():
    process = Process("machiavelli", [str(release / "machiavelli-server"), str(root / "machiavelli"), "0"], machiavelli_env)
    for _ in range(600):
        match = re.search(r"listening on (127\.0\.0\.1:\d+)", process.read())
        if match:
            return process, "http://" + match.group(1)
        if not process.alive():
            raise AssertionError("MachiavelliDB exited: " + process.read()[-4000:])
        time.sleep(0.05)
    process.stop(signal.SIGINT)
    raise AssertionError("MachiavelliDB startup timed out")


def request(base, path, body=None, expected=200):
    data = None if body is None else json.dumps(body).encode()
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + machiavelli_token}
    req = urllib.request.Request(base + path, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            code, value = response.status, response.read()
    except urllib.error.HTTPError as error:
        code, value = error.code, error.read()
    assert code == expected, (path, code, value)
    return json.loads(value) if value else None


def node(identifier):
    return {
        "meta": {
            "id": identifier, "virtu": 0.5, "fortuna_exposure": 0.1, "strategic_mass": 1.0,
            "hamiltonian_energy": 0.0, "nash_reach_prob": 0.0, "cumulative_regret": 0.0,
            "vcg_credit_balance": 0.0, "posture": "Expanding", "fog_level": 0, "alliance_cluster": 0,
            "dike_count": 0, "parent_nietzsche_id": str(uuid.UUID(int=0)), "last_heraclitus_lsn": 0,
            "created_at_hlc": 1, "betrayal_horizon_secs": 3600,
        },
        "phase": {"q": [0.3], "p": [0.7], "c": []},
    }


def ally(operation_id, sequence, source, target):
    return {
        "operation_id": operation_id,
        "expected_sequence": sequence,
        "expires_ms": int(time.time() * 1000) + 120_000,
        "action": {"action": "ally", "from": source, "to": target},
    }


def wait_acknowledged(base, operation_id, seconds=60):
    deadline = time.time() + seconds
    while time.time() < deadline:
        entry = request(base, "/actions/" + operation_id)
        if entry["acknowledged"]:
            return entry
        time.sleep(0.5)
    raise AssertionError(f"{operation_id} still pending after {seconds}s")


def edges(base, identifier):
    return {adj["edge"] for adj in request(base, f"/nodes/{identifier}/adjacency")["adjacency"]}


heraclitus = start_heraclitus()
machiavelli, base = start_machiavelli()
try:
    a, b = str(uuid.uuid4()), str(uuid.uuid4())
    request(base, "/transactions", {"expected_sequence": 0, "operations": [
        {"operation": "put", "value": node(a)}, {"operation": "put", "value": node(b)}]})

    # 1. Authorized ALLY: pending (2), effect + Applied marker (3), remote ACK (4).
    first = ally("e2e-ally-1", 1, a, b)
    entry = request(base, "/actions", first)
    assert entry["acknowledged"] and entry["result"]["applied"], entry
    assert entry["result"]["local_sequence"] == 3, entry
    receipt = entry["receipt"]
    assert receipt["principal"] == "legacy-admin" and receipt["intent"]["operation_id"] == "e2e-ally-1", receipt
    assert len(edges(base, a)) == 1 and edges(base, a) == edges(base, b)
    assert request(base, "/health")["sequence"] == 4

    # 2. Replaying the same request is idempotent; a different body is refused.
    replay = request(base, "/actions", first)
    assert replay == entry, (replay, entry)
    assert request(base, "/health")["sequence"] == 4
    conflicting = dict(first, action={"action": "ally", "from": b, "to": a})
    request(base, "/actions", conflicting, expected=409)

    # 3. Heraclitus outage: the operation stays pending and fences other writes.
    heraclitus.stop(signal.SIGTERM)
    second = ally("e2e-ally-2", 4, b, a)
    pending = request(base, "/actions", second)
    assert not pending["acknowledged"] and pending["result"] is None and pending["receipt"] is None, pending
    assert request(base, "/health")["sequence"] == 5
    request(base, "/transactions", {"expected_sequence": 5, "operations": [
        {"operation": "put", "value": node(str(uuid.uuid4()))}]}, expected=409)

    # 4. MachiavelliDB crashes with the pending entry; both processes restart.
    machiavelli.process.kill()
    machiavelli.process.wait(timeout=10)
    machiavelli.output.close()
    heraclitus = start_heraclitus()
    machiavelli, base = start_machiavelli()
    recovered = wait_acknowledged(base, "e2e-ally-2")
    assert recovered["result"]["applied"] and recovered["result"]["local_sequence"] == 6, recovered
    assert len(edges(base, a)) == 2
    assert request(base, "/health")["sequence"] == 7

    # 5. Fencing lifts once the operation is resolved.
    request(base, "/transactions", {"expected_sequence": 7, "operations": [
        {"operation": "put", "value": node(str(uuid.uuid4()))}]})
    print(json.dumps({
        "status": "pass", "heraclitus": str(heraclitus_binary), "runtime": "linux",
        "intent_lsn": receipt["intent_lsn"], "evidence": str(root),
    }))
finally:
    machiavelli.stop(signal.SIGINT)
    heraclitus.stop(signal.SIGTERM)
