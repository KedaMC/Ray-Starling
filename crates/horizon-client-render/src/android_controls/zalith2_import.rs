//! Imports a ZalithLauncher 2 control layout (`editorVersion` and `layers`) as a GoyControl preset.
//! ZalithLauncher 1 and PojavLauncher layouts (`mControlDataList`) have their own importer,
//! `pojav_import`; the two never claim the same file.
//!
//! Zalith 2 keeps its controls in layers. Layers that start hidden are left out, as are the buttons
//! that only switch layers, because GoyControl has no layers. A button is shown where both its layer
//! and its own setting allow it. Positions and sizes are fractions of the screen or dp values, so
//! they are laid out on the same reference screen as the other importer.
use super::custom::CustomButton;
use super::layout::{Control, Placement, Settings};
use super::pojav_import::{DENSITY, Imported, MAX_EVENTS, SCREEN_H, SCREEN_W, apply_corner_radius};
use serde::Deserialize;

#[derive(Deserialize)]
struct Layout {
    #[serde(rename = "editorVersion")]
    _editor_version: i64,
    layers: Vec<Layer>,
    #[serde(default)]
    styles: Vec<Style>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Layer {
    hide: bool,
    #[serde(rename = "visibilityType")]
    visibility: String,
    #[serde(rename = "normalButtons")]
    buttons: Vec<Button>,
    #[serde(rename = "textBoxes")]
    texts: Vec<serde::de::IgnoredAny>,
    #[serde(rename = "joystickButtons")]
    joysticks: Vec<Joystick>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Text {
    default: String,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(default)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Size {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "widthDp")]
    width_dp: f32,
    #[serde(rename = "heightDp")]
    height_dp: f32,
    #[serde(rename = "widthPercentage")]
    width_percentage: f32,
    #[serde(rename = "heightPercentage")]
    height_percentage: f32,
    #[serde(rename = "widthReference")]
    width_reference: String,
    #[serde(rename = "heightReference")]
    height_reference: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    key: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Button {
    text: Text,
    position: Position,
    #[serde(rename = "buttonSize")]
    size: Size,
    #[serde(rename = "buttonStyle")]
    style: Option<String>,
    #[serde(rename = "visibilityType")]
    visibility: String,
    #[serde(rename = "clickEvents")]
    events: Vec<Event>,
    #[serde(rename = "isSwipple")]
    swipeable: bool,
    #[serde(rename = "isPenetrable")]
    penetrable: bool,
    #[serde(rename = "isToggleable")]
    toggleable: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Joystick {
    position: Position,
    #[serde(rename = "sizeType")]
    size_type: String,
    #[serde(rename = "sizeDp")]
    size_dp: f32,
    #[serde(rename = "sizePercentage")]
    size_percentage: f32,
}

#[derive(Deserialize)]
#[serde(default)]
struct Style {
    uuid: String,
    #[serde(rename = "commonStyle")]
    common: bool,
    #[serde(rename = "lightStyle")]
    light: StyleConfig,
    #[serde(rename = "darkStyle")]
    dark: StyleConfig,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            uuid: String::new(),
            common: true,
            light: StyleConfig::default(),
            dark: StyleConfig::default(),
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(default)]
struct StyleConfig {
    alpha: f32,
    #[serde(rename = "backgroundColor")]
    background: i64,
    #[serde(rename = "pressedBackgroundColor")]
    pressed_background: i64,
    #[serde(rename = "borderWidth")]
    border_width: f32,
    #[serde(rename = "borderColor")]
    border_color: i64,
    #[serde(rename = "pressedBorderColor")]
    pressed_border_color: i64,
    #[serde(rename = "borderRadius")]
    radius: Corners,
}
impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            background: 0x8000_0000,
            pressed_background: 0xB380_8080,
            border_width: 0.0,
            border_color: 0xFFFF_FFFF,
            pressed_border_color: 0xFFFF_FFFF,
            radius: Corners::default(),
        }
    }
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(default)]
struct Corners {
    #[serde(rename = "topStart")]
    top_start: f32,
    #[serde(rename = "topEnd")]
    top_end: f32,
    #[serde(rename = "bottomEnd")]
    bottom_end: f32,
    #[serde(rename = "bottomStart")]
    bottom_start: f32,
}

/// True when `bytes` look like a ZalithLauncher 2 control layout.
pub fn looks_like_zalith2(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .is_some_and(|v| {
            v.get("editorVersion").is_some_and(|n| n.is_i64())
                && v.get("layers").is_some_and(|l| l.is_array())
                && v.get("mControlDataList").is_none()
        })
}

/// Converts the JSON of a ZalithLauncher 2 layout into a GoyControl layout.
pub fn convert(bytes: &[u8]) -> Result<Imported, String> {
    let layout: Layout =
        serde_json::from_slice(bytes).map_err(|e| format!("not a control layout: {e}"))?;
    let mut settings = Settings::default_goycontrol();
    let mut skipped = 0;
    let mut dropped_events = 0;
    let mut buttons = Vec::new();
    let mut stick = None;
    for layer in &layout.layers {
        if layer.hide {
            skipped += layer.buttons.len() + layer.texts.len() + layer.joysticks.len();
            continue;
        }
        skipped += layer.texts.len();
        let layer_shown = shown_in(&layer.visibility);
        for source in &layer.buttons {
            let Some((tokens, dropped)) = tokens_of(&source.events) else {
                skipped += 1;
                continue;
            };
            let shown = intersect(layer_shown, shown_in(&source.visibility));
            let Some(placement) = button_placement(source, shown, &layout.styles) else {
                skipped += 1;
                continue;
            };
            dropped_events += dropped;
            let mut button = CustomButton::new(String::new(), buttons.len());
            for (slot, token) in tokens.into_iter().enumerate() {
                button.set_event_slot(slot, token);
            }
            button.custom_label = source.text.default.trim().chars().take(12).collect();
            button.always_visible = shown.1;
            button.layouts = [placement; 3];
            buttons.push(button);
        }
        for source in &layer.joysticks {
            match joystick_placement(source) {
                Some(placement) if stick.is_none() => stick = Some(placement),
                _ => skipped += 1,
            }
        }
    }
    // The layout decides whether there is a joystick: without one GoyControl's own is hidden
    // (1 = shown, 2 = hidden in `button_visibility`).
    if let Some(placement) = stick {
        for row in &mut settings.layouts {
            row[Control::Move as usize] = placement;
        }
    } else {
        settings.button_visibility[Control::Move as usize] = 2;
    }
    let count = buttons.len();
    settings.custom_buttons = buttons;
    settings.sanitize();
    for button in &mut settings.custom_buttons {
        for placement in &mut button.layouts {
            placement.sanitize();
        }
    }
    Ok(Imported {
        settings,
        buttons: count,
        skipped,
        dropped_events,
    })
}

/// (shown while playing, shown in menus) for a `visibilityType`.
fn shown_in(visibility: &str) -> (bool, bool) {
    match visibility {
        "in_game" => (true, false),
        "in_menu" => (false, true),
        _ => (true, true),
    }
}
fn intersect(a: (bool, bool), b: (bool, bool)) -> (bool, bool) {
    (a.0 && b.0, a.1 && b.1)
}

/// Event tokens of a button (at most three) and how many were dropped. None when nothing it does
/// has an equivalent: layer switches, chat text, the launcher's keyboard and menu.
fn tokens_of(events: &[Event]) -> Option<(Vec<String>, usize)> {
    let mut tokens: Vec<String> = Vec::new();
    for event in events {
        if !matches!(event.kind.as_str(), "key" | "launcher_event") {
            continue;
        }
        let Some(token) = token_of(&event.key).filter(|t| CustomButton::valid_token(t)) else {
            continue;
        };
        if !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    if tokens.is_empty() {
        return None;
    }
    let dropped = tokens.len().saturating_sub(MAX_EVENTS);
    tokens.truncate(MAX_EVENTS);
    Some((tokens, dropped))
}

/// A Zalith 2 key, mouse or scroll name to the game's token.
fn token_of(name: &str) -> Option<String> {
    let mouse = |button: &str| Some(format!("key.mouse.{button}"));
    match name {
        "GLFW_MOUSE_BUTTON_LEFT" | "GLFW_MOUSE_BUTTON_1" => mouse("left"),
        "GLFW_MOUSE_BUTTON_RIGHT" | "GLFW_MOUSE_BUTTON_2" => mouse("right"),
        "GLFW_MOUSE_BUTTON_MIDDLE" | "GLFW_MOUSE_BUTTON_3" => mouse("middle"),
        "GLFW_MOUSE_BUTTON_4" => mouse("4"),
        "GLFW_MOUSE_BUTTON_5" => mouse("5"),
        // Zalith 2 stores the scroll wheel's "up" and "down" under these names.
        "SPECIAL_KEY_MOUSE_SCROLL_UP"
        | "SPECIAL_KEY_MOUSE_SCROLL_LEFT"
        | "launcher.event.scroll_up"
        | "launcher.event.scroll_up.single" => Some("mouse.scroll.up".into()),
        "SPECIAL_KEY_MOUSE_SCROLL_DOWN"
        | "SPECIAL_KEY_MOUSE_SCROLL_RIGHT"
        | "launcher.event.scroll_down"
        | "launcher.event.scroll_down.single" => Some("mouse.scroll.down".into()),
        _ => {
            let key = name.strip_prefix("GLFW_KEY_")?;
            let named = match key {
                "UNKNOWN" => return None,
                "LEFT_SUPER" => "left.win".to_owned(),
                "RIGHT_SUPER" => "right.win".to_owned(),
                "KP_DECIMAL" => "keypad.period".to_owned(),
                "KP_EQUAL" => "keypad.equal".to_owned(),
                k => match k.strip_prefix("KP_") {
                    Some(pad) => format!("keypad.{}", pad.to_lowercase().replace('_', ".")),
                    None => k.to_lowercase().replace('_', "."),
                },
            };
            Some(format!("key.keyboard.{named}"))
        }
    }
}

/// Size in pixels of a button on the reference screen.
fn size_px(size: &Size, label: &str) -> (f32, f32) {
    let reference = |name: &str| {
        if name == "screen_width" {
            SCREEN_W
        } else {
            SCREEN_H
        }
    };
    match size.kind.as_str() {
        "dp" => (size.width_dp * DENSITY, size.height_dp * DENSITY),
        "wrap_content" => {
            let width_dp = (label.chars().count() as f32 * 9.0 + 24.0).max(40.0);
            (width_dp * DENSITY, 36.0 * DENSITY)
        }
        _ => (
            reference(&size.width_reference) * size.width_percentage / 10_000.0,
            reference(&size.height_reference) * size.height_percentage / 10_000.0,
        ),
    }
}

/// Where a widget of `width` x `height` px sits for a Zalith 2 position: the free space of the
/// screen split by the fractions, with 10000 the right or bottom edge.
fn center_of(position: Position, width: f32, height: f32) -> [f32; 2] {
    let (x, y) = (
        (position.x / 10_000.0).clamp(0.0, 1.0),
        (position.y / 10_000.0).clamp(0.0, 1.0),
    );
    [
        ((SCREEN_W - width) * x + width / 2.0) / SCREEN_W,
        ((SCREEN_H - height) * y + height / 2.0) / SCREEN_H,
    ]
}

fn button_placement(button: &Button, shown: (bool, bool), styles: &[Style]) -> Option<Placement> {
    if !(shown.0 || shown.1) {
        return None;
    }
    let (width, height) = size_px(&button.size, &button.text.default);
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        return None;
    }
    let [x, y] = center_of(button.position, width, height);
    let mut p = Placement::new(x, y, height / SCREEN_H);
    p.width_ratio = width / height;
    p.height_ratio = 1.0;
    p.use_goy_style = true;
    p.passthrough = button.penetrable;
    p.toggleable = button.toggleable;
    p.swipeable = button.swipeable;
    p.visible_in_game = shown.0;
    p.visible_in_menu = shown.1;
    let style = button
        .style
        .as_deref()
        .and_then(|id| styles.iter().find(|s| s.uuid == id));
    // The dark style, as the game is dark; a "common" style is the one both themes share.
    let config = style.map_or_else(StyleConfig::default, |s| {
        if s.common { s.light } else { s.dark }
    });
    p.opacity = if config.alpha.is_finite() {
        config.alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    p.button_color = argb(config.background);
    p.on_click_color = argb(config.pressed_background);
    p.stroke_size = config.border_width;
    p.stroke_color = argb(config.border_color);
    p.on_click_stroke_color = argb(config.pressed_border_color);
    let r = config.radius;
    apply_corner_radius(
        &mut p,
        (r.top_start + r.top_end + r.bottom_end + r.bottom_start) / 4.0,
    );
    Some(p)
}

fn joystick_placement(joystick: &Joystick) -> Option<Placement> {
    let diameter = if joystick.size_type == "dp" {
        joystick.size_dp * DENSITY
    } else {
        SCREEN_H * joystick.size_percentage / 10_000.0
    };
    if !(diameter.is_finite() && diameter > 0.0) {
        return None;
    }
    let [x, y] = center_of(joystick.position, diameter, diameter);
    let mut p = Placement::new(x, y, diameter / SCREEN_H);
    p.use_goy_style = true;
    Some(p)
}

/// Zalith 2 stores a colour as a number: ARGB in the low 32 bits, or in the high 32 in older files.
fn argb(value: i64) -> u32 {
    let high = ((value >> 32) & 0xFFFF_FFFF) as u32;
    if high != 0 { high } else { value as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: &str = r#"{"uuid":"s1","commonStyle":false,
        "lightStyle":{"alpha":1.0,"backgroundColor":4294967295},
        "darkStyle":{"alpha":0.5,"backgroundColor":2147483648,"pressedBackgroundColor":3012069512,
        "borderWidth":2,"borderColor":4294901760,
        "borderRadius":{"topStart":0.0,"topEnd":40.0,"bottomEnd":0.0,"bottomStart":0.0}}}"#;

    fn button(text: &str, x: i32, y: i32, events: &str, visibility: &str) -> String {
        format!(
            r#"{{"text":{{"default":"{text}","matchQueue":[]}},"uuid":"b","position":{{"x":{x},"y":{y}}},
            "buttonSize":{{"type":"percentage","widthDp":50.0,"heightDp":50.0,"widthPercentage":2000,
            "heightPercentage":1000,"widthReference":"screen_height","heightReference":"screen_height"}},
            "buttonStyle":"s1","visibilityType":"{visibility}","clickEvents":{events},
            "isSwipple":true,"isPenetrable":true,"isToggleable":true}}"#
        )
    }
    fn layer(hide: bool, visibility: &str, buttons: &[String], extra: &str) -> String {
        format!(
            r#"{{"name":"l","uuid":"u","hide":{hide},"visibilityType":"{visibility}",
            "normalButtons":[{}]{extra}}}"#,
            buttons.join(",")
        )
    }
    fn layout(layers: &[String]) -> Vec<u8> {
        format!(
            r#"{{"info":{{}},"editorVersion":12,"styles":[{STYLE}],"layers":[{}]}}"#,
            layers.join(",")
        )
        .into_bytes()
    }
    fn key(name: &str) -> String {
        format!(r#"[{{"type":"key","key":"{name}"}}]"#)
    }

    #[test]
    fn a_button_keeps_its_key_position_size_and_style() {
        let json = layout(&[layer(
            false,
            "always",
            &[button(
                "Jump",
                5000,
                10000,
                &key("GLFW_KEY_SPACE"),
                "always",
            )],
            "",
        )]);
        let out = convert(&json).unwrap();
        assert_eq!((out.buttons, out.skipped), (1, 0));
        let b = &out.settings.custom_buttons[0];
        assert_eq!(b.key, "key.keyboard.space");
        assert_eq!(b.custom_label, "Jump");
        let p = b.layouts[0];
        // 20 % x 10 % of the screen height: 216 x 108 px. Centred horizontally, on the bottom edge.
        assert!((p.center[0] - 0.5).abs() < 1e-4);
        assert!((p.center[1] - (1.0 - 54.0 / 1080.0)).abs() < 1e-4);
        assert!((p.size - 0.1).abs() < 1e-4);
        assert!((p.width_ratio - 2.0).abs() < 1e-4);
        assert!(p.toggleable && p.passthrough && p.swipeable);
        // The dark style: half opaque black, a red border, a rounded top-end corner.
        assert!((p.opacity - 0.5).abs() < 1e-4);
        assert_eq!(p.button_color, 0x8000_0000);
        assert_eq!(p.on_click_color, 3_012_069_512);
        assert_eq!(p.stroke_color, 0xFFFF_0000);
        assert!((p.stroke_size - 2.0).abs() < 1e-4);
        assert_eq!(p.shape, 0);
    }

    #[test]
    fn hidden_layers_and_layer_switches_are_left_out() {
        let switch = r#"[{"type":"switch_layer","key":"x"}]"#;
        let json = layout(&[
            layer(
                false,
                "always",
                &[
                    button("A", 0, 0, &key("GLFW_KEY_A"), "always"),
                    button("Tab", 0, 0, switch, "always"),
                    button(
                        "Both",
                        0,
                        0,
                        r#"[{"type":"switch_layer","key":"x"},{"type":"key","key":"GLFW_KEY_B"}]"#,
                        "always",
                    ),
                ],
                "",
            ),
            layer(
                true,
                "always",
                &[button("H", 0, 0, &key("GLFW_KEY_H"), "always")],
                "",
            ),
        ]);
        let out = convert(&json).unwrap();
        let keys: Vec<_> = out.settings.custom_buttons.iter().map(|b| &b.key).collect();
        assert_eq!(keys, ["key.keyboard.a", "key.keyboard.b"]);
        assert_eq!(out.skipped, 2);
    }

    #[test]
    fn layer_and_button_visibility_both_apply() {
        let json = layout(&[
            layer(
                false,
                "in_game",
                &[button("A", 0, 0, &key("GLFW_KEY_A"), "always")],
                "",
            ),
            layer(
                false,
                "always",
                &[button("B", 0, 0, &key("GLFW_KEY_B"), "in_menu")],
                "",
            ),
            layer(
                false,
                "in_game",
                &[button("C", 0, 0, &key("GLFW_KEY_C"), "in_menu")],
                "",
            ),
        ]);
        let out = convert(&json).unwrap();
        let seen: Vec<_> = out
            .settings
            .custom_buttons
            .iter()
            .map(|b| {
                (
                    &b.key[..],
                    b.layouts[0].visible_in_game,
                    b.layouts[0].visible_in_menu,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("key.keyboard.a", true, false),
                ("key.keyboard.b", false, true)
            ]
        );
        // C is shown nowhere, so it is left out.
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn mouse_scroll_and_extra_keys_map_to_the_games_tokens() {
        let events = r#"[{"type":"launcher_event","key":"GLFW_MOUSE_BUTTON_RIGHT"},
            {"type":"launcher_event","key":"launcher.event.scroll_down"},
            {"type":"key","key":"GLFW_KEY_LEFT_SUPER"},{"type":"key","key":"GLFW_KEY_KP_DECIMAL"}]"#;
        let json = layout(&[layer(
            false,
            "always",
            &[button("m", 0, 0, events, "always")],
            "",
        )]);
        let out = convert(&json).unwrap();
        let b = &out.settings.custom_buttons[0];
        assert_eq!(
            (&b.key[..], &b.second_key[..], &b.third_key[..]),
            (
                "key.mouse.right",
                "mouse.scroll.down",
                "key.keyboard.left.win"
            )
        );
        assert_eq!(out.dropped_events, 1);
    }

    #[test]
    fn buttons_with_no_equivalent_are_skipped() {
        let ime = r#"[{"type":"launcher_event","key":"launcher.event.switch_ime"}]"#;
        let chat = r#"[{"type":"send_text","key":"hello"}]"#;
        let json = layout(&[layer(
            false,
            "always",
            &[
                button("kb", 0, 0, ime, "always"),
                button("hi", 0, 0, chat, "always"),
            ],
            r#","textBoxes":[{"uuid":"t"}]"#,
        )]);
        let out = convert(&json).unwrap();
        assert_eq!((out.buttons, out.skipped), (0, 3));
    }

    #[test]
    fn the_joystick_moves_the_movement_control_and_without_one_it_is_hidden() {
        let stick = r#","joystickButtons":[{"uuid":"j","position":{"x":0,"y":10000},"sizePercentage":3000}]"#;
        let with = convert(&layout(&[layer(false, "always", &[], stick)])).unwrap();
        let p = with.settings.layouts[0][Control::Move as usize];
        // 30 % of the screen height, in the bottom-left corner.
        assert!((p.size - 0.3).abs() < 1e-4);
        assert!((p.center[0] - 162.0 / 2400.0).abs() < 1e-4);
        assert!((p.center[1] - (1.0 - 162.0 / 1080.0)).abs() < 1e-4);
        assert!(with.settings.visible(Control::Move));
        let without = convert(&layout(&[layer(false, "always", &[], "")])).unwrap();
        assert!(!without.settings.visible(Control::Move));
    }

    #[test]
    fn only_zalith2_layouts_are_recognised() {
        assert!(looks_like_zalith2(&layout(&[])));
        assert!(!looks_like_zalith2(br#"{"layers":[]}"#));
        assert!(!looks_like_zalith2(
            br#"{"editorVersion":12,"layers":[],"mControlDataList":[]}"#
        ));
        assert!(!looks_like_zalith2(b"not json"));
        // Neither importer takes the other's files.
        assert!(!super::super::pojav_import::looks_like_pojav(&layout(&[])));
        assert!(!looks_like_zalith2(
            br#"{"version":8,"mControlDataList":[],"mJoystickDataList":[]}"#
        ));
    }
}
