# Quick Field Keys for RustGIS

Version 0.1.0, official-SDK supported scope. **Not full QGIS feature parity.**
The required lock/unlock icon beside each attribute input is not implemented.
The current separate lock-checkbox form is an interim alternative, not QGIS UI
parity. The agreed scope is now the closest practical QGIS workflow using the
official SDK; upstream-only features need not block a clearly documented release.

## Workflow

1. Choose the editing layer at the top of the panel. This prepares navigation;
   no Hold or Capture button is required. The editing target is independent of
   the host's active layer.
2. Choose sorting, text/SQL filtering and the selected-only scope. Enter a text
   filter or a RustGIS SQL condition directly. **↻** applies the filter and
   rebuilds navigation. Clear filter empties the input; ↻ applies it.
   The requested QGIS-style expression editor is not implemented. The rejected
   simple condition dialog is not exposed in the panel or command registry.
3. Navigate with First, Previous, Next and Last. Choose fit-feature, keep-scale,
   fixed-scale or off for navigation zoom. Previous selection / view restores
   the prior target-layer selection and camera, not attribute edits.
4. Configure preset slots 1–9 with Values. Apply commands are available for
   host shortcut assignment; there are no per-preset Apply buttons.
5. Use Columns to stage visible columns, then Apply columns or Cancel. Hidden
   values, preset assignments and field locks are retained. Only column names
   are remembered across processes; feature values and Undo are not settings.
6. Edit the current feature. Undo feature edits reverts the last plugin edit
   group and reveals its feature. Consecutive edits to one feature are grouped.
7. **Press Save to apply changes to the original file.** Save writes all pending
   edits of the designated layer, including other tools' edits, without a
   confirmation dialog. It is not project Save. Saved edits cannot be restored
   by feature Undo.

## QGIS comparison

This is not full QGIS feature parity. Comparison baseline:
`qgis-quick-field-keys-python-release/quick_field_keys`, version 2.0.0.

| Capability | RustGIS status / difference |
| --- | --- |
| Designated layer; automatic preparation | Implemented; active layer is separate from the target. |
| First/previous/next/last; sorting; selected-only scope | Implemented. Reload rebuilds the frozen navigation list. |
| Four zoom modes; prior selection/view | Implemented, with projection/context guards. |
| Visible columns; Apply/Cancel; remembered choices | Implemented; column names only are persisted. |
| Typed presets, NULL and inline editing | Implemented for the current captured feature. |
| Edit all selected features in one action | Not implemented; preserving a selection or navigating a selected-only list is not multi-feature editing. |
| Field lock protection | Implemented for plugin edits; not a provider security permission. |
| Lock icon immediately right of each input | Not implemented; current SDK form schema lacks the trailing action. Upstream proposal pending. |
| Toggle the focused field's lock by shortcut | Not implemented. |
| Feature Undo | Implemented, guarded against external value changes and locked fields; not the host's global Undo. |
| QGIS's 200 edit groups | Not equivalent: RustGIS history is limited to 4,000 UTF-8 bytes. |
| Filter expression | Text contains or RustGIS SQL, not full QGIS expressions. SQL uses the host's drawn-feature scope, including definition/time/display filters. |
| QGIS-style expression builder | Not implemented. Needs a real expression editor, field/value and function browsing, help and validation. The rejected simple condition form is excluded, not a substitute. Native editor integration has been requested upstream. |
| Previous/next map theme | Not implemented; SDK feasibility has not been established. |
| Flash selected features | Not implemented; no supported plugin API has been established. |
| Shortcut settings and default bindings | Commands can use host shortcut settings. Direct opening of that editor and automatic defaults are not implemented. |
| Independent editor; staged preset Apply/discard | Not implemented. Current-feature buffer edits and source Save are distinct. |
| Project reopen/session/layout restoration | Not equivalent. No matching project-load generation or lifecycle guarantee. |
| Source Save | Whole local SHP/GeoJSON only. GeoPackage/PostGIS Save is not supported by this plugin. |

## Limits and safety

- Capture payload and plugin Undo history each have a 4,000-byte budget. This
  usually limits usable data before the 10,000-feature layer guard; it is not
  a promise that 10,000 features can be captured.
- Save, target changes and navigation reload clear plugin Undo history.
  Reload keeps buffer edits but discards the history needed to undo them.
- External source modifications and host IO failures are refused. If the
  source write succeeds but panel preparation fails, the write remains
  committed; a preparation error does not imply disk rollback.
- Session values, presets and history are not promised to survive reopening a
  project. Only visible-column names are deliberately persisted by this plugin.
- `edit` and `files` permissions are declared for guarded host editing and
  supported source writes. Do not use the development build as an unrestricted
  database/source editor.

## Verification and release gate

Current supported-scope WASM:

`f9cb5610eba32de6485c63ce5c88fceff79b4883f79f3d2dc0e9f233d257e640`

Size: 483,604 bytes. The rejected condition-dialog command and panel button are
not registered. Its source files remain in the development tree, outside the
release WASM; pure SQL-generation tests remain test-only.

This build passed 26 unit tests, native/WASM Clippy, release compilation,
registry schema/module checks and real polygon-host tests. The host test also
attempted the removed command and confirmed it could not run or change state.
These checks are not a claim of final GUI approval or package publication.

The same hash also passed all six existing-host regression suites: automatic
preparation, columns, CLI smoke, source publish, Undo and polygons, reusing the
existing fixture roots. The second-field constraint-failure case confirmed
rollback of the first field's write in the real host.

Current-build GUI checks reused one Windows process/profile and its eight
polygons: SQL refresh navigated IDs 3–6; keyboard editing, moving to another
feature and Undo restored uid=6's original value; explicit Save wrote the tested
value to the original fixture and disabled Undo. Native Tab and Shift+Tab
navigation were exercised. These are Windows observations, not Linux/macOS or
all-version compatibility claims.

Historical experimental build, not the current release candidate:

`533108cf7d0a061189c057511f32acdc79b063adae12693c7490786b04382547`

- That build: 26 unit tests, native/WASM Clippy, release build, registry
  schema/module checks and polygon integration tests passed. Builder-only
  submissions preserved capture, Undo and project state; a generated AND query
  reloaded to IDs 3–6; bad target/field/operator/numeric input was rejected.
  These tests validate the experimental condition form, not the requested
  QGIS-style editor. The form was subsequently rejected and excluded.
- Prior build `9482b77b1c5a45f530a7bfa402d353402f2c061fe4cc2a8b727aad37cb6cec90`:
  existing GUI process reused; Save instruction
  displayed; editing followed by navigation and Undo restored uid=2's original
  value; Save wrote `undo-gui-saved` to that fixture and disabled Undo; eight
  fixture features remained.
- Prior build `6da4ad9e2052deebe90edcb45a3bef502ce75fd61ae6171dbe41203250a272cd`:
  21 unit tests, native/WASM Clippy, Undo and polygon tests, automatic
  preparation, columns, CLI smoke, source-publish tests and registry schema/module
  checks passed. These are not a claim that all tests ran on the newer hash.
- The visual review uses light/dark captures at 1280×800 and 1440×900, including
  actual panel-bottom coverage. Older-build screenshots do not prove this build.
- Publication requires the independent GUI review and package validation to
  pass. GitHub publication and registry submission/approval are separate states;
  see their live pages rather than assuming one implies the other.

Licensed GPL-3.0-or-later. The navigation design follows the QGIS Quick Field
Keys workflow, whose design draws on FeatureNavEd by Irene Jaya (MIT).
`LICENSE.FeatureNavEd` preserves that notice. No QGIS Qt runtime is embedded.

## Implementation plan

Keep the comparison table above current as features change. A command declaration
alone is not proof of a working feature. Mark completion only after the relevant
test or real GUI check, and identify the build used for that evidence.

| Next work | Decision / completion evidence |
| --- | --- |
| Full latest-build regression | Reuse existing test roots; record the actual build hash and test exits. |
| Latest GUI review | Reuse the existing process/profile/polygons; check light/dark, narrow/wide sizes, keyboard and actual displayed labels. |
| User-facing labels and help | Replace stale Hold/Capture terminology and autosave warnings with the actual button/action instructions; keep buffer editing distinct from source Save. |
| Missing QGIS actions | Inspect official host commands for map themes, focused-field locking and selected-feature batch edits. Implement only a verified supported route with target/lock/Undo tests; otherwise retain a documented difference. These features are not yet promised or completed. |
| QGIS-style expression editor | Await an approved native-editor integration route. Do not reintroduce the rejected field/operator/value form. Direct SQL input and ↻ remain available. |
| SDK-only parity | Await the upstream trailing-action design; keep lock checkboxes identified as an interim UI. Do not patch the installed host. |
| Package and publish | After the supported-scope gate, validate the final manifest/module/package, calculate hashes, publish, re-download assets and verify them, then submit the registry entry. Registry submission is not approval. |

Do not rerun completed tests solely to create new evidence folders. Do not count
the feature-parity audit itself as implementation of the missing actions.

SDK proposal: <https://github.com/opengeos/RustGIS/issues/2315#issuecomment-6940232717>

Native expression-editor request:
<https://github.com/opengeos/RustGIS/issues/2315#issuecomment-6941013235>

The QGIS Python plugin is a separate distribution:
<https://github.com/HaYanJongSeong/qgis-quick-field-keys/releases/tag/v2.0.0>.
Its official repository approval is separate from GitHub release publication.
