"""Column visibility is independent of data, preset assignment and the active layer."""
import json
import os
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]
run = Path(os.environ["QFK_TEST_ROOT"]) if "QFK_TEST_ROOT" in os.environ else ROOT / "test-artifacts" / ("columns-" + uuid.uuid4().hex)
run.mkdir(parents=True, exist_ok=True)
preference_file = run/"plugins/settings/quick-field-keys.json"
previous_preferences = preference_file.read_bytes() if preference_file.exists() else None
paths = []
for name in ["columns-a", "columns-b"]:
    path = run / (name + ".geojson")
    path.write_text(json.dumps({"type":"FeatureCollection","features":[
        {"type":"Feature","geometry":{"type":"Point","coordinates":[i,0]},
         "properties":{"uid":i+1,"status":"review","note":"keep","rank":10}}
        for i in range(3)]}), encoding="utf-8")
    paths.append(path)
originals = [p.read_bytes() for p in paths]
args = ["C:/Program Files/RustGIS/rustgis-cli.exe", "mcp", "--yes", "--no-plugins", "--plugin", str(ROOT)]
for path in paths:
    args.extend(["--data",str(path)])
env = dict(os.environ,RUSTGIS_CONFIG_DIR=str(run/"config"),RUSTGIS_APP_DATA_DIR=str(run/"data"),
           RUSTGIS_PLUGINS_DIR=str(run/"plugins"),RUSTGIS_CACHE_DIR=str(run/"cache"))
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


def choose(result, selected):
    values = {field["id"]:field["label"] in selected for field in result["panel"]["items"]["columns"]["fields"]}
    plugin("set-visible-fields",{"panel":{"columns":values}})
    return plugin("apply-visible-fields")


try:
    state = plugin("state")
    layers = {l["name"]:l["id"] for l in state["layers"]}
    a,b = layers["columns-a"],layers["columns-b"]
    plugin("choose-layer",{"panel":{"layer":str(a)}})
    configured = plugin("configure",{"layer":str(a),"identity":"uid","slot":"1",
        "field":"status","value":"done"})
    assert {f["label"] for f in configured["panel"]["items"]["columns"]["fields"]} == {"uid","status","note","rank"}
    before = call("info.project")
    plugin("visible-fields")
    all_choices = plugin("columns-all")
    assert all(all_choices["panel"]["values"]["columns"].values())
    assert "attributes" not in all_choices["panel"]["values"]
    cancelled = plugin("cancel-visible-fields")
    assert cancelled["panel"]["items"]["columns"]["visible"] is False
    assert "attributes" not in cancelled["panel"]["values"]
    assert (preference_file.read_bytes() if preference_file.exists() else None) == previous_preferences
    plugin("visible-fields")
    none = plugin("columns-none")
    assert not any(none["panel"]["values"]["columns"].values())
    plugin("cancel-visible-fields")
    assert (preference_file.read_bytes() if preference_file.exists() else None) == previous_preferences
    assert call("info.project") == before
    print("PASS Select all and Deselect all stage choices only; Cancel preserves display and writes no preferences")
    opened = plugin("visible-fields")
    assert opened["panel"]["items"]["columns"]["visible"] is True
    assert "attributes" not in opened["panel"]["items"]
    shown = choose(configured,{"note","rank"})
    fields = shown["panel"]["items"]["attributes"]["fields"]
    assert {f["label"] for f in fields} == {"note","note: NULL","rank","rank: NULL"}
    assert json.loads(shown["panel"]["values"]["config"]["preset1"]) == {"status":"done"}
    assert call("info.project") == before
    print("PASS eye opens real column choices; showing independent fields preserves data and preset assignments")

    hidden = choose(shown,set())
    assert hidden["panel"]["items"]["attributes"]["visible"] is False
    applied = plugin("apply-1")
    assert applied["current"]["properties"]["status"] == "done"
    assert applied["panel"]["items"]["attributes"]["visible"] is False
    restored = choose(applied,{"status","note"})
    field = next(f for f in restored["panel"]["items"]["attributes"]["fields"] if f["label"]=="status")
    assert restored["panel"]["values"]["attributes"][field["id"]] == "done"
    print("PASS hiding all columns does not disable presets; showing a column restores its real current value")

    locks = restored["panel"]["values"]["locks"]
    lock_id = next(f["id"] for f in restored["panel"]["items"]["locks"]["fields"] if f["label"]=="Lock status")
    locks[lock_id] = True
    plugin("set-locks",{"panel":{"locks":locks}})
    note_only = choose(restored,{"note"})
    note_locks = {key:True for key in note_only["panel"]["values"]["locks"]}
    kept = plugin("set-locks",{"panel":{"locks":note_locks}})
    assert set(json.loads(kept["panel"]["values"]["config"]["locks"])) == {"note","status"}
    plugin("apply-1",error=True)
    restored = choose(note_only,{"note","status"})
    plugin("set-locks",{"panel":{"locks":{key:False for key in restored["panel"]["values"]["locks"]}}})
    print("PASS changing visible locks preserves hidden-field locks; hidden locked presets remain blocked")

    attributes = restored["panel"]["values"]["attributes"]
    note = next(f for f in restored["panel"]["items"]["attributes"]["fields"] if f["label"]=="note")
    attributes[note["id"]] = "changed-note"
    edited = plugin("edit-current",{"panel":{"attributes":attributes}})
    assert edited["current"]["properties"]["note"] == "changed-note"
    stale = edited["panel"]["values"]["columns"]
    plugin("next")
    plugin("set-visible-fields",{"panel":{"columns":stale}})
    moved = plugin("apply-visible-fields")
    assert json.loads(moved["panel"]["values"]["config"]["visible_fields"]) == ["note","status"]
    assert plugin("first")["current"]["properties"]["note"] == "changed-note"
    print("PASS chosen fields are editable; stale chooser input cannot corrupt the newly current row")

    other = plugin("choose-layer",{"panel":{"layer":str(b)}})
    assert other["panel"]["values"]["config"]["visible_fields"] == ""
    assert other["panel"]["items"]["attributes"]["visible"] is False
    assert plugin("first")["current"]["properties"]["note"] == "keep"
    returned = plugin("choose-layer",{"panel":{"layer":str(a)}})
    assert json.loads(returned["panel"]["values"]["config"]["visible_fields"]) == ["note","status"]
    assert [p.read_bytes() for p in paths] == originals
    print("PASS each layer restores its own saved column choices; original files remain untouched")
    preference_file = run/"plugins/settings/quick-field-keys.json"
    preferences = json.loads(preference_file.read_text(encoding="utf-8"))["values"]["column-visibility"]
    assert all(isinstance(names,list) and all(name in {"uid","status","note","rank"} for name in names) for names in preferences.values())
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode == 0,process.stderr.read()
    reopened_args = args[:6]
    for path in reversed(paths):
        reopened_args.extend(["--data",str(path)])
    process = subprocess.Popen(reopened_args,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,text=True,encoding="utf-8")
    reopened_state = plugin("state")
    reopened_id = next(l["id"] for l in reopened_state["layers"] if l["name"]=="columns-a")
    reopened = plugin("choose-layer",{"panel":{"layer":str(reopened_id)}})
    assert json.loads(reopened["panel"]["values"]["config"]["visible_fields"]) == ["note","status"]
    assert reopened["current"]["properties"]["note"] == "keep"
    print("PASS new RustGIS process restores column names only; unsaved feature values were not persisted")
finally:
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode==0,process.stderr.read()
