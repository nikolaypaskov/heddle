//! UI state formerly used by the ambient agent progress/loading screen.
//!
//! The cloud-agent loading/error screen rendering has been removed in Heddle; this
//! state is retained only because it hangs off the `AmbientAgentViewModel` and is
//! deleted together with that model in a later slice of the ambient removal.

use warpui::ModelHandle;
use warpui::elements::shimmering_text::ShimmeringTextStateHandle;
use warpui::elements::{MouseStateHandle, SelectionHandle};

use crate::ai::agent_tips::AITipModel;
use crate::terminal::view::ambient_agent::CloudModeTip;
use crate::terminal::view::ambient_agent::model::AmbientAgentViewModel;

/// UI state that backed the (now-removed) ambient agent progress screen. Retained
/// on the `AmbientAgentViewModel` until that model is removed.
pub struct AmbientAgentProgressUIState {
    /// Shimmering-text animation handle (formerly the loading screen).
    pub loading_shimmer_handle: ShimmeringTextStateHandle,

    /// Tip model with a 60s cooldown (formerly the loading screen).
    pub tip_model: ModelHandle<AITipModel<CloudModeTip>>,

    /// Selection handle for error text (formerly the error screen).
    pub error_selection_handle: SelectionHandle,

    /// Selected error text for copying (formerly the error screen).
    pub error_selected_text: std::rc::Rc<parking_lot::RwLock<Option<String>>>,

    /// Authenticate-button mouse state (formerly the GitHub auth screen).
    pub auth_button_mouse_state: MouseStateHandle,
}

impl AmbientAgentProgressUIState {
    /// Creates a new ambient agent progress UI state with initialized handles.
    pub fn new(ctx: &mut warpui::ModelContext<AmbientAgentViewModel>) -> Self {
        let tip_model = ctx.add_model(|_ctx| {
            use crate::terminal::view::ambient_agent;
            AITipModel::new(ambient_agent::get_cloud_mode_tips())
        });

        Self {
            loading_shimmer_handle: ShimmeringTextStateHandle::new(),
            tip_model,
            error_selection_handle: SelectionHandle::default(),
            error_selected_text: std::rc::Rc::new(parking_lot::RwLock::new(None)),
            auth_button_mouse_state: MouseStateHandle::default(),
        }
    }
}
