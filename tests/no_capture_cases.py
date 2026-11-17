"""No-capture public commands must fail without exposing JSON parser errors."""


def check_manifest(manifest):
    # These ids are present in the official icons.rs ALL list and paint match arms.
    icons = {"pane_right", "settings", "select", "calculate", "field", "save", "time_first", "time_prev", "time_next", "time_last", "zoom_sel"}
    commands = {c["id"]: c for c in manifest["commands"]}
    assert manifest["panel"]["icon"] in icons
    for command in ["configure", "capture", "first", "previous", "next", "last", "zoom-current"]:
        assert commands[command]["icon"] in icons
    for item in manifest["panel"]["items"]:
        assert "icon" not in item
        if item.get("command") in {"first", "previous", "next", "last"}:
            assert set(item["label"]) <= set("|◀▶") and item["help"]
    items = manifest["panel"]["items"]
    actions = [i.get("command") for i in items]
    index = actions.index("configure")
    assert actions[index:index + 4] == ["configure", "visible-fields", "save", "options"]
    assert "toggle-advanced" not in actions
    keys = [i.get("id", i.get("command")) for i in items]
    assert keys.index("layer") < keys.index("filter") < keys.index("first") < keys.index("view") < keys.index("configure") < keys.index("attributes")
    assert "capture" not in actions
    print("PASS built-in ribbon icons; navigation glyphs and no Hold/Capture panel control")


def check_no_capture(call):
    # Given: a fresh private CLI project with no captured features.
    before = call("plugin:quick-field-keys/state")
    project = call("info.project")
    for widget in ["first", "previous", "next", "last", "zoom-current", "attributes", "locks"]:
        assert before["panel"]["items"][widget]["enabled"] is False
    for widget in ["attributes", "locks"]:
        assert before["panel"]["items"][widget]["visible"] is False
    # When: public commands are invoked without a capture, even outside disabled panel buttons.
    for command in ["next", "first", "edit-current"]:
        error = call("plugin:quick-field-keys/" + command, error=True)
        # Then: the user sees an action, not a serde implementation detail.
        assert "EOF" not in error and "while parsing" not in error
    after = call("plugin:quick-field-keys/state")
    assert after["capture"] == before["capture"]
    assert call("info.project") == project
    print("PASS no-capture navigation/edit commands give friendly errors; controls disabled, state/project unchanged")


def check_captured_controls(result):
    for widget in ["first", "previous", "next", "last", "zoom-current", "attributes", "locks"]:
        assert result["panel"]["items"][widget]["enabled"] is True
    for widget in ["attributes", "locks"]:
        assert result["panel"]["items"][widget]["visible"] is True
