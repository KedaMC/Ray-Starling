from pathlib import Path
import re

path = Path("crates/horizon-client-render/src/options_ui/layout.rs")
source = path.read_text(encoding="utf-8")
start_marker = (
    "                let entries = [\n"
    "                    // Everywhere for the layers and Main Hand;"
)
end_marker = "                out.push(done(h - 27.0));"

if source.count(start_marker) != 1:
    raise SystemExit(
        f"Expected one upstream Main options-grid block, found {source.count(start_marker)}"
    )

start = source.index(start_marker)
end = source.index(end_marker, start) + len(end_marker)

replacement = """                // Starling Settings sits centered above the vanilla Options grid.
                let settings_key = "horizon.pages.open.horizon_settings";
                let settings_y_rows = [
                    ("options.skinCustomisation", Some(Page::Skin)),
                    ("options.sounds", Some(Page::Sound)),
                    ("options.video", Some(Page::Video)),
                    ("options.controls", Some(Page::Controls)),
                    (
                        "options.language",
                        (!self.split_guest).then_some(Page::Language),
                    ),
                    ("options.chat", Some(Page::Chat)),
                    (
                        "options.resourcepack",
                        (!self.split_guest).then_some(Page::Packs),
                    ),
                    ("options.accessibility", Some(Page::Accessibility)),
                    ("options.credits_and_attribution", Some(Page::Credits)),
                ];
                let entries = with_diagnostics(settings_y_rows);
                let rows = 1.0 + entries.len().div_ceil(2) as f32;
                let gy = (61.0 + 30.0f32).min(h - 33.0 - rows * 24.0);
                out.push(button(
                    cx - 75.0,
                    gy,
                    150.0,
                    tr(settings_key),
                    Act::Push(Page::Horizon),
                ));
                for (i, (key, page)) in entries.iter().enumerate() {
                    let x = cx - 158.0 + 4.0 + (i % 2) as f32 * 158.0;
                    let y = gy + 24.0 + (i / 2) as f32 * 24.0;
                    let b = button(x, y, 150.0, tr(key), page.map_or(Act::None, Act::Push));
                    out.push(if page.is_some() { b } else { inactive(b) });
                }
                out.push(done(h - 27.0));"""

path.write_text(source[:start] + replacement + source[end:], encoding="utf-8")
print("Restored Ray's centered Starling Settings row.")

# Keep Ray's graphics organization on top of the new 0.9.0 settings-shell API:
# Quality is part of General, while the preset, resolution-scale and battery
# controls remain hidden (the underlying upstream settings stay intact).
graphics_path = Path("crates/horizon-client-render/src/graphics_ui.rs")
graphics = graphics_path.read_text(encoding="utf-8")

old_all = """    pub const ALL: [Page; 6 + cfg!(target_os = "android") as usize] = [
        Page::General,
        #[cfg(target_os = "android")]
        Page::Display,
        Page::Quality,"""
new_all = """    pub const ALL: [Page; 5 + cfg!(target_os = "android") as usize] = [
        Page::General,
        #[cfg(target_os = "android")]
        Page::Display,"""
if graphics.count(old_all) != 1:
    raise SystemExit("Expected one upstream graphics page list")
graphics = graphics.replace(old_all, new_all, 1)

rows_start = graphics.index("pub fn rows()")
quality_row = graphics.find("            Page::Quality,", rows_start)
if quality_row < 0:
    raise SystemExit("Expected the upstream Quality settings group")
graphics = graphics[:quality_row] + graphics[quality_row:].replace(
    "            Page::Quality,", "            Page::General,", 1
)

old_return = """    pages
}

fn display_setting"""
new_return = """    // Ray keeps these controls out of the graphics UI: the preset changes
    // several unrelated options at once, resolution scale is controlled by
    // the render-path source of truth, and Battery Mode duplicates power policy.
    for (_, groups) in &mut pages {
        for (_, rows) in groups.iter_mut() {
            rows.retain(|row| {
                !matches!(
                    row.item,
                    Item::Horizon(H::Preset | H::ResolutionScale | H::BatteryMode)
                )
            });
        }
        groups.retain(|(_, rows)| !rows.is_empty());
    }
    pages
}

fn display_setting"""
if graphics.count(old_return) != 1:
    raise SystemExit("Expected one graphics rows() return block")
graphics = graphics.replace(old_return, new_return, 1)

tests_at = graphics.find("#[cfg(test)]")
if tests_at < 0:
    raise SystemExit("Expected graphics UI tests")
tests = graphics[tests_at:]
tests = tests.replace(
    "assert_eq!(settings(&mut ui, &s), on(Page::Quality));",
    "assert_eq!(settings(&mut ui, &s), on(Page::Performance));",
)
tests = tests.replace(
    "assert_eq!(ui.current_page(&s, SIZE), Some(Page::Quality));",
    "assert_eq!(ui.current_page(&s, SIZE), Some(Page::Performance));",
)
tests = tests.replace(
    "(Page::Quality, Item::Vanilla(Opt::Clouds))",
    "(Page::General, Item::Vanilla(Opt::Clouds))",
)
tests = tests.replace(
    "(Page::Quality, Item::Vanilla(Opt::CloudRange))",
    "(Page::General, Item::Vanilla(Opt::CloudRange))",
)
old_preset_test = """        // Both presets answer "preset"; Horizon's own, named apart from vanilla's, answers its name.
        ui.set_search("preset");
        let found = ui.listed(&s, SIZE);
        let general = |item| (Page::General, item);
        assert!(found.contains(&general(Item::Horizon(H::Preset))));
        assert!(found.contains(&general(Item::Vanilla(Opt::Preset))));
        ui.set_search("horizon preset");
        assert_eq!(ui.listed(&s, SIZE), [general(Item::Horizon(H::Preset))]);"""
new_preset_test = """        // Vanilla presets remain available; Ray hides Horizon's bulk preset control.
        ui.set_search("preset");
        let found = ui.listed(&s, SIZE);
        let general = |item| (Page::General, item);
        assert!(!found.contains(&general(Item::Horizon(H::Preset))));
        assert!(found.contains(&general(Item::Vanilla(Opt::Preset))));
        ui.set_search("horizon preset");
        assert!(ui.listed(&s, SIZE).is_empty());"""
if tests.count(old_preset_test) != 1:
    raise SystemExit("Expected the upstream preset search test")
tests = tests.replace(old_preset_test, new_preset_test, 1)
graphics = graphics[:tests_at] + tests
graphics_path.write_text(graphics, encoding="utf-8")
print("Restored Ray's merged General graphics page and hidden legacy controls.")

build_path = Path("packaging/android/build_apk.py")
build = build_path.read_text(encoding="utf-8")
build, name_count = re.subn(
    r'^VERSION_NAME = "[^"]+"$',
    'VERSION_NAME = "0.9.0"',
    build,
    count=1,
    flags=re.MULTILINE,
)
build, code_count = re.subn(
    r"^VERSION_CODE = \d+$",
    "VERSION_CODE = 14",
    build,
    count=1,
    flags=re.MULTILINE,
)
if name_count != 1 or code_count != 1:
    raise SystemExit(
        f"Expected one version name/code declaration, got {name_count}/{code_count}"
    )
build, app_id_count = re.subn(
    r'^APPLICATION_ID = "[^"]+"
workflow_path = Path(".github/workflows/build.yml")
workflow = workflow_path.read_text(encoding="utf-8")
old_rust = """          rustup toolchain install stable --profile minimal --target ${{ matrix.target }}
          rustup default stable"""
new_rust = """          bash tools/ci/rustup_install.sh --profile minimal --target ${{ matrix.target }}"""
if workflow.count(old_rust) != 1:
    raise SystemExit(f"Expected one floating Rust toolchain block, found {workflow.count(old_rust)}")
workflow_path.write_text(workflow.replace(old_rust, new_rust, 1), encoding="utf-8")
print("Switched Ray APK workflow to the repository-pinned Rust toolchain.")
,
    'APPLICATION_ID = "com.raystarling.strong"',
    build,
    count=1,
    flags=re.MULTILINE,
)
if name_count != 1 or code_count != 1 or app_id_count != 1:
    raise SystemExit(
        f"Expected one app id/version name/version code declaration, got "
        f"{app_id_count}/{name_count}/{code_count}"
    )
build_path.write_text(build, encoding="utf-8")
print("Set Ray package ID and APK version to com.raystarling.strong / 0.9.0 (14).")

workflow_path = Path(".github/workflows/build.yml")
workflow = workflow_path.read_text(encoding="utf-8")
old_rust = """          rustup toolchain install stable --profile minimal --target ${{ matrix.target }}
          rustup default stable"""
new_rust = """          bash tools/ci/rustup_install.sh --profile minimal --target ${{ matrix.target }}"""
if workflow.count(old_rust) != 1:
    raise SystemExit(f"Expected one floating Rust toolchain block, found {workflow.count(old_rust)}")
workflow_path.write_text(workflow.replace(old_rust, new_rust, 1), encoding="utf-8")
print("Switched Ray APK workflow to the repository-pinned Rust toolchain.")
