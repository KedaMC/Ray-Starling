from pathlib import Path

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
