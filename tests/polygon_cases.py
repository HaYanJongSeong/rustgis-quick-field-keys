"""Exercise copied attributes on isolated polygon datasets through real WASM."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]
source = ROOT / "test-artifacts/gui-columns-apply/qfk-ui-fixture.geojson"
source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
copied = json.loads(source.read_text(encoding="utf-8"))["features"]
existing = [p for p in (ROOT / "test-artifacts").glob("polygon-examples-*")
    if (p / "polygon-grid.geojson").is_file() and (p / "polygon-triangles.geojson").is_file()]
run = max(existing, key=lambda p:p.stat().st_mtime_ns) if existing else ROOT / "test-artifacts" / ("polygon-examples-" + uuid.uuid4().hex)
run.mkdir(parents=True, exist_ok=True)
paths = []
originals = []
for name, count in [("polygon-grid", 8), ("polygon-triangles", 4)]:
    path = run / (name + ".geojson")
    if path.exists():
        assert len(json.loads(path.read_text(encoding="utf-8"))["features"]) == count
        paths.append(path)
        originals.append(path.read_bytes())
        continue
    features = []
    for index in range(count):
        x = round(127 + (index % 4) * 0.004, 3)
        y = round(37.5 + (index // 4) * 0.004, 3)
        ring = [[x,y],[round(x+0.002,3),y],[round(x+0.002,3),round(y+0.002,3)]]
        if name == "polygon-grid":
            ring.append([x,round(y+0.002,3)])
        ring.append([x,y])
        properties = dict(copied[index % len(copied)]["properties"], uid=index+1)
        features.append({"type":"Feature","id":f"{name}-{index+1}",
            "geometry":{"type":"Polygon","coordinates":[ring]},"properties":properties})
    path = run / (name + ".geojson")
    path.write_text(json.dumps({"type":"FeatureCollection","features":features}), encoding="utf-8")
    paths.append(path)
    originals.append(path.read_bytes())
args = ["C:/Program Files/RustGIS/rustgis-cli.exe","mcp","--yes","--no-plugins","--plugin",str(ROOT)]
for path in paths:
    args.extend(["--data",str(path)])
env = dict(os.environ, RUSTGIS_PLUGINS_DIR=str(run/"plugins"),
    RUSTGIS_CONFIG_DIR=str(run/"config"), RUSTGIS_APP_DATA_DIR=str(run/"data"),
    RUSTGIS_CACHE_DIR=str(run/"cache"))
process = subprocess.Popen(args,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,text=True,encoding="utf-8")
counter = 0


def call(command, params=None, error=False):
    global counter
    counter += 1
    process.stdin.write(json.dumps({"jsonrpc":"2.0","id":counter,"method":"tools/call",
        "params":{"name":"run_command","arguments":{"command":command,"params":params or {}}}})+"\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())["result"]
    text = response["content"][0]["text"]
    assert bool(response.get("isError")) == error,(command,text)
    return text if error else json.loads(text)


def plugin(command, params=None, error=False):
    return call("plugin:quick-field-keys/"+command,params,error)


try:
    # Given: eight rectangles and four triangles copied from existing example attributes.
    state = plugin("state")
    layers = {layer["name"]:layer["id"] for layer in state["layers"]}
    grid, triangles = layers["polygon-grid"],layers["polygon-triangles"]
    ready = plugin("choose-layer",{"panel":{"layer":str(grid)}})
    snapshot = json.loads(ready["panel"]["values"]["session"]["payload"])
    assert len(snapshot["captured"]) == 8
    assert all(f["geometry"]["type"]=="Polygon" for f in snapshot["captured"])
    print("PASS 12 copied-attribute polygons loaded in two isolated layers; existing fixtures reused when available",run)

    # When: navigate using polygon geometry, then restore a non-contiguous selection.
    plugin("configure",{"layer":str(grid),"identity":"uid","slot":"1","field":"status","value":"polygon-tested"})
    plugin("zoom-current",{"panel":{"view":{"zoom-mode":"feature"}}})
    fitted = call("info.project")
    assert abs(fitted["center"]["lon"]-127.001)<1e-5
    assert abs(fitted["center"]["lat"]-37.501)<1e-5
    call("select.rows",{"layer":grid,"rows":[1,6],"mode":"new"})
    call("map.goToXY",{"lon":127.02,"lat":37.52,"scale":25000,"bearing":27,"pitch":35})
    previous = call("info.project")
    moved = plugin("next",{"panel":{"view":{"zoom-mode":"keep-scale"}}})
    history = json.loads(moved["panel"]["values"]["session"]["payload"])["previous_view"]
    assert [(f["row"],f["id"],f["identity"]) for f in history["selected"]] == [(1,2,2),(6,7,7)]
    assert abs(call("info.project")["scale"]-25000)<2
    plugin("apply-1")
    restored = plugin("previous-view")
    # Then: exact identities and camera return without reverting the attribute edit.
    stats = call("table.statistics",{"layer":grid,"field":"uid","selected":True})
    assert (stats["count"],stats["min"],stats["max"],stats["sum"]) == (2,2,7,9)
    current = call("info.project")
    for axis in ["lon","lat"]:
        assert abs(current["center"][axis]-previous["center"][axis])<1e-6
    for key in ["scale","bearing","pitch"]:
        assert abs(current[key]-previous[key])<1e-3
    payload = json.loads(restored["panel"]["values"]["session"]["payload"])
    assert payload["captured"][1]["properties"]["status"] == "polygon-tested"
    assert paths[0].read_bytes() == originals[0]
    print("PASS polygon fit/keep-scale, exact IDs 2 and 7, rotated/tilted previous view; edit retained only in memory")

    # When: save the designated polygon layer, with the other layer active.
    call("layer.select",{"layer":triangles})
    plugin("save")
    # Then: only the intended attribute changes; all polygons and the other dataset are preserved.
    saved = json.loads(paths[0].read_text(encoding="utf-8"))["features"]
    expected = json.loads(originals[0])["features"]
    assert [f["geometry"] for f in saved] == [f["geometry"] for f in expected]
    expected[1]["properties"]["status"] = "polygon-tested"
    assert [f["properties"] for f in saved] == [f["properties"] for f in expected]
    assert paths[1].read_bytes() == originals[1]
    print("PASS polygon Save writes the designated source immediately, preserving geometry and the other layer")

    # When: change target, then attempt to restore the old layer's selection.
    plugin("choose-layer",{"panel":{"layer":str(triangles)}})
    before = call("info.project")
    plugin("previous-view",error=True)
    # Then: foreign history cannot select rows or move the view.
    assert call("info.project") == before
    last = plugin("last",{"panel":{"view":{"zoom-mode":"fixed-scale","scale":5000}}})
    assert last["current"]["properties"]["uid"] == 4
    assert abs(call("info.project")["scale"]-5000)<2
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    print("PASS triangle fixed-scale navigation and foreign-history rejection; copied-from source unchanged")

    # Given: polygon attributes and a selection in another layer.
    plugin("choose-layer",{"panel":{"layer":str(grid)}})
    call("select.rows",{"layer":triangles,"rows":[0,1,2,3],"mode":"remove"})
    call("select.rows",{"layer":triangles,"rows":[2],"mode":"add"})
    # When: rebuild with a host SQL condition and descending numeric sort.
    filtered = plugin("rebuild-navigation",{"panel":{"filter":{
        "mode":"RustGIS SQL","query":"uid >= 3 AND uid <= 6",
        "sort":"uid","ascending":False,"selected-only":False}}})
    # Then: navigation uses matching IDs, not a substring; the other selection survives.
    subset = json.loads(filtered["panel"]["values"]["session"]["payload"])
    assert [f["properties"]["uid"] for f in subset["captured"]] == [6,5,4,3]
    other = call("table.statistics",{"layer":triangles,"field":"uid","selected":True})
    assert other["count"] == 1 and other["min"] == 3,other
    print("PASS real host SQL expression filters polygons and sorts numerically without clearing another layer selection")

    # Given: the original target selection has IDs 2 and 7.
    call("select.rows",{"layer":grid,"rows":[1,6],"mode":"add"})
    call("select.rows",{"layer":grid,"rows":[5],"mode":"remove"})
    # When
    selected_filter = plugin("rebuild-navigation",{"panel":{"filter":{
        "mode":"RustGIS SQL","query":"uid >= 3","sort":"uid",
        "ascending":True,"selected-only":True}}})
    # Then: expression matching intersects the selection from before evaluation.
    subset = json.loads(selected_filter["panel"]["values"]["session"]["payload"])
    assert [f["properties"]["uid"] for f in subset["captured"]] == [7]
    print("PASS SQL with Selected only intersects the original selection instead of using a temporary selection")

    # Given: a valid navigation list, selection and view.
    retained = plugin("state")["capture"]["payload"]
    before = call("info.project")
    # When: a syntax error or an empty result is applied.
    for query in ["uid >=", "uid > 100"]:
        plugin("rebuild-navigation",{"panel":{"filter":{"mode":"RustGIS SQL","query":query}}},error=True)
        # Then: the complete prior host state and list are retained.
        assert plugin("state")["capture"]["payload"] == retained
        assert call("info.project") == before
    print("PASS invalid SQL and zero matches preserve navigation, all layer selections and the view atomically")

    # Given: a provider definition query hides higher IDs.
    call("layer.definitionQuery",{"layer":grid,"where":"uid <= 6"})
    # When
    drawn_filter = plugin("rebuild-navigation",{"panel":{"filter":{
        "mode":"RustGIS SQL","query":"uid >= 5","selected-only":False}}})
    # Then: the documented host-drawn scope applies, without replacing the provider filter.
    subset = json.loads(drawn_filter["panel"]["values"]["session"]["payload"])
    assert [f["properties"]["uid"] for f in subset["captured"]] == [5,6]
    project = call("info.project")
    assert next(l for l in project["layers"] if l["id"]==grid)["definitionQuery"] == "uid <= 6"
    call("layer.definitionQuery",{"layer":grid,"where":""})
    assert paths[1].read_bytes() == originals[1]
    assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
    print("PASS SQL respects the documented drawn-feature scope and preserves provider filters and source files")

    # Given: the rejected experimental builder is not a registered command.
    manifest = json.loads((ROOT/"plugin.json").read_text(encoding="utf-8"))
    assert not any(c["id"] == "expression-builder" for c in manifest["commands"])
    assert not any(i.get("command") == "expression-builder" for i in manifest["panel"]["items"])
    retained = plugin("state")
    before = call("info.project")
    # When: an attempt is made to invoke its old command ID.
    plugin("expression-builder",error=True)
    # Then: it cannot open a form or change panel/project state.
    assert plugin("state") == retained
    assert call("info.project") == before
    print("PASS rejected simple builder is absent from panel/command registry and cannot be invoked")
    print("POLYGON_EXAMPLES",run)
finally:
    process.stdin.close()
    process.wait(timeout=15)
    assert process.returncode == 0,process.stderr.read()
