//! Per-user client preferences, deliberately separate from game installs/maps.
//! Passwords, session tokens, chat and server-provided data are never serialized.
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
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema: 1,
            name: "Skier".into(),
            directory: "https://dir.peakrunner.net/servers".into(),
            direct: "quic://play.peakrunner.net:7777".into(),
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
    Ok(Preferences::default().edited(&raw.name, &raw.directory, &raw.direct))
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
        assert_eq!(value.as_object().unwrap().len(), 4);
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
