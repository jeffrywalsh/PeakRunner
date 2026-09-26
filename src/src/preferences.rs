//! Per-user client preferences, deliberately separate from game installs/maps.
//! Passwords, session tokens, chat and server-provided data are never serialized.
use peakrunner_core::bot_nav::Difficulty;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub schema: u32,
    pub name: String,
    pub directory: String,
    pub direct: String,
    /// 4x multisample anti-aliasing when the GPU supports it.
    pub antialiasing: bool,
    /// Offline bot difficulty. An unknown value reads as the default.
    #[serde(deserialize_with = "lenient_difficulty")]
    pub bot_difficulty: Difficulty,
    /// Glow around lights, flames and the sun. An unknown value reads as the default.
    #[serde(deserialize_with = "lenient_bloom")]
    pub bloom: Bloom,
    /// Keyboard bindings. Unknown or bad entries read as the defaults.
    pub keys: crate::keybinds::Keybinds,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Bloom {
    Off,
    #[default]
    Low,
    High,
}

impl Bloom {
    /// `scene::BLOOM_LEVEL` value.
    pub fn level(self) -> u8 {
        match self {
            Bloom::Off => 0,
            Bloom::Low => 1,
            Bloom::High => 2,
        }
    }
}

fn lenient_difficulty<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Difficulty, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

fn lenient_bloom<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Bloom, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema: 1,
            name: "Skier".into(),
            directory: "https://dir.peakrunner.net/servers".into(),
            direct: "quic://play.peakrunner.net:7777".into(),
            antialiasing: true,
            bot_difficulty: Difficulty::default(),
            bloom: Bloom::default(),
            keys: Default::default(),
        }
    }
}

fn address(raw: &str) -> Option<String> {
    let value = raw.trim();
    // No credentials or URL query secrets in a preferences file. Network code
    // still validates scheme/host/transport independently before connecting.
    (!value.is_empty()
        && raw.len() <= 2048
        && !raw.chars().any(char::is_control)
        && !value.contains(['@', '?', '#']))
    .then(|| value.to_owned())
}

impl Preferences {
    pub fn edited(&self, name: &str, directory: &str, direct: &str) -> Self {
        Self {
            schema: 1,
            name: peakrunner_core::names::validate(name)
                .map(str::to_owned)
                .unwrap_or_else(|| self.name.clone()),
            directory: address(directory).unwrap_or_else(|| self.directory.clone()),
            direct: address(direct).unwrap_or_else(|| self.direct.clone()),
            antialiasing: self.antialiasing,
            bot_difficulty: self.bot_difficulty,
            bloom: self.bloom,
            keys: self.keys.clone(),
        }
    }
}

fn path_for(
    os: &str,
    home: Option<PathBuf>,
    roaming: Option<PathBuf>,
    xdg: Option<PathBuf>,
) -> Option<PathBuf> {
    let absolute = |p: Option<PathBuf>| p.filter(|p| p.is_absolute());
    let home = absolute(home);
    let base = match os {
        "macos" => home?.join("Library/Application Support"),
        "windows" => absolute(roaming).or_else(|| home.map(|p| p.join("AppData/Roaming")))?,
        _ => absolute(xdg).or_else(|| home.map(|p| p.join(".config")))?,
    };
    Some(base.join("PeakRunner").join("client.json"))
}

pub(crate) struct Store {
    pub path: Option<PathBuf>,
    pub saved: Preferences,
    pub warning: Option<String>,
}

impl Store {
    pub fn memory() -> Self {
        Self {
            path: None,
            saved: Preferences::default(),
            warning: None,
        }
    }

    pub fn user() -> Self {
        // Automated direct-join sessions must not overwrite the user's settings.
        if std::env::var_os("PEAKRUNNER_JOIN").is_some() {
            return Self::memory();
        }
        let path = if let Some(dir) = std::env::var_os("PEAKRUNNER_CONFIG_DIR") {
            let dir = PathBuf::from(dir);
            dir.is_absolute().then(|| dir.join("client.json"))
        } else {
            path_for(
                std::env::consts::OS,
                std::env::home_dir(),
                std::env::var_os("APPDATA").map(PathBuf::from),
                std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            )
        };
        match path {
            Some(path) => Self::load(path),
            None => Self {
                warning: Some(
                    "No absolute user configuration folder; settings are temporary.".into(),
                ),
                ..Self::memory()
            },
        }
    }

    pub fn load(path: PathBuf) -> Self {
        let mut store = Self {
            path: Some(path.clone()),
            ..Self::memory()
        };
        match read(&path) {
            Ok(p) => store.saved = p,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => {
                store.warning = Some(
                    "Couldn't read saved settings; using defaults. The original file is unchanged."
                        .into(),
                )
            }
        }
        store
    }

    pub fn save(&mut self, name: &str, directory: &str, direct: &str) {
        let next = self.saved.edited(name, directory, direct);
        self.commit(next);
    }

    pub fn set_antialiasing(&mut self, on: bool) {
        let next = Preferences { antialiasing: on, ..self.saved.clone() };
        self.commit(next);
    }

    pub fn set_bot_difficulty(&mut self, difficulty: Difficulty) {
        let next = Preferences { bot_difficulty: difficulty, ..self.saved.clone() };
        self.commit(next);
    }

    pub fn set_bloom(&mut self, bloom: Bloom) {
        let next = Preferences { bloom, ..self.saved.clone() };
        self.commit(next);
    }

    pub fn set_keys(&mut self, keys: crate::keybinds::Keybinds) {
        let next = Preferences { keys, ..self.saved.clone() };
        self.commit(next);
    }

    fn commit(&mut self, next: Preferences) {
        if next == self.saved {
            return;
        }
        let Some(path) = &self.path else {
            return;
        };
        match write(path, &next) {
            Ok(()) => {
                self.saved = next;
                self.warning = None;
            }
            Err(_) => {
                self.warning =
                    Some("Couldn't save settings. Changes will last only for this session.".into())
            }
        }
    }
}

fn read(path: &Path) -> io::Result<Preferences> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(16 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 16 * 1024 {
        return Err(io::Error::other("oversized preferences"));
    }
    let raw: Preferences = serde_json::from_slice(&data).map_err(io::Error::other)?;
    if raw.schema != 1 {
        return Err(io::Error::other("unsupported preferences schema"));
    }
    let mut prefs = Preferences::default().edited(&raw.name, &raw.directory, &raw.direct);
    prefs.antialiasing = raw.antialiasing;
    prefs.bot_difficulty = raw.bot_difficulty;
    prefs.bloom = raw.bloom;
    prefs.keys = raw.keys;
    Ok(prefs)
}

fn write(path: &Path, prefs: &Preferences) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    fs::create_dir_all(parent)?;
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let temp = parent.join(format!(
        ".client-{}-{}.tmp",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    let result = (|| {
        file.write_all(&serde_json::to_vec_pretty(prefs).map_err(io::Error::other)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        // Same-directory rename replaces atomically on supported native systems;
        // never delete the previous preferences file first.
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "peakrunner-prefs-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    #[test]
    fn paths_follow_native_conventions_and_ignore_relative_xdg() {
        let root = std::env::temp_dir().join("prefs-path-test");
        let home = Some(root.join("home"));
        assert_eq!(
            path_for("macos", home.clone(), None, None).unwrap(),
            root.join("home/Library/Application Support/PeakRunner/client.json")
        );
        assert_eq!(
            path_for("linux", home.clone(), None, Some("relative".into())).unwrap(),
            root.join("home/.config/PeakRunner/client.json")
        );
        assert_eq!(
            path_for("linux", home.clone(), None, Some(root.join("custom"))).unwrap(),
            root.join("custom/PeakRunner/client.json")
        );
        assert_eq!(
            path_for("windows", home, Some(root.join("roaming")), None).unwrap(),
            root.join("roaming/PeakRunner/client.json")
        );
        assert!(path_for("linux", None, None, None).is_none());
    }
    #[test]
    fn round_trip_replacement_and_password_exclusion() {
        let dir = temp();
        let path = dir.join("client.json");
        let mut s = Store::load(path.clone());
        s.save(
            "  Pilot 2  ",
            "https://example.net/servers",
            "quic://example.net:7777",
        );
        assert_eq!(Store::load(path.clone()).saved.name, "Pilot 2");
        s.save(
            "Pilot 3",
            &s.saved.directory.clone(),
            &s.saved.direct.clone(),
        );
        assert_eq!(Store::load(path.clone()).saved.name, "Pilot 3");
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        // schema, name, directory, direct, antialiasing, bot_difficulty, bloom, keys: nothing else.
        assert_eq!(value.as_object().unwrap().len(), 8);
        assert!(value.get("password").is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn antialiasing_defaults_on_persists_and_survives_name_edits() {
        let dir = temp();
        let path = dir.join("client.json");
        // A file written before the setting existed keeps anti-aliasing on.
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, br#"{"schema":1,"name":"Pilot","directory":"https://example.net/s","direct":"quic://example.net:7777"}"#).unwrap();
        let mut s = Store::load(path.clone());
        assert!(s.saved.antialiasing);
        s.set_antialiasing(false);
        assert!(!Store::load(path.clone()).saved.antialiasing);
        s.save("Pilot 9", &s.saved.directory.clone(), &s.saved.direct.clone());
        let loaded = Store::load(path.clone()).saved;
        assert_eq!(loaded.name, "Pilot 9");
        assert!(!loaded.antialiasing, "editing the name must not reset anti-aliasing");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bloom_defaults_low_persists_and_tolerates_unknown_values() {
        let dir = temp();
        let path = dir.join("client.json");
        fs::create_dir_all(&dir).unwrap();
        // A file written before the setting existed gets the default.
        fs::write(&path, br#"{"schema":1,"name":"Pilot","directory":"https://example.net/s","direct":"quic://example.net:7777"}"#).unwrap();
        let mut s = Store::load(path.clone());
        assert_eq!(s.saved.bloom, Bloom::Low);
        s.set_bloom(Bloom::High);
        assert_eq!(Store::load(path.clone()).saved.bloom, Bloom::High);
        s.save("Pilot 5", &s.saved.directory.clone(), &s.saved.direct.clone());
        assert_eq!(Store::load(path.clone()).saved.bloom, Bloom::High, "a name edit keeps bloom");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains(r#""bloom": "high""#) || raw.contains(r#""bloom":"high""#));
        fs::write(&path, raw.replace("high", "blinding")).unwrap();
        assert_eq!(Store::load(path.clone()).saved.bloom, Bloom::Low);
        assert_eq!([Bloom::Off.level(), Bloom::Low.level(), Bloom::High.level()], [0, 1, 2]);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bot_difficulty_defaults_to_normal_persists_and_tolerates_unknown_values() {
        let dir = temp();
        let path = dir.join("client.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, br#"{"schema":1,"name":"Pilot","directory":"https://example.net/s","direct":"quic://example.net:7777"}"#).unwrap();
        let mut s = Store::load(path.clone());
        assert_eq!(s.saved.bot_difficulty, Difficulty::Normal);
        s.set_bot_difficulty(Difficulty::Hard);
        assert_eq!(Store::load(path.clone()).saved.bot_difficulty, Difficulty::Hard);
        s.save("Pilot 4", &s.saved.directory.clone(), &s.saved.direct.clone());
        assert_eq!(Store::load(path.clone()).saved.bot_difficulty, Difficulty::Hard, "editing the name keeps it");
        // A value from a newer build must not throw away the name.
        fs::write(&path, br#"{"schema":1,"name":"Pilot 5","directory":"https://example.net/s","direct":"quic://example.net:7777","bot_difficulty":"Brutal"}"#).unwrap();
        let loaded = Store::load(path.clone()).saved;
        assert_eq!((loaded.name.as_str(), loaded.bot_difficulty), ("Pilot 5", Difficulty::Normal));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_or_secret_fields_keep_last_valid_values() {
        let old = Preferences::default();
        assert_eq!(
            old.edited(
                "<script>",
                "https://name:secret@example.net",
                "quic://host?token=secret"
            ),
            old
        );
        assert_eq!(old.edited("\n", &"x".repeat(2049), "host\n"), old);
    }
    #[test]
    fn a_stored_map_key_for_a_removed_map_is_ignored() {
        // Preferences never persist a map; the client starts on its default map.
        // A file edited by hand or by a future build must not break loading.
        let dir = temp();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("client.json");
        fs::write(&path, br#"{"schema":1,"name":"Pilot","map":"skybreak-bastions"}"#).unwrap();
        let s = Store::load(path);
        assert!(s.warning.is_none());
        assert_eq!(s.saved.name, "Pilot");
        assert_eq!(peakrunner_core::terrain::MapId::parse("skybreak-bastions"), None);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupt_oversized_future_and_unwritable_files_are_nonfatal() {
        let dir = temp();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("client.json");
        for bytes in [
            b"not json".to_vec(),
            vec![b'x'; 16385],
            br#"{"schema":2}"#.to_vec(),
        ] {
            fs::write(&path, &bytes).unwrap();
            let s = Store::load(path.clone());
            assert!(s.warning.is_some());
            assert_eq!(s.saved, Preferences::default());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        let mut s = Store::load(path.join("cannot-exist.json"));
        s.save(
            "Pilot",
            &Preferences::default().directory,
            &Preferences::default().direct,
        );
        assert!(s.warning.is_some());
        assert_eq!(s.saved.name, "Skier");
        fs::remove_dir_all(dir).unwrap();
    }
}
