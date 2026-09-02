#!/usr/bin/env python3
import json
import os
import socket
import sys

runtime_dir = os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")
socket_path = os.path.join(runtime_dir, "truss-0.sock")

try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(socket_path)
    s.sendall(b'{"id": 1, "command": "state.get"}\n')
    data = b""
    while b"\n" not in data:
        chunk = s.recv(4096)
        if not chunk:
            break
        data += chunk
    s.close()

    resp = json.loads(data.split(b"\n")[0])
    state = resp["result"]["State"]
    active_ws = state.get("active_workspace_id", 1)

    ws_blocks = []
    for ws_id in sorted(state.get("workspaces", {}).keys(), key=lambda x: int(x)):
        ws_info = state["workspaces"][ws_id]
        has_windows = len(ws_info.get("windows", [])) > 0
        if int(ws_id) == active_ws:
            ws_blocks.append(f"[{ws_id}]")
        elif has_windows:
            ws_blocks.append(f"*{ws_id}*")
        else:
            ws_blocks.append(f" {ws_id} ")

    active_ws_obj = state.get("workspaces", {}).get(str(active_ws), {})
    focused_id = active_ws_obj.get("focused_window")
    active_window = "desktop"
    if focused_id is not None:
        win = state.get("windows", {}).get(str(focused_id), {})
        active_window = win.get("title") or win.get("app_id") or "window"
        if len(active_window) > 30:
            active_window = active_window[:27] + "..."

    layout = active_ws_obj.get("layout", "master")
    text = f"{' '.join(ws_blocks)}   |   {layout}   |   {active_window}"
    print(json.dumps({"text": text, "tooltip": f"Truss Wayland Compositor\nLayout: {layout}\nWindow: {active_window}"}))
except Exception as e:
    print(json.dumps({"text": f"truss: {e}"}))
