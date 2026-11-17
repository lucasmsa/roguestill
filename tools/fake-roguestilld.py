#!/usr/bin/env python3
"""Serve canned Status JSON on a Unix socket. Usage: fake-roguestilld.py <socket> [scenario]
Scenarios: ok, warning, exceeded, degraded, future, cycle (default, advances every 5s)."""
import json, os, socket, sys, time

RULE = {"name": "noita", "target": "noita.exe", "used_secs": 0, "remaining_secs": 1800,
        "runs_today": 0, "degraded": False, "state": "ok", "blocked_seq": 0}
SCENARIOS = {
    "ok": {"v": 1, "rules": [{**RULE, "used_secs": 600, "remaining_secs": 1200, "runs_today": 2}]},
    "warning": {"v": 1, "rules": [{**RULE, "used_secs": 1790, "remaining_secs": 10, "runs_today": 3, "state": "warning"}]},
    "exceeded": {"v": 1, "rules": [{**RULE, "used_secs": 1800, "remaining_secs": 0, "runs_today": 3, "state": "exceeded", "blocked_seq": 1}]},
    "degraded": {"v": 1, "rules": [{**RULE, "used_secs": 300, "remaining_secs": 1500, "degraded": True}]},
    "future": {"v": 2, "rules": []},
}
ORDER = ["ok", "warning", "exceeded", "degraded"]

def current(name, start):
    if name != "cycle":
        return SCENARIOS[name]
    return SCENARIOS[ORDER[int((time.time() - start) // 5) % len(ORDER)]]

path, name = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else "cycle")
if os.path.exists(path):
    os.remove(path)
server = socket.socket(socket.AF_UNIX)
server.bind(path)
os.chmod(path, 0o600)
server.listen()
start = time.time()
while True:
    conn, _ = server.accept()
    conn.sendall(json.dumps(current(name, start)).encode() + b"\n")
    conn.close()
