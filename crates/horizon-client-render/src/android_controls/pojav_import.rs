//! Imports control layouts saved by PojavLauncher-family launchers (PojavLauncher, ZalithLauncher 1,
//! and forks that keep `CustomControls` JSON: `mControlDataList`, `mDrawerDataList`,
//! `mJoystickDataList`) as a GoyControl preset.
//!
//! Positions in those files are arithmetic expressions over the screen size, so they are evaluated
//! against a reference screen and stored as fractions, which is what `Placement` keeps.
use super::custom::CustomButton;
use super::layout::{Control, ControlStyle, Placement, PresetMeta, Settings};
use serde::Deserialize;

/// Reference screen the position expressions are evaluated on (landscape phone), in pixels.
const SCREEN_W: f32 = 2400.0;
const SCREEN_H: f32 = 1080.0;
/// Pixels per dp on the reference screen (xxhdpi-ish).
const DENSITY: f32 = 2.75;
/// Pojav's `${margin}`: two dp.
const MARGIN_DP: f32 = 2.0;
/// GoyControl buttons carry at most this many simultaneous events.
const MAX_EVENTS: usize = 3;

#[derive(Deserialize, Default)]
#[serde(default)]
struct PojavLayout {
    #[serde(rename = "scaledAt")]
    scaled_at: Option<f32>,
    #[serde(rename = "mControlDataList")]
    controls: Option<Vec<PojavButton>>,
    #[serde(rename = "mDrawerDataList")]
    drawers: Vec<serde_json::Value>,
    #[serde(rename = "mJoystickDataList")]
    joysticks: Vec<PojavJoystick>,
}

#[derive(Deserialize)]
#[serde(default)]
struct PojavButton {
    name: String,
    #[serde(rename = "dynamicX")]
    dynamic_x: String,
    #[serde(rename = "dynamicY")]
    dynamic_y: String,
    keycodes: Vec<i64>,
    #[serde(rename = "isToggle")]
    is_toggle: bool,
    #[serde(rename = "passThruEnabled")]
    pass_thru: bool,
    opacity: f32,
    #[serde(rename = "bgColor")]
    bg_color: i64,
    #[serde(rename = "strokeColor")]
    stroke_color: i64,
    #[serde(rename = "strokeWidth")]
    stroke_width: f32,
    #[serde(rename = "cornerRadius")]
    corner_radius: f32,
    #[serde(rename = "isSwipeable")]
    swipeable: bool,
    #[serde(rename = "displayInGame")]
    display_in_game: bool,
    #[serde(rename = "displayInMenu")]
    display_in_menu: bool,
    width: f32,
    height: f32,
}
impl Default for PojavButton {
    fn default() -> Self {
        Self {
            name: String::new(),
            dynamic_x: "0".into(),
            dynamic_y: "0".into(),
            keycodes: Vec::new(),
            is_toggle: false,
            pass_thru: false,
            opacity: 1.0,
            bg_color: 0x4D00_0000,
            stroke_color: 0xFFFF_FFFF,
            stroke_width: 0.0,
            corner_radius: 0.0,
            swipeable: false,
            display_in_game: true,
            display_in_menu: true,
            width: 50.0,
            height: 50.0,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PojavJoystick {
    #[serde(flatten)]
    base: PojavButton,
    absolute: bool,
}

/// What an import made, and what it could not carry over.
#[derive(Debug)]
pub struct Imported {
    pub settings: Settings,
    pub buttons: usize,
    /// Buttons left out: drawers, launcher-only actions and positions that did not evaluate.
    pub skipped: usize,
    /// Keys beyond the third on a button.
    pub dropped_events: usize,
}

/// True when `bytes` look like a Pojav-family control layout.
pub fn looks_like_pojav(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .is_some_and(|v| v.get("mControlDataList").is_some_and(|l| l.is_array()))
}

/// Converts the JSON of a Pojav-family layout into a GoyControl layout.
pub fn convert(bytes: &[u8]) -> Result<Imported, String> {
    let layout: PojavLayout =
        serde_json::from_slice(bytes).map_err(|e| format!("not a control layout: {e}"))?;
    let controls = layout
        .controls
        .ok_or("not a PojavLauncher/ZalithLauncher control layout")?;
    let scale = layout.scaled_at.filter(|s| s.is_finite() && *s > 0.0);
    let mut skipped = layout.drawers.len();
    let mut dropped_events = 0;
    let mut settings = Settings::default_goycontrol();
    let mut buttons = Vec::new();
    for source in &controls {
        let Some((tokens, dropped)) = tokens_of(source) else {
            skipped += 1;
            continue;
        };
        let Some(placement) = placement_of(source, scale) else {
            skipped += 1;
            continue;
        };
        dropped_events += dropped;
        let mut button = CustomButton::new(String::new(), buttons.len());
        for (slot, token) in tokens.into_iter().enumerate() {
            button.set_event_slot(slot, token);
        }
        button.custom_label = source.name.trim().chars().take(12).collect();
        button.always_visible = source.display_in_menu;
        button.layouts = [placement; 3];
        buttons.push(button);
    }
    // The layout decides whether there is a joystick: without one, GoyControl's own is hidden
    // (1 = shown, 2 = hidden in `button_visibility`), as the buttons are all the player has.
    let stick = layout
        .joysticks
        .first()
        .and_then(|stick| Some((stick, placement_of(&stick.base, scale)?)));
    if let Some((stick, mut placement)) = stick {
        placement.use_goy_style = true;
        for row in &mut settings.layouts {
            row[Control::Move as usize] = placement;
        }
        settings.dynamic_joystick = stick.absolute;
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

/// Event tokens of a button (at most three) and how many keys were dropped. None when the button
/// only triggers something the launcher does (on-screen keyboard, hiding the controls).
fn tokens_of(button: &PojavButton) -> Option<(Vec<String>, usize)> {
    let mut tokens: Vec<String> = Vec::new();
    for (slot, &code) in button.keycodes.iter().enumerate() {
        // -1 is both GLFW_KEY_UNKNOWN (an empty slot) and the on-screen keyboard button.
        if code == -1 && slot == 0 && button.name.trim().eq_ignore_ascii_case("keyboard") {
            return None;
        }
        let token = match code {
            -2 => return None,
            -3 => "key.mouse.left".to_owned(),
            -4 => "key.mouse.right".to_owned(),
            -5 => "mouse.virtual.toggle".to_owned(),
            -6 => "key.mouse.middle".to_owned(),
            -7 => "mouse.scroll.up".to_owned(),
            -8 => "mouse.scroll.down".to_owned(),
            -9 => "key.keyboard.escape".to_owned(),
            c => match glfw_token(c) {
                Some(t) if CustomButton::valid_token(&t) => t,
                _ => continue,
            },
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

fn placement_of(button: &PojavButton, scale: Option<f32>) -> Option<Placement> {
    let vars = Vars {
        width: button.width * DENSITY,
        height: button.height * DENSITY,
        scale: scale.unwrap_or(100.0),
    };
    let x = vars.eval(&button.dynamic_x)?;
    let y = vars.eval(&button.dynamic_y)?;
    if !(vars.width > 0.0 && vars.height > 0.0) {
        return None;
    }
    let size = vars.height / SCREEN_H;
    let mut p = Placement::new(
        (x + vars.width / 2.0) / SCREEN_W,
        (y + vars.height / 2.0) / SCREEN_H,
        size,
    );
    p.width_ratio = vars.width / vars.height;
    p.height_ratio = 1.0;
    p.use_goy_style = true;
    p.opacity = if button.opacity.is_finite() {
        button.opacity
    } else {
        1.0
    };
    p.passthrough = button.pass_thru;
    p.toggleable = button.is_toggle;
    p.swipeable = button.swipeable;
    p.button_color = button.bg_color as i32 as u32;
    p.stroke_color = button.stroke_color as i32 as u32;
    p.stroke_size = button.stroke_width;
    let radius = if button.corner_radius.is_finite() {
        button.corner_radius.clamp(0.0, 100.0)
    } else {
        0.0
    };
    p.shape = match radius {
        r if r <= 0.0 => 1,
        r if r >= 100.0 => 2,
        _ => 0,
    };
    p.roundness = radius / 2.0;
    p.stroke_roundness = radius / 2.0;
    p.visible_in_game = button.display_in_game;
    p.visible_in_menu = button.display_in_menu;
    Some(p)
}

/// GLFW key code to the key's name in the game's key table.
fn glfw_token(code: i64) -> Option<String> {
    let named = |name: &str| Some(format!("key.keyboard.{name}"));
    match code {
        32 => named("space"),
        39 => named("apostrophe"),
        44 => named("comma"),
        45 => named("minus"),
        46 => named("period"),
        47 => named("slash"),
        48..=57 => named(&((code - 48) as u8).to_string()),
        59 => named("semicolon"),
        61 => named("equal"),
        65..=90 => named(&((code as u8 - 65 + b'a') as char).to_string()),
        91 => named("left.bracket"),
        92 => named("backslash"),
        93 => named("right.bracket"),
        96 => named("grave.accent"),
        161 => named("world.1"),
        256 => named("escape"),
        257 => named("enter"),
        258 => named("tab"),
        259 => named("backspace"),
        260 => named("insert"),
        261 => named("delete"),
        262 => named("right"),
        263 => named("left"),
        264 => named("down"),
        265 => named("up"),
        266 => named("page.up"),
        267 => named("page.down"),
        268 => named("home"),
        269 => named("end"),
        280 => named("caps.lock"),
        281 => named("scroll.lock"),
        282 => named("num.lock"),
        283 => named("print.screen"),
        284 => named("pause"),
        290..=314 => named(&format!("f{}", code - 289)),
        320..=329 => named(&format!("keypad.{}", code - 320)),
        330 => named("keypad.period"),
        331 => named("keypad.divide"),
        332 => named("keypad.multiply"),
        333 => named("keypad.subtract"),
        334 => named("keypad.add"),
        335 => named("keypad.enter"),
        336 => named("keypad.equal"),
        340 => named("left.shift"),
        341 => named("left.control"),
        342 => named("left.alt"),
        343 => named("left.win"),
        344 => named("right.shift"),
        345 => named("right.control"),
        346 => named("right.alt"),
        347 => named("right.win"),
        348 => named("menu"),
        _ => None,
    }
}

struct Vars {
    width: f32,
    height: f32,
    scale: f32,
}
impl Vars {
    /// Evaluates a position expression the way Pojav does: `${...}` variables, `+ - * / ^`,
    /// parentheses, and `dp()` / `px()`.
    fn eval(&self, expr: &str) -> Option<f32> {
        let text = self.substitute(expr)?;
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.sum()?;
        parser.skip();
        (parser.at == parser.bytes.len() && value.is_finite()).then_some(value)
    }
    fn substitute(&self, expr: &str) -> Option<String> {
        let mut out = String::with_capacity(expr.len());
        let mut rest = expr;
        while let Some(start) = rest.find("${") {
            out.push_str(&rest[..start]);
            let end = rest[start..].find('}')? + start;
            let value = match &rest[start + 2..end] {
                "top" | "left" => 0.0,
                "right" => SCREEN_W - self.width,
                "bottom" => SCREEN_H - self.height,
                "width" => self.width,
                "height" => self.height,
                "screen_width" => SCREEN_W,
                "screen_height" => SCREEN_H,
                "margin" => MARGIN_DP * DENSITY,
                "preferred_scale" => self.scale,
                _ => return None,
            };
            out.push_str(&format!("({value})"));
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        Some(out)
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Parser<'_> {
    fn skip(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.skip();
        let hit = self.bytes.get(self.at) == Some(&c);
        if hit {
            self.at += 1;
        }
        hit
    }
    fn sum(&mut self) -> Option<f32> {
        let mut v = self.product()?;
        loop {
            if self.eat(b'+') {
                v += self.product()?;
            } else if self.eat(b'-') {
                v -= self.product()?;
            } else {
                return Some(v);
            }
        }
    }
    fn product(&mut self) -> Option<f32> {
        let mut v = self.unary()?;
        loop {
            if self.eat(b'*') {
                v *= self.unary()?;
            } else if self.eat(b'/') {
                v /= self.unary()?;
            } else {
                return Some(v);
            }
        }
    }
    fn unary(&mut self) -> Option<f32> {
        if self.eat(b'-') {
            return Some(-self.unary()?);
        }
        if self.eat(b'+') {
            return self.unary();
        }
        let base = self.atom()?;
        Some(if self.eat(b'^') {
            base.powf(self.unary()?)
        } else {
            base
        })
    }
    fn atom(&mut self) -> Option<f32> {
        if self.eat(b'(') {
            let v = self.sum()?;
            return self.eat(b')').then_some(v);
        }
        self.skip();
        let start = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'.' || *c == b'_')
        {
            self.at += 1;
        }
        let word = std::str::from_utf8(&self.bytes[start..self.at]).ok()?;
        if word.is_empty() {
            return None;
        }
        if let Ok(n) = word.parse::<f32>() {
            return Some(n);
        }
        // dp(x): pixels to dp; px(x): dp to pixels.
        if !self.eat(b'(') {
            return None;
        }
        let arg = self.sum()?;
        if !self.eat(b')') {
            return None;
        }
        match word {
            "dp" => Some(arg / DENSITY),
            "px" => Some(arg * DENSITY),
            _ => None,
        }
    }
}

impl Settings {
    /// Stores `settings` as a new preset of its control system without making it the active one,
    /// and returns what the preset list shows for it.
    pub fn import_preset(
        mut settings: Settings,
        game_home: &std::path::Path,
        name: &str,
        description: &str,
    ) -> std::io::Result<PresetMeta> {
        let style = ControlStyle::GoyControl;
        let shown: String = name.trim().chars().take(48).collect();
        if shown.is_empty() || shown.eq_ignore_ascii_case("default") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the layout needs a name other than Default",
            ));
        }
        let id = Self::allocate_unique_preset_id(game_home, style, &shown);
        settings.control_style = style;
        settings.preset_id = id.clone();
        settings.active_profile = id.clone();
        settings.preset_name = shown.clone();
        settings.preset_description = description.chars().take(120).collect();
        settings.sanitize();
        settings.save_to(&Self::system_dir(game_home, style).join(format!("{id}.json")))?;
        Ok(PresetMeta {
            id,
            name: shown,
            description: settings.preset_description,
            system: style,
            is_default: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(name: &str, x: &str, y: &str, keys: &str) -> String {
        format!(
            r#"{{"name":"{name}","dynamicX":"{x}","dynamicY":"{y}","keycodes":{keys},
            "width":50.0,"height":50.0,"opacity":0.8,"bgColor":-872415232,"strokeColor":-1,
            "strokeWidth":2.0,"cornerRadius":30.0,"isToggle":true,"passThruEnabled":true,
            "displayInGame":true,"displayInMenu":false}}"#
        )
    }
    fn layout(buttons: &[String]) -> Vec<u8> {
        format!(
            r#"{{"version":8,"scaledAt":100.0,"mControlDataList":[{}],"mDrawerDataList":[],"mJoystickDataList":[]}}"#,
            buttons.join(",")
        )
        .into_bytes()
    }

    #[test]
    fn a_button_keeps_its_key_position_and_look() {
        let json = layout(&[button(
            "JUMP",
            "${screen_width} / 2 - ${width} / 2",
            "${bottom}",
            "[32,-1,-1,-1]",
        )]);
        let out = convert(&json).unwrap();
        assert_eq!((out.buttons, out.skipped), (1, 0));
        let b = &out.settings.custom_buttons[0];
        assert_eq!(b.key, "key.keyboard.space");
        assert!(b.second_key.is_empty());
        assert_eq!(b.custom_label, "JUMP");
        let p = b.layouts[0];
        // x: centred. y: the bottom edge on a 1080 px screen with a 50 dp (137.5 px) button.
        assert!((p.center[0] - 0.5).abs() < 1e-4);
        assert!((p.center[1] - (1.0 - 137.5 / 2160.0)).abs() < 1e-4);
        assert!((p.size - 137.5 / 1080.0).abs() < 1e-4);
        assert!(p.toggleable && p.passthrough && !p.swipeable);
        assert_eq!(p.button_color, 0xCC00_0000);
        assert_eq!(p.stroke_color, 0xFFFF_FFFF);
        assert!(p.visible_in_game && !p.visible_in_menu);
    }

    #[test]
    fn launcher_specials_become_mouse_actions_or_are_skipped() {
        let json = layout(&[
            button("PRI", "0", "0", "[-3,-1,-1,-1]"),
            button("SEC", "0", "0", "[-4,-1,-1,-1]"),
            button("Keyboard", "0", "0", "[-1,-1,-1,-1]"),
            button("GUI", "0", "0", "[-2,-1,-1,-1]"),
            button("empty", "0", "0", "[-1,-1,-1,-1]"),
        ]);
        let out = convert(&json).unwrap();
        let keys: Vec<_> = out.settings.custom_buttons.iter().map(|b| &b.key).collect();
        assert_eq!(keys, ["key.mouse.left", "key.mouse.right"]);
        assert_eq!(out.skipped, 3);
    }

    #[test]
    fn a_fourth_key_is_dropped_and_counted() {
        let json = layout(&[button("combo", "0", "0", "[341,87,65,68]")]);
        let out = convert(&json).unwrap();
        let b = &out.settings.custom_buttons[0];
        assert_eq!(
            (&b.key[..], &b.second_key[..], &b.third_key[..]),
            (
                "key.keyboard.left.control",
                "key.keyboard.w",
                "key.keyboard.a"
            )
        );
        assert_eq!(out.dropped_events, 1);
    }

    #[test]
    fn positions_that_cannot_be_evaluated_skip_the_button() {
        let json = layout(&[
            button("bad", "${nope} + 1", "0", "[32]"),
            button("good", "100 + 2 * (3 - 1)", "dp(275)", "[32]"),
        ]);
        let out = convert(&json).unwrap();
        assert_eq!((out.buttons, out.skipped), (1, 1));
        // x = 100 + 2 * (3 - 1) = 104 px; y = dp(275 px) = 100.
        let p = out.settings.custom_buttons[0].layouts[0];
        assert!((p.center[0] - (104.0 + 68.75) / 2400.0).abs() < 1e-4);
        assert!((p.center[1] - (100.0 + 68.75) / 1080.0).abs() < 1e-4);
    }

    #[test]
    fn only_pojav_layouts_are_recognised() {
        assert!(looks_like_pojav(&layout(&[])));
        assert!(!looks_like_pojav(br#"{"layouts":[]}"#));
        assert!(!looks_like_pojav(b"not json"));
        assert!(convert(br#"{"a":1}"#).is_err());
    }

    #[test]
    fn the_joystick_moves_the_movement_control() {
        let stick = button("joystick", "100", "${bottom} - 50", "[-1,-1,-1,-1]");
        let json = format!(
            r#"{{"scaledAt":100,"mControlDataList":[],"mJoystickDataList":[{},"absolute":true}}]}}"#,
            &stick[..stick.len() - 1]
        );
        let out = convert(json.as_bytes()).unwrap();
        let p = out.settings.layouts[0][Control::Move as usize];
        assert!((p.center[0] - (100.0 + 68.75) / 2400.0).abs() < 1e-4);
        assert!(out.settings.dynamic_joystick);
        assert!(out.settings.visible(Control::Move));
    }

    #[test]
    fn without_a_joystick_the_default_one_is_hidden() {
        let out = convert(&layout(&[button("JUMP", "0", "0", "[32]")])).unwrap();
        assert!(!out.settings.visible(Control::Move));
        // A joystick whose position cannot be evaluated is no joystick either.
        let json = br#"{"mControlDataList":[],"mJoystickDataList":[{"dynamicX":"${nope}","dynamicY":"0"}]}"#;
        assert!(!convert(json).unwrap().settings.visible(Control::Move));
    }

    #[test]
    fn an_imported_preset_is_listed_without_becoming_active() {
        let home = std::env::temp_dir().join(format!(
            "pojav-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let out = convert(&layout(&[button("JUMP", "0", "0", "[32]")])).unwrap();
        let meta = Settings::import_preset(out.settings, &home, "My Zalith", "Imported").unwrap();
        let listed = Settings::list_presets(&home, ControlStyle::GoyControl);
        assert!(
            listed
                .iter()
                .any(|p| p.id == meta.id && p.name == "My Zalith")
        );
        assert!(!home.join("horizon-touch.json").exists());
        let loaded = Settings::load_preset(&home, ControlStyle::GoyControl, &meta.id);
        assert_eq!(loaded.custom_buttons[0].key, "key.keyboard.space");
        let _ = std::fs::remove_dir_all(&home);
    }
}
