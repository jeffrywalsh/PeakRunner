//! Rebindable keyboard controls. The mouse (fire, jet) and the arrow keys
//! (extra movement) stay fixed; Escape always opens the menu. Bindings save
//! with the other client preferences as action → egui key name, and an
//! unknown action or key name reads as that action's default.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Action {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Use,
    Repair,
    Deploy,
    View,
    Grenade,
    Mine,
    Disc,
    Chaingun,
    Launcher,
    Rifle,
    ChatPublic,
    ChatTeam,
    Suicide,
}

impl Action {
    pub const ALL: [Action; 18] = [
        Action::Forward, Action::Back, Action::Left, Action::Right, Action::Jump,
        Action::Use, Action::Repair, Action::Deploy, Action::View, Action::Grenade, Action::Mine,
        Action::Disc, Action::Chaingun, Action::Launcher, Action::Rifle,
        Action::ChatPublic, Action::ChatTeam, Action::Suicide,
    ];

    /// Stable name in the preferences file.
    pub fn key(self) -> &'static str {
        match self {
            Action::Forward => "forward",
            Action::Back => "back",
            Action::Left => "left",
            Action::Right => "right",
            Action::Jump => "jump",
            Action::Use => "use",
            Action::Repair => "repair",
            Action::Deploy => "deploy",
            Action::View => "view",
            Action::Grenade => "grenade",
            Action::Mine => "mine",
            Action::Disc => "weapon1",
            Action::Chaingun => "weapon2",
            Action::Launcher => "weapon3",
            Action::Rifle => "weapon4",
            Action::ChatPublic => "chat",
            Action::ChatTeam => "team_chat",
            Action::Suicide => "suicide",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Forward => "Move forward",
            Action::Back => "Move back",
            Action::Left => "Strafe left",
            Action::Right => "Strafe right",
            Action::Jump => "Jump / ski (hold)",
            Action::Use => "Use station · zoom a rifle (hold)",
            Action::Repair => "Repair tool (hold)",
            Action::Deploy => "Deploy pack (then click to place)",
            Action::View => "Third-person view · turn a pack being placed",
            Action::Grenade => "Throw grenade (hold to wind up)",
            Action::Mine => "Throw mine (hold to wind up)",
            Action::Disc => "Disc launcher",
            Action::Chaingun => "Chaingun",
            Action::Launcher => "Grenade launcher / mortar",
            Action::Rifle => "Rifle (laser / railgun, if bought)",
            Action::ChatPublic => "Chat",
            Action::ChatTeam => "Team chat",
            Action::Suicide => "Respawn (with Ctrl)",
        }
    }

    pub fn default_key(self) -> egui::Key {
        use egui::Key;
        match self {
            Action::Forward => Key::W,
            Action::Back => Key::S,
            Action::Left => Key::A,
            Action::Right => Key::D,
            Action::Jump => Key::Space,
            Action::Use => Key::E,
            Action::Repair => Key::Q,
            Action::Deploy => Key::B,
            Action::View => Key::R,
            Action::Grenade => Key::G,
            Action::Mine => Key::M,
            Action::Disc => Key::Num1,
            Action::Chaingun => Key::Num2,
            Action::Launcher => Key::Num3,
            Action::Rifle => Key::Num4,
            Action::ChatPublic => Key::T,
            Action::ChatTeam => Key::Y,
            Action::Suicide => Key::K,
        }
    }
}

/// Keys no action may take: Escape opens the menu, Tab the scoreboard, Enter
/// sends chat.
pub(crate) fn reserved(key: egui::Key) -> bool {
    matches!(key, egui::Key::Escape | egui::Key::Tab | egui::Key::Enter)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Keybinds {
    keys: BTreeMap<Action, egui::Key>,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self { keys: Action::ALL.iter().map(|&a| (a, a.default_key())).collect() }
    }
}

impl Keybinds {
    pub fn get(&self, action: Action) -> egui::Key {
        self.keys.get(&action).copied().unwrap_or(action.default_key())
    }

    /// The key's display name, for hints ("G", "Space").
    pub fn name(&self, action: Action) -> &'static str {
        self.get(action).symbol_or_name()
    }

    /// Bind `key` to `action`. An action already on that key swaps onto the
    /// action's old key, so no two actions share one. Reserved keys are refused.
    pub fn bind(&mut self, action: Action, key: egui::Key) -> bool {
        if reserved(key) { return false; }
        let old = self.get(action);
        if let Some((&other, _)) = self.keys.iter().find(|(&a, &k)| k == key && a != action) {
            self.keys.insert(other, old);
        }
        self.keys.insert(action, key);
        true
    }

    pub fn is_default(&self) -> bool { *self == Self::default() }

    pub fn down(&self, i: &egui::InputState, action: Action) -> bool { i.key_down(self.get(action)) }
    pub fn pressed(&self, i: &egui::InputState, action: Action) -> bool { i.key_pressed(self.get(action)) }
}

impl Serialize for Keybinds {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let map: BTreeMap<&str, &str> = self.keys.iter().map(|(a, k)| (a.key(), k.name())).collect();
        map.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Keybinds {
    /// Lenient: unknown actions are ignored, and bad or reserved keys keep
    /// that action's default.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        let mut binds = Self::default();
        if let Some(map) = value.as_object() {
            for action in Action::ALL {
                let Some(key) = map.get(action.key()).and_then(|v| v.as_str()).and_then(egui::Key::from_name) else { continue };
                // Refuses reserved keys; a repeated key swaps, so none is shared.
                binds.bind(action, key);
            }
        }
        Ok(binds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_unique_and_rebinding_swaps() {
        let mut b = Keybinds::default();
        let mut seen: Vec<egui::Key> = Action::ALL.iter().map(|&a| b.get(a)).collect();
        seen.sort_by_key(|k| k.name());
        seen.dedup();
        assert_eq!(seen.len(), Action::ALL.len());
        assert_eq!(b.get(Action::Deploy), egui::Key::B);
        assert!(b.bind(Action::Grenade, egui::Key::M));
        assert_eq!((b.get(Action::Grenade), b.get(Action::Mine)), (egui::Key::M, egui::Key::G), "swapped");
        assert!(!b.bind(Action::Jump, egui::Key::Escape));
    }

    #[test]
    fn saved_bindings_round_trip_and_bad_ones_fall_back() {
        let mut b = Keybinds::default();
        b.bind(Action::Deploy, egui::Key::F);
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(serde_json::from_str::<Keybinds>(&json).unwrap(), b);
        let odd: Keybinds = serde_json::from_str(r#"{"jump":"Escape","deploy":"NotAKey","mine":"F","grenade":"F","dance":"X"}"#).unwrap();
        assert_eq!(odd.get(Action::Jump), egui::Key::Space);
        assert_eq!(odd.get(Action::Deploy), egui::Key::B);
        let keys: Vec<egui::Key> = Action::ALL.iter().map(|&a| odd.get(a)).collect();
        for (n, k) in keys.iter().enumerate() { assert!(!keys[n + 1..].contains(k), "{k:?} twice"); }
    }
}
