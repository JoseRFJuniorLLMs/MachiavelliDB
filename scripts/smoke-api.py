#!/usr/bin/env python3
"""Native process/recovery smoke using only the Linux Python standard library."""
import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

binary = pathlib.Path(sys.argv[1]).resolve()
token = "linux-smoke-token-" + "7" * 32
root = pathlib.Path(tempfile.mkdtemp(prefix="machiavelli-smoke-", dir=os.environ.get("TMPDIR", "/mnt/d/tmp")))
log = root / "server.log"
env = dict(os.environ, MACHIAVELLI_TOKEN=token)

def request(base, path, body=None, auth=True, expected=200):
    data = None if body is None else json.dumps(body).encode()
    headers = {"Content-Type": "application/json"}
    if auth:
        headers["Authorization"] = "Bearer " + token
    req = urllib.request.Request(base + path, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=20) as response:
            code, value = response.status, response.read()
    except urllib.error.HTTPError as error:
        code, value = error.code, error.read()
    assert code == expected, (path, code, value)
    return json.loads(value) if value else None

def read_log():
    # drvfs (/mnt/d) can briefly answer ENODATA while the writer truncates.
    try:
        return log.read_text()
    except OSError:
        return ""

def start():
    output = log.open("w")
    process = subprocess.Popen([str(binary / "machiavelli-server"), str(root / "data"), "0"], env=env, stdout=output, stderr=output)
    try:
        for _ in range(400):
            text = read_log()
            match = re.search(r"listening on (127\.0\.0\.1:\d+)", text)
            if match:
                return process, output, "http://" + match.group(1)
            if process.poll() is not None:
                raise AssertionError(text)
            time.sleep(0.05)
        raise AssertionError("server startup timed out: " + read_log())
    except BaseException:
        process.kill()
        process.wait()
        output.close()
        raise

def node(identifier, q):
    return {
        "meta": {
            "id": identifier, "virtu": 0.5, "fortuna_exposure": 0.1, "strategic_mass": 1.0,
            "hamiltonian_energy": 0.0, "nash_reach_prob": 0.0, "cumulative_regret": 0.0,
            "vcg_credit_balance": 0.0, "posture": "Expanding", "fog_level": 0, "alliance_cluster": 0,
            "dike_count": 0, "parent_nietzsche_id": str(uuid.UUID(int=0)), "last_heraclitus_lsn": 0,
            "created_at_hlc": 1, "betrayal_horizon_secs": 3600,
        },
        "phase": {"q": q, "p": [0.7], "c": []},
    }

process, output, base = start()
try:
    assert request(base, "/health", auth=False)["sequence"] == 0
    request(base, "/nodes", auth=False, expected=401)
    identifier, other = str(uuid.uuid4()), str(uuid.uuid4())
    assert request(base, "/nodes", node(identifier, [0.3]))["sequence"] == 1
    assert request(base, "/nodes/" + identifier)["phase"]["q"] == [0.3]
    assert request(base, "/pql", {"query": "PRINCE KUHN ITERATIONS 20000 DEADLINE 1000"})["exploitability"] < 1e-3
    rollout = request(base, "/pql", {"query": f"WARGAME {identifier} STEPS 1000 DT 0.001 DEADLINE 1000"})
    assert rollout["persisted"] is False and rollout["steps"] == 1000
    request(base, "/pql", {"query": f"BETRAY {identifier}"}, expected=409)
    mcts = request(base, "/pql", {"query": f"PRINCE MCTS {identifier} ITERATIONS 512 HORIZON 4 SEED 42 DEADLINE 1000"})
    assert mcts["persisted"] is False and mcts["source_sequence"] == 1 and mcts["search"]["iterations"] == 512
    assert mcts["search"]["selected_action"] in ("hold", "damp", "retreat")
    request(base, "/pql", {"query": f"PRINCE MCTS {uuid.uuid4()} ITERATIONS 1 HORIZON 1 SEED 0"}, expected=404)
    request(base, "/nodes", node(other, []), expected=422)
    assert request(base, "/nodes/" + identifier)["phase"]["q"] == [0.3]
    assert request(base, "/health", auth=False)["sequence"] == 1
    batch = {"expected_sequence": 1, "operations": [{"operation": "put", "value": node(other, [0.4])}]}
    assert request(base, "/transactions", batch)["sequence"] == 2
    request(base, "/transactions", batch, expected=409)
    request(base, "/transactions", {"expected_sequence": 2, "operations": [{"operation": "delete", "value": "00000000-0000-0000-0000-000000000001"}]}, expected=409)
    strategic = request(base, "/strategic?at_sequence=2")
    assert strategic["sequence"] == 2 and strategic["records"] == []
    assert request(base, f"/nodes/{other}/adjacency")["adjacency"] == []
    # Without HERACLITUS_ENDPOINT tactical actions are refused explicitly.
    request(base, "/actions", {}, expected=503)
    # SIGKILL proves recovery without relying on graceful shutdown.
    process.kill()
    process.wait(timeout=10)
    output.close()
    process, output, base = start()
    assert request(base, "/health", auth=False)["sequence"] == 2
    assert request(base, "/nodes/" + other)["phase"]["q"] == [0.4]
    process.send_signal(signal.SIGINT)
    process.wait(timeout=15)
    output.close()
    cli = str(binary / "machiavelli")
    subprocess.run([cli, str(root / "data"), "delete", identifier], check=True, capture_output=True)
    stats = json.loads(subprocess.run([cli, str(root / "data"), "stats"], check=True, capture_output=True).stdout)
    assert stats["nodes"] == 1 and stats["sequence"] == 3, stats
    print(json.dumps({"status": "pass", "backend": "rocksdb", "runtime": "linux", "evidence": str(root)}))
finally:
    if process.poll() is None:
        process.send_signal(signal.SIGINT)
        process.wait(timeout=15)
    output.close()
