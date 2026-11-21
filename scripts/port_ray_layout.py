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
# Quality is part of General; Horizon's bulk preset, resolution-scale and
# Battery Mode controls remain hidden, as in Ray's existing graphics UI.
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
    "focus_on(&mut ui, Item::Horizon(H::Preset));",
    "focus_on(&mut ui, Item::Horizon(H::LodDistance));",
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


# Ray's Discord Rich Presence switch remains a Horizon Settings option in 0.9.0.
extras_path = Path("crates/horizon-client-render/src/extras.rs")
extras = extras_path.read_text(encoding="utf-8")

def replace_once(source, old, new, label):
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"Expected one {label}, found {count}")
    return source.replace(old, new, 1)

extras = replace_once(
    extras,
    "    pub repro_keys: bool,\n    /// Another controller can join P1's world",
    "    pub repro_keys: bool,\n    /// Shows Playing Horizon on the player's Discord profile (on by default).\n    pub discord_presence: bool,\n    /// Another controller can join P1's world",
    "Discord Presence field",
)
extras = replace_once(
    extras,
    "            repro_keys: false,\n            splitscreen: false,",
    "            repro_keys: false,\n            discord_presence: true,\n            splitscreen: false,",
    "Discord Presence default",
)
extras = replace_once(
    extras,
    '                "reproKeys" => e.repro_keys = flag(e.repro_keys),\n                "splitscreen" =>',
    '                "reproKeys" => e.repro_keys = flag(e.repro_keys),\n                "discordPresence" => e.discord_presence = flag(e.discord_presence),\n                "splitscreen" =>',
    "Discord Presence parser",
)
extras = replace_once(
    extras,
    r"reproKeys:{}\nsplitscreen:{}",
    r"reproKeys:{}\ndiscordPresence:{}\nsplitscreen:{}",
    "Discord Presence serialization key",
)
extras = replace_once(
    extras,
    "            bool_text(self.repro_keys),\n            bool_text(self.splitscreen),",
    "            bool_text(self.repro_keys),\n            bool_text(self.discord_presence),\n            bool_text(self.splitscreen),",
    "Discord Presence serialization value",
)
extras = replace_once(
    extras,
    "    ReproKeys,\n    /// On the Splitscreen settings page.",
    "    ReproKeys,\n    /// On the Horizon Settings page.\n    DiscordPresence,\n    /// On the Splitscreen settings page.",
    "Discord Presence item",
)
extras = replace_once(
    extras,
    '            Item::ReproKeys => "horizon.extras.repro_keys",',
    '            Item::ReproKeys => "horizon.extras.repro_keys",\n            Item::DiscordPresence => "horizon.extras.discord_presence",',
    "Discord Presence caption key",
)
extras = replace_once(
    extras,
    '            Item::ReproKeys => "horizon.extras.repro_keys.tooltip",',
    '            Item::ReproKeys => "horizon.extras.repro_keys.tooltip",\n            Item::DiscordPresence => "horizon.extras.discord_presence.tooltip",',
    "Discord Presence description key",
)
extras = replace_once(
    extras,
    '            Item::ReproKeys => "Recording shortcuts",',
    '            Item::ReproKeys => "Recording shortcuts",\n            Item::DiscordPresence => "Discord Rich Presence",',
    "Discord Presence caption",
)
extras = replace_once(
    extras,
    "| AutoReconnect | NoAi | AgentControl | ReproKeys | Splitscreen",
    "| AutoReconnect | NoAi | AgentControl | ReproKeys | DiscordPresence | Splitscreen",
    "Discord Presence parent match",
)
extras = replace_once(
    extras,
    '            Item::Splitscreen => {\n                "In a world, another controller can join as a new player:',
    '            Item::DiscordPresence => {\n                "Shows Playing Horizon on your Discord profile while Discord runs: menus, single player or multiplayer, the dimension and the time played. Never a world\\'s name or a server\\'s address."\n            }\n            Item::Splitscreen => {\n                "In a world, another controller can join as a new player:',
    "Discord Presence description",
)
extras = replace_once(
    extras,
    "| Item::Splitscreen | Item::WheelCyclesSettings | Item::PackBrowser => Some(Cost::None),",
    "| Item::DiscordPresence | Item::Splitscreen | Item::WheelCyclesSettings | Item::PackBrowser => Some(Cost::None),",
    "Discord Presence cost",
)
extras = replace_once(
    extras,
    "            Item::ReproKeys => e.repro_keys,\n            Item::Splitscreen =>",
    "            Item::ReproKeys => e.repro_keys,\n            Item::DiscordPresence => e.discord_presence,\n            Item::Splitscreen =>",
    "Discord Presence switch",
)
extras = replace_once(
    extras,
    "            Item::ReproKeys => e.repro_keys = !e.repro_keys,\n            Item::Splitscreen =>",
    "            Item::ReproKeys => e.repro_keys = !e.repro_keys,\n            Item::DiscordPresence => e.discord_presence = !e.discord_presence,\n            Item::Splitscreen =>",
    "Discord Presence cycle",
)
extras = replace_once(
    extras,
    "        assert!(!d.wheel_cycles_settings);",
    "        assert!(!d.wheel_cycles_settings);\n        assert!(d.discord_presence);",
    "Discord Presence default test",
)
extras_path.write_text(extras, encoding="utf-8")
print("Restored Ray's Discord Rich Presence setting on the upstream 0.9.0 model.")

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
build, app_id_count = re.subn(
    r'^APPLICATION_ID = "[^"]+"$',
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

workflow_path = Path(".github/workflows/android.yml")
workflow = workflow_path.read_text(encoding="utf-8")
target = "$" + "{{ matrix.target }}"
old_rust = (
    "          rustup toolchain install stable --profile minimal --target " + target + "\n"
    "          rustup default stable"
)
new_rust = "          bash tools/ci/rustup_install.sh --profile minimal --target " + target
if workflow.count(old_rust) == 1:
    workflow = workflow.replace(old_rust, new_rust, 1)
elif "bash tools/ci/rustup_install.sh --profile minimal --component rust-src --target" not in workflow:
    raise SystemExit(
        "Android workflow does not use the repository-pinned Rust installer"
    )
workflow_path.write_text(workflow, encoding="utf-8")
print("Verified Ray APK workflow uses the repository-pinned Rust toolchain.")
