"""Exercise installed RustGIS in a private CLI session, never the desktop project."""
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid
import zipfile
from native_ui_cases import check_settings_controls
from no_capture_cases import check_no_capture, check_manifest, check_captured_controls

ROOT = Path(__file__).resolve().parents[1]
CLI = "C:/Program Files/RustGIS/rustgis-cli.exe"
PLUGIN = os.environ.get("QFK_TEST_PLUGIN", str(ROOT))
if Path(PLUGIN).is_dir():
    manifest = json.loads((Path(PLUGIN) / "plugin.json").read_text(encoding="utf-8"))
else:
    with zipfile.ZipFile(PLUGIN) as archive:
        manifest = json.loads(archive.read("plugin.json"))
preset_commands = {f"apply-{i}" for i in range(1, 10)}
assert not any(item.get("command") in preset_commands for item in manifest["panel"]["items"])
assert all(item not in preset_commands for group in manifest["tab"]["groups"] for item in group["items"])
assert len(next(c for c in manifest["commands"] if c["id"] == "configure")["form"]) == 7
assert not {"sort", "ascending", "selected-only", "zoom-mode"} & {i.get("id") for i in manifest["panel"]["items"]}
assert {f"apply-{i}" for i in range(1, 10)} <= {command["id"] for command in manifest["commands"]}
print("PASS no visible preset application controls; commands retained for shortcuts/search")
check_manifest(manifest)
run_dir = Path(os.environ["QFK_TEST_ROOT"]) if "QFK_TEST_ROOT" in os.environ else ROOT / "test-artifacts" / uuid.uuid4().hex
run_dir.mkdir(parents=True, exist_ok=True)
fixture = run_dir / "fixture.geojson"
fixture.write_text(json.dumps({"type": "FeatureCollection", "features": [
    {"type": "Feature", "geometry": {"type": "Point", "coordinates": [i, 0]},
     "properties": {"uid": i + 1, "label": "original", "n": 10, "real": 1.25, "flag": True}}
    for i in range(3)
]}), encoding="utf-8")
original_fixture = fixture.read_bytes()
env = os.environ.copy()
env["RUSTGIS_PLUGINS_DIR"] = str(run_dir / "plugins")
cli_config = {"layer": "fixture", "identity": "uid", "locks": "[]",
              **{f"preset{i}": '{"label":"cli-run"}' for i in range(1, 10)}}
commands = ["plugin:quick-field-keys/state", 'select.rows={"layer":"fixture","rows":[0]}',
            'plugin:quick-field-keys/choose-layer={"panel":{"layer":"1"}}',
            "plugin:quick-field-keys/capture=" + json.dumps({"panel": {"config": cli_config}}),
            "plugin:quick-field-keys/apply-1"]
args = [CLI, "run", "--no-plugins", "--plugin", PLUGIN, "--data", str(fixture)]
for command in commands:
    args.extend(["--cmd", command])
cli = subprocess.run(args, env=env, capture_output=True, text=True, encoding="utf-8", timeout=30)
assert cli.returncode == 0, cli.stderr
tail = cli.stdout.strip()
decoder = json.JSONDecoder()
last = None
while tail:
    last, end = decoder.raw_decode(tail)
    tail = tail[end:].lstrip()
assert last["current"]["properties"]["label"] == "cli-run"
print("PASS CLI run functional WASM command (not merely plugin load)")
process = subprocess.Popen(
    [CLI, "mcp", "--yes", "--no-plugins", "--plugin", PLUGIN, "--data", str(fixture)],
    env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    text=True, encoding="utf-8",
)
counter = 0


def call(command, params=None, error=False):
    global counter
    counter += 1
    request = {"jsonrpc": "2.0", "id": counter, "method": "tools/call", "params": {
        "name": "run_command", "arguments": {"command": command, "params": params or {}}}}
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())
    result = response["result"]
    text = result["content"][0]["text"]
    if error:
        assert result.get("isError"), (command, text)
        return text
    assert not result.get("isError"), (command, text)
    return json.loads(text)


def plugin(command, config=None, error=False):
    params = {} if config is None else {"panel": {"config": config}}
    return call("plugin:quick-field-keys/" + command, params, error)


try:
    state = plugin("state")
    assert state["host"]["rustgis"] == "0.7.0"
    assert state["host"]["permissions"] == ["edit", "files"]
    check_no_capture(call)
    layer = state["layers"][0]["id"]
    call("plugin:quick-field-keys/choose-layer", {"panel":{"layer":str(layer)}})
    assert state["panel"]["items"]["config"]["visible"] is False
    assert state["panel"]["items"]["session"]["visible"] is False
    assert state["panel"]["items"]["filter"]["visible"] is True
    assert state["panel"]["items"]["view"]["visible"] is True
    assert not any(i.get("id") == "config" and i.get("label") for i in manifest["panel"]["items"])
    # Given: typed native settings, not JSON entry boxes on the main panel.
    call("select.rows", {"layer":layer,"rows":[0]})
    before_settings = call("info.project")
    settings = {"layer": str(layer), "identity": "uid", "slot": "1", "field": "label", "value": "settings-preset"}
    for slot, field, value, null in [(1, "label", "settings-preset", False), (2, "n", "42", False),
                                     (3, "flag", "false", False), (4, "real", "", True),
                                     (5, "n", "9007199254740993", False)]:
        call("plugin:quick-field-keys/configure", dict(settings, slot=str(slot), field=field, value=value, null=null))
    unassigned = call("plugin:quick-field-keys/configure", dict(settings, slot="2", unassign=True))
    saved = unassigned["panel"]["values"]["config"]
    assert json.loads(saved["preset1"])["label"] == "settings-preset"
    assert saved["preset2"] == "{}" and json.loads(saved["preset4"])["real"] is None
    call("plugin:quick-field-keys/configure", dict(settings, slot="2", field="n", value="42"))
    oversized_settings = dict(settings, value="x" * 4000)
    call("plugin:quick-field-keys/configure", oversized_settings, error=True)
    assert call("info.project") == before_settings
    print("PASS compact single-slot settings preserve other slots, unassign/NULL and reject overflow without feature writes")
    call("select.rows", {"layer": layer, "rows": [0, 1]})
    check_captured_controls(plugin("capture"))
    assert plugin("apply-1")["current"]["properties"]["label"] == "settings-preset"
    assert plugin("apply-2")["current"]["properties"]["n"] == 42
    assert plugin("apply-3")["current"]["properties"]["flag"] is False
    out = plugin("apply-4")
    assert out["current"]["properties"]["real"] is None
    large = plugin("apply-5")
    assert large["current"]["properties"]["n"] == 9007199254740993
    assert next(f for f in large["panel"]["items"]["attributes"]["fields"] if f["label"] == "n")["kind"] == "text"
    out = plugin("apply-2")
    fields = out["panel"]["items"]["attributes"]["fields"]
    n_id = next(f["id"] for f in fields if f["label"] == "n")
    attributes = out["panel"]["values"]["attributes"]
    attributes[n_id] = 31
    edited = call("plugin:quick-field-keys/edit-current", {"panel": {"attributes": attributes}})
    assert edited["current"]["properties"]["n"] == 31
    check_settings_controls(call, layer, edited)
    attributes = dict(edited["panel"]["values"]["attributes"])
    n_id = next(f["id"] for f in edited["panel"]["items"]["attributes"]["fields"] if f["label"] == "n")
    locks = edited["panel"]["values"]["locks"]
    lock_id = next(f["id"] for f in edited["panel"]["items"]["locks"]["fields"] if f["label"] == "Lock n")
    locks[lock_id] = True
    call("plugin:quick-field-keys/set-locks", {"panel": {"locks": locks}})
    plugin("apply-2", error=True)
    attributes[n_id] = 50
    call("plugin:quick-field-keys/edit-current", {"panel": {"attributes": attributes}}, error=True)
    print("PASS native settings, typed preset commands, NULL, inline edits and plugin locks")
    # Given: all-feature capture with an explicit substring filter and descending order.
    filtered = call("plugin:quick-field-keys/capture", {"panel": {
        "filter": {"selected-only": False, "sort": "uid", "ascending": False, "query": "original"}}})
    assert filtered["current"]["properties"]["uid"] == 3
    assert plugin("last")["current"]["properties"]["uid"] == 2
    assert plugin("first")["current"]["properties"]["uid"] == 3
    plugin("zoom-current")
    project = call("info.project")
    assert abs(project["center"]["lon"] - 2) < 0.001
    assert project["layers"][0]["definitionQuery"] is None
    call("plugin:quick-field-keys/clear-filter")
    print("PASS frozen substring-filter/sort capture, first/last and actual map zoom; provider filter unchanged")
    # Restore only this test's in-memory fixture edits before the original regression suite.
    for field, value in {"label": "original", "n": 10, "real": 1.25, "flag": True}.items():
        call("edit.setValue", {"layer": layer, "row": 0, "field": field, "value": value})
    call("plugin:quick-field-keys/state")
    config = {"layer": str(layer), "identity": "uid", "locks": "[]",
              **{f"preset{i}": json.dumps({"label": f"preset-{i}"}) for i in range(1, 10)}}
    call("select.rows", {"layer": layer, "rows": []})
    call("plugin:quick-field-keys/clear-filter")
    # The old suite explicitly tests selection-only capture and default ordering.
    call("plugin:quick-field-keys/state")
    config_params = {"panel": {"filter": {"query": "", "selected-only": True, "sort": "row", "ascending": True}, "config": config}}
    call("plugin:quick-field-keys/state", config_params)
    plugin("capture", config, error=True)
    call("select.rows", {"layer": layer, "rows": [0, 1]})
    first = plugin("capture", config)
    assert first["current"]["properties"]["uid"] == 1
    call("select.rows", {"layer": layer, "rows": [2]})
    for slot in range(1, 10):
        applied = plugin(f"apply-{slot}")
        assert applied["current"]["properties"]["label"] == f"preset-{slot}"
        assert applied["current"]["properties"]["uid"] == 1
    print("PASS presets 1-9; captured target independent of live selection")
    assert plugin("next")["current"]["properties"]["uid"] == 2
    plugin("next", error=True)
    assert plugin("previous")["current"]["properties"]["uid"] == 1
    plugin("previous", error=True)
    print("PASS captured navigation and boundary errors")
    config["preset1"] = '{"n":null,"real":2.5,"flag":false}'
    out = plugin("apply-1", config)
    assert out["current"]["properties"]["n"] is None
    assert out["current"]["properties"]["real"] == 2.5
    assert out["current"]["properties"]["flag"] is False
    print("PASS NULL, double and boolean writes")
    config["locks"] = '["label"]'
    config["preset1"] = '{"label":"forbidden"}'
    plugin("apply-1", config, error=True)
    config["locks"] = '[]'
    config["preset1"] = '{"n":"not integer"}'
    plugin("apply-1", config, error=True)
    config["preset1"] = '{"uid":99}'
    plugin("apply-1", config, error=True)
    print("PASS lock, identity lock and typed-value rejection")
    call("edit.setValue", {"layer": layer, "row": 0, "field": "n", "value": 20})
    plugin("apply-2", config, error=True)
    print("PASS stale capture rejection after external edit")
    call("select.rows", {"layer": layer, "rows": [1]})
    plugin("capture", config)
    call("data.attributeRule", {"layer": layer, "name": "positive", "kind": "constraint", "expression": "n >= 0"})
    config["preset1"] = '{"label":"must-roll-back","n":-1}'
    plugin("apply-1", config, error=True)
    after = plugin("capture", config)
    assert after["current"]["properties"]["label"] == "original"
    assert after["current"]["properties"]["n"] == 10
    print("PASS host rollback after second field constraint failure")
    # Given: a valid capture padded to exactly the conservative host boundary.
    payload = plugin("state")["capture"]["payload"]
    padding = 4000 - len(payload.encode("utf-8"))
    assert padding > 0
    call("edit.setValue", {"layer": layer, "row": 1, "field": "label",
                           "value": "original" + "a" * padding})
    plugin("capture", config)
    payload = plugin("state")["capture"]["payload"]
    assert len(payload.encode("utf-8")) == 4000
    assert json.loads(payload)["captured"][0]["properties"]["uid"] == 2
    config["preset1"] = '{"n":11}'
    # When: the next WASM call reads the host-retained text field.
    near_limit = plugin("apply-1", config)
    # Then: the complete JSON survives the host roundtrip and remains usable.
    assert near_limit["current"]["properties"]["n"] == 11
    print("PASS exact 4000-byte capture host roundtrip and subsequent apply")
    # Given: one more byte would make the post-edit capture too large.
    retained = plugin("state")["capture"]["payload"]
    project_before = call("info.project")
    growth_config = dict(config)
    history_before = plugin("state")["panel"]["values"]["edit-history"]
    old_label = near_limit["current"]["properties"]["label"]
    growth_config["preset1"] = json.dumps({"label": old_label + "x"})
    # When: capture growth or the new Undo history would exceed its byte budget.
    rejection = plugin("apply-1", growth_config, error=True)
    # Then: both the attribute edit and panel/config changes roll back.
    assert "4000 UTF-8 bytes" in rejection or "Undo history exceeds 4000 bytes" in rejection, rejection
    assert plugin("state")["capture"]["payload"] == retained
    assert plugin("state")["panel"]["values"]["edit-history"] == history_before
    assert call("info.project") == project_before
    assert plugin("apply-1")["current"]["properties"]["label"] == old_label
    print("PASS capture/Undo history size refusal preserves values, capture, history and project")
    # Given: another selected feature whose capture cannot fit the host field.
    retained = plugin("state")["capture"]["payload"]
    call("edit.setValue", {"layer": layer, "row": 2, "field": "label", "value": "b" * 4001})
    call("select.rows", {"layer": layer, "rows": [2]})
    project_before = call("info.project")
    # When: Capture rejects it before returning panel state or changing selection.
    rejection = plugin("capture", config, error=True)
    # Then: the old capture and project are retained without truncation.
    assert "4000 UTF-8 bytes" in rejection
    assert plugin("state")["capture"]["payload"] == retained
    assert call("info.project") == project_before
    assert plugin("apply-1")["current"]["properties"]["uid"] == 2
    print("PASS oversized capture rejected; previous capture/project retained and usable")
    before_reset = call("info.project")
    reset = call("plugin:quick-field-keys/configure", dict(settings, identity="n", slot="9", unassign=True))
    saved = reset["panel"]["values"]["config"]
    assert all(saved[f"preset{i}"] == "{}" for i in range(1, 10))
    assert json.loads(plugin("state")["capture"]["payload"])["identity"] == "n"
    plugin("apply-1", error=True)
    assert call("info.project") == before_reset
    print("PASS identity change resets foreign presets and automatically prepares without feature writes")
    assert fixture.read_bytes() == original_fixture
    assert not (run_dir / "plugins" / "settings").exists()
    print("PASS source fixture unchanged and no plugin settings writes")
finally:
    process.stdin.close()
    process.wait(timeout=15)
    stderr = process.stderr.read()
    if process.returncode:
        print(stderr, file=sys.stderr)
