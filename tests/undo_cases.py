"""Exercise guarded plugin-only Undo using existing polygon fixtures and profile."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
fixtures = [p for p in (ROOT/"test-artifacts").glob("polygon-examples-*")
    if (p/"polygon-grid.geojson").is_file() and (p/"polygon-triangles.geojson").is_file()]
run = max(fixtures,key=lambda p:p.stat().st_mtime_ns)
paths = [run/"polygon-grid.geojson",run/"polygon-triangles.geojson"]
before = [hashlib.sha256(p.read_bytes()).hexdigest() for p in paths]
env = dict(os.environ, **{f"RUSTGIS_{key}":str(run/value) for key,value in
    [("CONFIG_DIR","config"),("APP_DATA_DIR","data"),("CACHE_DIR","cache"),("PLUGINS_DIR","plugins")]})
args = ["C:/Program Files/RustGIS/rustgis-cli.exe","mcp","--yes","--no-plugins","--plugin",str(ROOT)]
for path in paths:
    args.extend(["--data",str(path)])
process = subprocess.Popen(args,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,text=True,encoding="utf-8")
counter = 0


def call(command, params=None, error=False):
    global counter
    counter += 1
    process.stdin.write(json.dumps({"jsonrpc":"2.0","id":counter,"method":"tools/call",
        "params":{"name":"run_command","arguments":{"command":command,"params":params or {}}}})+"\n")
    process.stdin.flush()
    result = json.loads(process.stdout.readline())["result"]
    text = result["content"][0]["text"]
    assert bool(result.get("isError")) == error,(command,text)
    return text if error else json.loads(text)


def plugin(command, params=None, error=False):
    return call("plugin:quick-field-keys/"+command,params,error)


try:
    state = plugin("state")
    layers = {l["name"]:l["id"] for l in state["layers"]}
    grid,other = layers["polygon-grid"],layers["polygon-triangles"]
    prepared = plugin("choose-layer",{"panel":{"layer":str(grid)}})
    original = prepared["current"]["properties"]["status"]
    second_original = json.loads(prepared["panel"]["values"]["session"]["payload"])["captured"][1]["properties"]["status"]
    assert prepared["panel"]["items"]["undo-edit"]["enabled"] is False
    plugin("undo-edit",error=True)
    edited = next(v for v in ["undo-first", "undo-again", "undo-third"] if v not in [original, second_original])
    plugin("configure",{"layer":str(grid),"identity":"uid","slot":"1","field":"status","value":edited})

    # Given: two consecutive edits to one feature and an unrelated layer selection.
    call("select.rows",{"layer":other,"rows":[0,1,2,3],"mode":"remove"})
    call("select.rows",{"layer":other,"rows":[2],"mode":"add"})
    applied = plugin("apply-1")
    attributes = applied["panel"]["values"]["attributes"]
    field = next(f["id"] for f in applied["panel"]["items"]["attributes"]["fields"] if f["label"]=="status")
    attributes[field] = "undo-second"
    plugin("edit-current",{"panel":{"attributes":attributes}})
    plugin("next")
    # When
    reverted = plugin("undo-edit")
    # Then: the entire consecutive group is reverted and its feature revealed, not the current one.
    assert reverted["current"]["properties"]["uid"] == 1
    assert reverted["current"]["properties"]["status"] == original
    assert reverted["panel"]["items"]["undo-edit"]["enabled"] is False
    other_selection = call("table.statistics",{"layer":other,"field":"uid","selected":True})
    assert other_selection["count"]==1 and other_selection["min"]==3
    assert [hashlib.sha256(p.read_bytes()).hexdigest() for p in paths] == before
    print("PASS consecutive feature edits merge; Undo reveals and restores the edited feature without touching another layer or disk")

    # Given: plugin edits to two different objects.
    plugin("apply-1")
    plugin("next")
    plugin("apply-1")
    # When
    first_undo = plugin("undo-edit")
    # Then
    assert first_undo["current"]["properties"]["uid"]==2
    assert first_undo["current"]["properties"]["status"]==second_original
    assert first_undo["panel"]["items"]["undo-edit"]["enabled"] is True
    second_undo = plugin("undo-edit")
    assert second_undo["current"]["properties"]["uid"]==1
    assert second_undo["current"]["properties"]["status"]==original
    print("PASS Undo follows plugin edit groups across objects rather than undoing navigation or another tool")

    # Given: a field was locked after a plugin edit.
    applied = plugin("apply-1")
    locks = applied["panel"]["values"]["locks"]
    lock = next(f["id"] for f in applied["panel"]["items"]["locks"]["fields"] if f["label"]=="Lock status")
    locks[lock] = True
    plugin("set-locks",{"panel":{"locks":locks}})
    retained = plugin("state")["panel"]["values"]["edit-history"]
    # When
    plugin("undo-edit",error=True)
    # Then
    assert plugin("state")["panel"]["values"]["edit-history"]==retained
    locks[lock] = False
    plugin("set-locks",{"panel":{"locks":locks}})
    assert plugin("undo-edit")["current"]["properties"]["status"]==original
    print("PASS locked fields refuse Undo without consuming its history; unlocking permits the original reversal")

    # Given: another tool changed the plugin-edited value.
    plugin("apply-1")
    call("edit.setValue",{"layer":grid,"row":0,"field":"status","value":"outside-history"})
    # When
    plugin("undo-edit",error=True)
    # Then
    stats = call("table.statistics",{"layer":grid,"field":"status","selected":True})
    assert stats["min"]=="outside-history" and stats["max"]=="outside-history",stats
    plugin("capture")
    plugin("undo-edit",error=True)
    print("PASS external edits are never overwritten; rebuilding navigation clears the obsolete history")

    # Given: an unsaved plugin edit.
    plugin("apply-1")
    # When
    saved = plugin("save")
    # Then: committed source writes cannot be undone by plugin memory history.
    assert json.loads(paths[0].read_text(encoding="utf-8"))["features"][0]["properties"]["status"]==edited
    assert saved["panel"]["items"]["undo-edit"]["enabled"] is False
    plugin("undo-edit",error=True)
    assert hashlib.sha256(paths[1].read_bytes()).hexdigest()==before[1]
    print("PASS Save clears Undo after the real source write; no fake disk rollback is offered")
    print("REUSED_POLYGONS",run)
finally:
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode==0,process.stderr.read()
