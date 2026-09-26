//! Discovery contracts and bounded networking helpers. No gameplay dependency.
pub mod lan;
pub mod http;
pub mod quic;
pub mod wire;
mod types;
pub use types::*;

/// Map labels in adverts are untrusted: accept map keys (`broadside-clone`) and
/// display names (`Old Holler`), both today's and older releases', without a
/// per-map list. ASCII letters, digits, single inner spaces and hyphens only.
pub fn valid_map_label(label: &str) -> bool {
    (1..=32).contains(&label.len())
        && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-')
        && label.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        && label.bytes().last().is_some_and(|b| b.is_ascii_alphanumeric())
        && !label.contains("  ")
}

#[cfg(test)]
mod tests {
    use super::valid_map_label;
    #[test]
    fn map_labels_accept_keys_and_names_and_reject_hostile_input() {
        for ok in ["raindance", "broadside-clone", "stonehenge-clone", "snowblind-clone",
            "desert-of-death-clone", "skybreak-bastions", "valley", "Valley", "Raindance",
            "Old Holler", "Tower Complex", "Cairnhold", "Frostline", "Dustreach",
            "Skybreak Bastions", "Broadside Clone", "Desert of Death Clone",
            // Current servers add the mode.
            "Old Holler - CTF", "Tower Complex - CnH", "Longfield - Football"] {
            assert!(valid_map_label(ok), "{ok}");
        }
        for bad in ["", " ", " Raindance", "Raindance ", "Old  Holler", "<b>Valley</b>",
            "a&b", "tab\there", "line\nbreak", "bidi\u{202e}map", "zero\u{200b}width",
            "Holler\u{0}", "caf\u{e9}", &"x".repeat(33)] {
            assert!(!valid_map_label(bad), "{bad:?}");
        }
    }
}
