"""Private CLI checks for visibility-only controls and retained field drafts."""
import json


def check_settings_controls(call, layer, edited):
    # Given: a pending panel draft and navigation options, not applied data edits.
    draft = dict(edited["panel"]["values"]["attributes"])
    n_id = next(f["id"] for f in edited["panel"]["items"]["attributes"]["fields"] if f["label"] == "n")
    draft[n_id] = 32
    options = {"sort": "uid", "ascending": False, "query": "pending filter", "selected-only": False}
    view = {"zoom-mode": "feature"}
    before = call("info.project")
    # When: open general settings without changing identity.
    first = call("plugin:quick-field-keys/options", {"panel": {
        "attributes": draft, "filter": options, "view": view}})
    second = call("plugin:quick-field-keys/options")
    # Then: settings do not return replacement attribute widgets or values.
    assert "panel" not in first and "panel" not in second
    assert first["identity"] == second["identity"] == "uid"
    assert call("info.project") == before
    # Explicit navigation may now refill fields; its stale-row guard proves data stayed unchanged.
    current = call("plugin:quick-field-keys/first")["current"]
    assert current["properties"] == edited["current"]["properties"]
    assert current["properties"]["n"] == 31
    assert current["row"] == 0
    print("PASS unchanged Options does not replace pending attributes or edit features")
    # Given: a delayed old-row form, arriving after an explicit navigation command.
    stale = dict(edited["panel"]["values"]["attributes"])
    stale[n_id] = 999
    next_row = call("plugin:quick-field-keys/next")["current"]
    # When: the old row/revision field ids are submitted after moving.
    answer = call("plugin:quick-field-keys/edit-current", {"panel": {"attributes": stale}})
    # Then: host rejects old ids, retains new-row defaults; no old text edits the new row.
    assert answer["current"]["properties"] == next_row["properties"]
    assert answer["current"]["properties"]["n"] == 10
    call("plugin:quick-field-keys/first")
    print("PASS stale old-row form after Next cannot edit the newly current feature")
