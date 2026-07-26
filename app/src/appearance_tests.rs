//! Invariants about the fonts this app bundles and renders.
//!
//! These exist because of a defect that shipped: upstream patched their logomark into all six
//! bundled Roboto faces as a private-use glyph (U+E500, `warpLogo`). The fork subsetted that glyph
//! out of the fonts, correctly, since it was Warp's trademark. But three renderers kept prepending
//! U+E500 to AI loading labels, so the codepoint resolved to nothing in the UI font and the
//! indicator drew a missing-glyph box before every "Thinking..." line.
//!
//! The asset scanner did not catch it. It verifies the branded asset is *gone*, which it was;
//! nothing verified that the code which *drew* it had been updated too. Removing an asset and
//! leaving its consumer behind is silent by construction -- the glyph just stops appearing -- so
//! these two assertions close that gap from both ends.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use warpui::AssetProvider as _;

use crate::ASSETS;

/// The six faces registered as the "Roboto" family in [`super::register_bundled_fonts`].
const ROBOTO_FACES: [&str; 6] = [
    "bundled/fonts/roboto/Roboto-Regular.ttf",
    "bundled/fonts/roboto/Roboto-Italic.ttf",
    "bundled/fonts/roboto/Roboto-Bold.ttf",
    "bundled/fonts/roboto/Roboto-BoldItalic.ttf",
    "bundled/fonts/roboto/Roboto-Medium.ttf",
    "bundled/fonts/roboto/RobotoFlex-Semibold.ttf",
];

/// The Unicode Private Use Area. Codepoints here carry no standard meaning, so anything mapped in
/// this range is vendor-specific artwork rather than text -- which is exactly why a logomark was
/// hidden here rather than in a normal Unicode block.
const PRIVATE_USE_AREA: std::ops::RangeInclusive<u32> = 0xE000..=0xF8FF;

/// The private-use codepoints Google's own Roboto ships, recorded so that anything *else* appearing
/// in the range fails the test below.
///
/// Every entry here is named `uniXXXX` in the font -- an auto-generated name, i.e. Google shipped
/// the glyph without giving it a meaning. Warp's addition was distinguishable precisely because it
/// had a descriptive name (`warpLogo`) and sat outside this set. That is the whole discrimination
/// this list buys: it is a ratchet on the stock content, not an endorsement of it.
const STOCK_ROBOTO_PRIVATE_USE: &[u32] = &[
    // Roboto-{Regular,Italic,Bold,BoldItalic,Medium}
    0xEE01, 0xEE02, 0xF6C3,
    // RobotoFlex-Semibold
    0xF50E, 0xF50F, 0xF510, 0xF511, 0xF518, 0xF519, 0xF51A, 0xF51B, 0xF522, 0xF523, 0xF524, 0xF525,
    0xF528, 0xF529, 0xF52C, 0xF52D,
];

/// The bundled UI font must map no private-use glyph beyond what Google shipped.
///
/// Asserted over the *embedded bytes*, not the files on disk, so it reflects what a build actually
/// ships. A font update that reintroduced Warp's logomark -- or hid any other vendor's mark at a
/// codepoint with no standard meaning -- fails here rather than being left to be noticed by eye.
#[test]
fn the_bundled_ui_font_maps_no_unexpected_private_use_glyph() {
    let allowed: BTreeSet<u32> = STOCK_ROBOTO_PRIVATE_USE.iter().copied().collect();

    for face in ROBOTO_FACES {
        let bytes = ASSETS.get(face).unwrap_or_else(|e| {
            panic!("{face} is registered as a UI font face but is not embedded: {e}")
        });
        let font = ttf_parser::Face::parse(&bytes, 0).unwrap_or_else(|e| panic!("{face}: {e}"));

        let mut unexpected = BTreeSet::new();
        for subtable in font
            .tables()
            .cmap
            .unwrap_or_else(|| panic!("{face} has no cmap table"))
            .subtables
        {
            if !subtable.is_unicode() {
                continue;
            }
            subtable.codepoints(|cp| {
                if PRIVATE_USE_AREA.contains(&cp) && !allowed.contains(&cp) {
                    unexpected.insert(cp);
                }
            });
        }

        assert!(
            unexpected.is_empty(),
            "{face} maps {} private-use codepoint(s) that Google's Roboto does not ship: {}. \
             Codepoints in this range have no standard meaning, so a glyph here is vendor artwork -- \
             upstream hid their logomark at U+E500 exactly this way. If the glyph is genuinely \
             wanted, add it to STOCK_ROBOTO_PRIVATE_USE deliberately and say why.",
            unexpected.len(),
            unexpected
                .iter()
                .map(|cp| format!("U+{cp:04X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

/// And no source file may ask the UI font to draw the glyph that was removed.
///
/// This is the other half. The assertion above stops the artwork coming back; this one stops a
/// renderer referring to artwork that is not there. Scoped to the one codepoint and one glyph name
/// that were actually removed -- a blanket ban on private-use literals would fail honestly, because
/// `font_fallback.rs` enumerates Nerd Font ranges on purpose, for fonts the *user* supplies.
#[test]
fn no_source_file_renders_the_removed_logomark() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    // This file names both banned tokens in order to search for them, so it must exclude itself or
    // it reports itself as the offender. Taken from `file!()` rather than hardcoded, so renaming the
    // file cannot silently widen the exclusion to nothing.
    let this_file = Path::new(file!())
        .file_name()
        .expect("file!() always has a final component")
        .to_owned();

    let mut offenders = Vec::new();

    fn walk(dir: &Path, skip: &std::ffi::OsStr, offenders: &mut Vec<String>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                walk(&path, skip, offenders);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") || path.file_name() == Some(skip) {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            for (n, line) in text.lines().enumerate() {
                // Comments may discuss the removal; only rendered text is an offence.
                if line.trim_start().starts_with("//") {
                    continue;
                }
                if line.contains("\\u{E500}") || line.contains("warpLogo") {
                    offenders.push(format!("{}:{}", path.display(), n + 1));
                }
            }
        }
    }
    walk(&src, &this_file, &mut offenders);

    assert!(
        offenders.is_empty(),
        "U+E500 (`warpLogo`) was Warp's logomark and is subsetted out of every bundled font. \
         Rendering it now produces a missing-glyph box, which is how it went unnoticed the first \
         time. Referenced at: {}",
        offenders.join(", ")
    );
}
