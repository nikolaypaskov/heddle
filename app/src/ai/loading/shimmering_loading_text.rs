//! Shimmering loading text for AI loading states.
//!
//! This used to prepend Warp's logomark, a private-use glyph (U+E500, `warpLogo`) that upstream
//! patched into all six bundled Roboto faces. That glyph was subsetted out of the fonts when the
//! fork removed Warp's branded binary assets, but these renderers kept asking for it -- so the
//! codepoint resolved to no glyph in the UI font and the indicator drew a missing-glyph box before
//! every "Thinking..." label. The glyph is now gone from the text as well as the font.

use warp_core::ui::appearance::Appearance;
use warpui::elements::Element;
use warpui::elements::shimmering_text::{
    ShimmerConfig, ShimmeringTextElement, ShimmeringTextStateHandle,
};
use warpui::{AppContext, SingletonEntity};

/// Creates a shimmering text element for a loading label.
pub fn shimmering_loading_text(
    text: impl Into<String>,
    font_size: f32,
    shimmer_handle: ShimmeringTextStateHandle,
    app: &AppContext,
) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();

    // Use same colors as common.rs for consistency
    let base_color = theme.disabled_text_color(theme.surface_1()).into_solid();
    let shimmer_color = theme.main_text_color(theme.surface_1()).into_solid();

    // Hardcoded shimmer config for consistent animation
    let config = ShimmerConfig::default();

    ShimmeringTextElement::new(
        text.into(),
        appearance.ui_font_family(),
        font_size,
        base_color,
        shimmer_color,
        config,
        shimmer_handle,
    )
    .finish()
}
