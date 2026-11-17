"""Exercise target preparation through real WASM in a private RustGIS CLI host."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]
CLI = "C:/Program Files/RustGIS/rustgis-cli.exe"
run = Path(os.environ["QFK_TEST_ROOT"]) if "QFK_TEST_ROOT" in os.environ else ROOT / "test-artifacts" / ("auto-" + uuid.uuid4().hex)
run.mkdir(parents=True, exist_ok=True)
sources = []
for name, text in [("first", "original"), ("second", "second-original"), ("large", "x" * 4001), ("invalid", "duplicate")]:
    path = run / (name + ".geojson")
    path.write_text(json.dumps({"type": "FeatureCollection", "features": [
        {"type": "Feature", "id": f"{name}-{i}", "geometry": {"type": "Point", "coordinates": [i, 0]},
         "properties": {"uid": 1 if name == "invalid" else i + 1, "label": text}} for i in range(3)
    ]}), encoding="utf-8")
    sources.append(path)
hashes = [hashlib.sha256(p.read_bytes()).hexdigest() for p in sources]
manifest = json.loads((ROOT / "plugin.json").read_text(encoding="utf-8"))
assert not any(i.get("command") == "capture" for i in manifest["panel"]["items"])
assert not any("capture" in g["items"] for g in manifest["tab"]["groups"])
args = [CLI, "mcp", "--yes", "--no-plugins", "--plugin", str(ROOT)]
for path in sources:
    args.extend(["--data", str(path)])
env = dict(os.environ, RUSTGIS_PLUGINS_DIR=str(run / "plugins"),
           RUSTGIS_CONFIG_DIR=str(run / "config"), RUSTGIS_APP_DATA_DIR=str(run / "data"),
           RUSTGIS_CACHE_DIR=str(run / "cache"))
process = subprocess.Popen(args, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, text=True, encoding="utf-8")
counter = 0


def call(command, params=None, error=False):
    global counter
    counter += 1
    process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": counter, "method": "tools/call",
        "params": {"name": "run_command", "arguments": {"command": command, "params": params or {}}}}) + "\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())["result"]
    text = response["content"][0]["text"]
    assert bool(response.get("isError")) == error, (command, text)
    return text if error else json.loads(text)


def plugin(command, params=None, error=False):
    return call("plugin:quick-field-keys/" + command, params, error)


try:
    state = plugin("state")
    assert state["panel"]["values"]["layer"] == "__choose__"
    layers = {layer["name"]: layer["id"] for layer in state["layers"]}
    first, second, large, invalid = [layers[n] for n in ["first", "second", "large", "invalid"]]
    prepared = plugin("choose-layer", {"panel": {"layer": str(first)}})
    session = json.loads(prepared["panel"]["values"]["session"]["payload"])
    assert session["layer"] == first and len(session["captured"]) == 3
    assert prepared["panel"]["values"]["config"]["identity"] == "uid"
    assert prepared["panel"]["items"]["previous-view"]["enabled"] is False
    assert "attributes" not in prepared["panel"]["values"]
    assert "locks" not in prepared["panel"]["values"]
    assert plugin("next")["current"]["properties"]["uid"] == 2
    print("PASS top layer selection automatically prepares all rows without Hold or selection")

    call("map.setScale", {"scale":25000})
    scale = call("info.project")["scale"]
    plugin("first", {"panel":{"view":{"zoom-mode":"keep-scale"}}})
    project = call("info.project")
    assert abs(project["center"]["lon"]) < 0.001
    assert abs(project["scale"]-scale) <= max(2,scale*0.0001)
    plugin("next")
    project = call("info.project")
    assert abs(project["center"]["lon"]-1) < 0.001
    assert abs(project["scale"]-scale) <= max(2,scale*0.0001)
    print("PASS keep-scale navigation changes object center without changing map scale")

    plugin("last", {"panel":{"view":{"zoom-mode":"fixed-scale","scale":5000}}})
    project = call("info.project")
    assert abs(project["center"]["lon"]-2) < 0.001 and abs(project["scale"]-5000)<=2
    print("PASS fixed-scale navigation centers the object at the requested scale")

    retained = plugin("state")["capture"]["payload"]
    plugin("first", {"panel":{"view":{"zoom-mode":"fixed-scale","scale":0}}}, error=True)
    assert plugin("state")["capture"]["payload"] == retained
    assert call("info.project") == project
    print("PASS invalid fixed scale leaves feature position, selection and view unchanged")

    plugin("first", {"panel":{"view":{"zoom-mode":"off"}}})
    unchanged = call("info.project")
    assert unchanged["center"]==project["center"] and unchanged["scale"]==project["scale"]
    plugin("zoom-current", {"panel":{"view":{"zoom-mode":"feature"}}})
    fitted = call("info.project")
    assert abs(fitted["center"]["lon"])<0.001 and fitted["scale"]!=project["scale"]
    plugin("first", {"panel":{"view":{"zoom-mode":"off"}}})
    print("PASS off leaves the view untouched; fit-feature actually fits the current object")

    # Given: a non-contiguous selection and a rotated, tilted camera.
    call("select.rows", {"layer":first,"rows":[0,2],"mode":"new"})
    call("select.rows", {"layer":second,"rows":[1],"mode":"add"})
    call("map.goToXY", {"lon":127.125,"lat":37.55,"scale":25000,"bearing":31,"pitch":45})
    previous_camera = call("info.project")
    original_cursor = json.loads(plugin("state")["capture"]["payload"])["cursor"]
    # When: navigate, edit, and restore the previous selection and view.
    moved = plugin("next", {"panel":{"view":{"zoom-mode":"feature"}}})
    other_selection = call("table.statistics", {"layer":second,"field":"uid","selected":True})
    assert other_selection["count"] == 1 and other_selection["min"] == 2,other_selection
    saved_view = json.loads(moved["panel"]["values"]["session"]["payload"])["previous_view"]
    assert moved["panel"]["items"]["previous-view"]["enabled"] is True
    assert [(f["row"],f["identity"]) for f in saved_view["selected"]] == [(0,1),(2,3)]
    captured = json.loads(moved["panel"]["values"]["session"]["payload"])["captured"]
    assert [f["id"] for f in saved_view["selected"]] == [captured[0]["id"],captured[2]["id"]]
    assert [f["id"] for f in saved_view["selected"]] == [1,3]
    plugin("configure", {"layer":str(first),"identity":"uid","slot":"1","field":"label","value":"retained-after-view-restore"})
    # Configure prepares a fresh target, so establish the navigation history again.
    call("select.rows", {"layer":first,"rows":[0,2],"mode":"new"})
    call("select.rows", {"layer":second,"rows":[1],"mode":"add"})
    call("map.goToXY", {"lon":127.125,"lat":37.55,"scale":25000,"bearing":31,"pitch":45})
    plugin("next", {"panel":{"view":{"zoom-mode":"feature"}}})
    plugin("apply-1")
    restored = plugin("previous-view")
    other_selection = call("table.statistics", {"layer":second,"field":"uid","selected":True})
    assert other_selection["count"] == 1 and other_selection["min"] == 2,other_selection
    # Then: exact selected identities and camera return, but the edit remains.
    stats = call("table.statistics", {"layer":first,"field":"uid","selected":True})
    assert stats["count"] == 2 and stats["min"] == 1 and stats["max"] == 3 and stats["sum"] == 4,stats
    restored_camera = call("info.project")
    for axis in ["lon","lat"]:
        assert abs(restored_camera["center"][axis]-previous_camera["center"][axis])<1e-6
    for key in ["scale","bearing","pitch"]:
        assert abs(restored_camera[key]-previous_camera[key])<1e-3,(key,restored_camera,previous_camera)
    assert restored_camera["projection"] == previous_camera["projection"]
    payload = json.loads(restored["panel"]["values"]["session"]["payload"])
    assert payload["cursor"] == original_cursor
    assert payload["captured"][1]["properties"]["label"] == "retained-after-view-restore"
    assert "previous_view" not in payload
    assert restored["panel"]["items"]["previous-view"]["enabled"] is False
    plugin("previous-view", error=True)
    print("PASS previous-view restores exact noncontiguous IDs, center, scale, rotation and tilt without undoing edits")

    # Given: a new empty selection, then a navigation step.
    call("select.rows", {"layer":first,"rows":[],"mode":"new"})
    plugin("next", {"panel":{"view":{"zoom-mode":"off"}}})
    # When
    plugin("previous-view")
    # Then
    assert call("table.statistics", {"layer":first,"field":"uid","selected":True})["rows"] == 0
    print("PASS previous-view restores an empty selection instead of selecting the current object")

    # Given: a saved view followed by a projection change.
    plugin("next")
    call("map.projection", {"type":"globe"})
    before_rejected_restore = call("info.project")
    before_payload = plugin("state")["capture"]["payload"]
    # When
    plugin("previous-view", error=True)
    # Then
    assert call("info.project") == before_rejected_restore
    assert plugin("state")["capture"]["payload"] == before_payload
    call("map.projection", {"type":"mercator"})
    plugin("previous-view")
    print("PASS projection changes reject previous-view atomically and preserve the saved history")
    plugin("capture")

    settings = {"layer": str(first), "identity": "uid", "slot": "1", "field": "label", "value": "preset"}
    configured = plugin("configure", settings)
    assert json.loads(configured["panel"]["values"]["session"]["payload"])["layer"] == first
    applied = plugin("apply-1")
    assert applied["current"]["properties"]["label"] == "preset"
    print("PASS settings automatically rebuild the target and presets apply without Capture")

    draft = applied["panel"]["values"]["attributes"]
    draft = dict(draft)
    label_id = next(f["id"] for f in applied["panel"]["items"]["attributes"]["fields"] if f["label"] == "label")
    draft[label_id] = "pending"
    same = plugin("choose-layer", {"panel": {"layer": str(first), "attributes": draft}})
    assert "attributes" not in same["panel"]["values"] and "items" not in same["panel"]
    edited = plugin("edit-current", {"panel": {"attributes": draft}})
    assert edited["current"]["properties"]["label"] == "pending"
    print("PASS reselecting the same target does not replace the pending draft")

    moved = plugin("choose-layer", {"panel": {"layer": str(second)}})
    assert json.loads(moved["panel"]["values"]["session"]["payload"])["layer"] == second
    assert all(moved["panel"]["values"]["config"][f"preset{i}"] == "{}" for i in range(1, 10))
    plugin("apply-1", error=True)
    plugin("edit-current", {"panel": {"attributes": draft}})
    assert plugin("first")["current"]["properties"]["label"] == "second-original"
    print("PASS layer change resets foreign presets and stale form cannot edit the new layer")

    before = plugin("state")["capture"]["payload"]
    project = call("info.project")
    plugin("choose-layer", {"panel": {"layer": str(large)}}, error=True)
    assert plugin("state")["capture"]["payload"] == before
    assert call("info.project") == project
    plugin("edit-current", {"panel": {"layer": str(large), "attributes": draft}}, error=True)
    assert call("info.project") == project
    print("PASS oversized automatic target preparation is atomic; mismatched top target cannot write")

    plugin("choose-layer", {"panel": {"layer": str(invalid)}}, error=True)
    assert plugin("state")["capture"]["payload"] == before
    assert call("info.project") == project
    print("PASS no unique identity rejects automatic preparation without replacing the old target")

    plugin("configure", dict(settings, layer=str(first)), error=True)
    assert plugin("state")["capture"]["payload"] == before
    print("PASS settings cannot bypass the top layer selector")

    returned = plugin("choose-layer", {"panel": {"layer": str(first)}})
    assert returned["current"]["properties"]["label"] == "pending"
    assert [hashlib.sha256(p.read_bytes()).hexdigest() for p in sources] == hashes
    print("PASS previous layer edits retained; all original source files unchanged")
finally:
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode == 0, process.stderr.read()
