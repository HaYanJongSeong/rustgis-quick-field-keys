"""Real file IO in a generated, isolated RustGIS CLI project."""
import json
import msvcrt
import os
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]
run = Path(os.environ["QFK_TEST_ROOT"]) if "QFK_TEST_ROOT" in os.environ else ROOT / "test-artifacts" / ("publish-" + uuid.uuid4().hex)
run.mkdir(parents=True, exist_ok=True)
sources = []
for name in ["primary.geojson", "other.geojson", "unsupported.json"]:
    path = run / name
    path.write_text(json.dumps({"type": "FeatureCollection", "features": [
        {"type": "Feature", "geometry": {"type": "Point", "coordinates": [i, 0]},
         "properties": {"uid": i + 1, "label": "original", "other": "unchanged"}}
        for i in range(3)]}), encoding="utf-8")
    sources.append(path)
primary, other, unsupported = sources
originals = [p.read_bytes() for p in sources]
env = dict(os.environ, RUSTGIS_PLUGINS_DIR=str(run / "plugins"),
           RUSTGIS_CONFIG_DIR=str(run / "config"), RUSTGIS_APP_DATA_DIR=str(run / "data"),
           RUSTGIS_CACHE_DIR=str(run / "cache"))
args = ["C:/Program Files/RustGIS/rustgis-cli.exe", "mcp", "--yes", "--no-plugins", "--plugin", str(ROOT)]
for path in sources:
    args.extend(["--data", str(path)])
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
    layers = {layer["name"]: layer["id"] for layer in state["layers"]}
    layer = layers["primary"]
    plugin("choose-layer", {"panel": {"layer": str(layer)}})
    configured = plugin("configure", {"layer": str(layer), "identity": "uid", "slot": "1",
        "field": "label", "value": "from-plugin"})
    applied = plugin("apply-1")
    before = plugin("state")["capture"]["payload"]
    manifest = json.loads((ROOT / "plugin.json").read_text(encoding="utf-8"))
    assert "form" not in next(c for c in manifest["commands"] if c["id"] == "save")
    assert primary.read_bytes() == originals[0]
    assert plugin("state")["capture"]["payload"] == before
    print("PASS Save has no confirmation form; editing alone does not publish")

    plugin("save", {"panel": {"layer": str(layers["other"])}}, error=True)
    assert [p.read_bytes() for p in sources] == originals
    print("PASS mismatched selected layer cannot redirect Save")

    # The host's publisher uses an OS lock. Hold an overlapping byte lock to force IO refusal.
    lock = run / ".primary.geojson.rustgis-lock"
    with lock.open("a+b") as handle:
        handle.seek(0)
        msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
        try:
            refusal = plugin("save", error=True)
            assert "publisher" in refusal.lower(), refusal
            assert primary.read_bytes() == originals[0]
            assert plugin("state")["capture"]["payload"] == before
            assert call("source.syncState", {"layer": layer})["edited"] is True
        finally:
            handle.seek(0)
            msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
    print("PASS actual source.publish IO refusal preserves source, panel and unsaved edits")

    call("edit.setValue", {"layer": layer, "row": 1, "field": "other", "value": "from-other-tool"})
    call("layer.select", {"layer": layers["other"]})
    saved = plugin("save")
    assert saved["sourcePublish"]["published"] is True
    assert saved["sourcePublish"]["layer"] == layer
    data = json.loads(primary.read_text(encoding="utf-8"))["features"]
    assert data[0]["properties"]["label"] == "from-plugin"
    assert data[1]["properties"]["other"] == "from-other-tool"
    assert other.read_bytes() == originals[1]
    assert call("source.syncState", {"layer": layer})["state"] == "inSync"
    assert saved["current"]["properties"]["label"] == "from-plugin"
    print("PASS Save publishes plugin and other-tool edits to designated source, not active layer")

    stamp = primary.stat().st_mtime_ns
    unchanged = plugin("save")
    assert unchanged["sourcePublish"]["published"] is False
    assert primary.stat().st_mtime_ns == stamp
    print("PASS unchanged layer returns no-write receipt without rewriting source")

    current = unchanged["panel"]["values"]["attributes"]
    label_id = next(f["id"] for f in unchanged["panel"]["items"]["attributes"]["fields"] if f["label"] == "label")
    current[label_id] = "pending-after-save"
    plugin("edit-current", {"panel": {"attributes": current}})
    before = plugin("state")["capture"]["payload"]
    changed = primary.read_bytes() + b"\n"
    primary.write_bytes(changed)
    plugin("save", error=True)
    assert primary.read_bytes() == changed
    assert plugin("state")["capture"]["payload"] == before
    assert plugin("rebuild-navigation")["current"]["properties"]["label"] == "pending-after-save"
    print("PASS external source change refuses overwrite while in-memory edits remain usable")

    untouched = plugin("state")["capture"]["payload"]
    options = plugin("options")
    assert options["identity"] == "uid" and "panel" not in options
    assert plugin("state")["capture"]["payload"] == untouched
    plugin("options", {"identity": "label"}, error=True)
    assert plugin("state")["capture"]["payload"] == untouched
    plugin("options", {"identity": "label", "reset-presets": True}, error=True)
    assert plugin("state")["capture"]["payload"] == untouched
    print("PASS Options does not replace drafts; identity reset requires confirmation and valid identity")

    for row, value in enumerate(["unique-a", "unique-b", "unique-c"]):
        call("edit.setValue", {"layer": layer, "row": row, "field": "label", "value": value})
    reset = plugin("options", {"identity": "label", "reset-presets": True})
    assert reset["panel"]["values"]["config"]["identity"] == "label"
    assert all(reset["panel"]["values"]["config"][f"preset{i}"] == "{}" for i in range(1, 10))
    assert primary.read_bytes() == changed
    print("PASS confirmed valid Options identity change prepares target and resets presets without file writes")

    shape = run / "shape.shp"
    call("share.exportLayer", {"layer": layers["other"], "path": str(shape)})
    call("layer.addData", {"path": str(shape)})
    shape_id = next(l["id"] for l in plugin("state")["layers"] if l["name"] == "shape")
    original_dbf = shape.with_suffix(".dbf").read_bytes()
    original_prj = shape.with_suffix(".prj").read_bytes()
    plugin("choose-layer", {"panel": {"layer": str(shape_id)}})
    plugin("configure", {"layer": str(shape_id), "identity": "uid", "slot": "1",
        "field": "label", "value": "shp-edit"})
    plugin("apply-1")
    saved_shape = plugin("save")
    assert saved_shape["sourcePublish"]["published"] is True
    assert shape.with_suffix(".dbf").read_bytes() != original_dbf
    assert shape.with_suffix(".prj").read_bytes() == original_prj
    existing = {l["id"] for l in plugin("state")["layers"]}
    call("layer.addData", {"path": str(shape)})
    reloaded_id = next(l["id"] for l in plugin("state")["layers"] if l["id"] not in existing)
    reloaded = plugin("choose-layer", {"panel": {"layer": str(reloaded_id)}})
    assert reloaded["current"]["properties"]["label"] == "shp-edit"
    assert call("source.syncState", {"layer": shape_id})["state"] == "inSync"
    print("PASS SHP publish changes DBF, preserves PRJ and reloads the saved attribute")

    plugin("choose-layer", {"panel": {"layer": str(layers["other"])}})
    call("edit.setValue", {"layer": layers["other"], "row": 0, "field": "other", "value": "z" * 4500})
    oversized = plugin("save")
    assert oversized["sourcePublish"]["published"] is True
    assert "preparationError" in oversized
    assert oversized["panel"]["values"]["session"]["payload"] == ""
    assert oversized["panel"]["items"]["attributes"]["enabled"] is False
    assert json.loads(other.read_text(encoding="utf-8"))["features"][0]["properties"]["other"] == "z" * 4500
    assert call("source.syncState", {"layer": layers["other"]})["state"] == "inSync"
    print("PASS committed Save is not falsely rolled back when subsequent panel preparation exceeds 4000 bytes")

    plugin("choose-layer", {"panel": {"layer": str(layers["unsupported"])}})
    plugin("configure", {"layer": str(layers["unsupported"]), "identity": "uid", "slot": "1",
        "field": "label", "value": "retained"})
    plugin("apply-1")
    before = plugin("state")["capture"]["payload"]
    plugin("save", error=True)
    assert unsupported.read_bytes() == originals[2]
    assert plugin("state")["capture"]["payload"] == before
    assert plugin("first")["current"]["properties"]["label"] == "retained"
    print("PASS unsupported source is rejected without losing memory edits or writing a file")
finally:
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode == 0, process.stderr.read()
