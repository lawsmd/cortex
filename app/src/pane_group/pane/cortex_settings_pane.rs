//! `CortexSettingsPane` — the [`PaneContent`] hosting the Cortex Settings UI.
//!
//! Lives in `pane_group/pane/` (rather than `cortex_settings/`) so it can use
//! the `super::` imports for [`PaneView`], [`PaneGroup`], and other types that
//! are `pub(super)` within the pane-group module hierarchy. The user-facing
//! parts of Cortex Settings (view, appearance page, action enum, manager)
//! still live under `app/src/cortex_settings/`; this file is just the pane
//! adapter.
//!
//! Mirrors `app/src/pane_group/pane/network_log_pane.rs`.
use warpui::{AppContext, ModelHandle, SingletonEntity, View, ViewContext, ViewHandle};

use crate::app_state::LeafContents;
use crate::cortex_settings::{CortexSettingsPaneManager, CortexSettingsView, CortexSettingsViewEvent};
use crate::workspace::PaneViewLocator;

use super::{
    view::PaneView, DetachType, PaneConfiguration, PaneContent, PaneGroup, PaneId, ShareableLink,
    ShareableLinkError,
};

pub struct CortexSettingsPane {
    view: ViewHandle<PaneView<CortexSettingsView>>,
    pane_configuration: ModelHandle<PaneConfiguration>,
}

impl CortexSettingsPane {
    pub fn from_view(
        cortex_settings_view: ViewHandle<CortexSettingsView>,
        ctx: &mut AppContext,
    ) -> Self {
        let pane_configuration = cortex_settings_view.as_ref(ctx).pane_configuration();

        let view = ctx.add_typed_action_view(cortex_settings_view.window_id(ctx), |ctx| {
            let pane_id = PaneId::from_cortex_settings_pane_ctx(ctx);
            PaneView::new(
                pane_id,
                cortex_settings_view,
                (),
                pane_configuration.clone(),
                ctx,
            )
        });

        Self {
            view,
            pane_configuration,
        }
    }

    pub fn new<V: View>(ctx: &mut ViewContext<V>) -> Self {
        let view = ctx.add_typed_action_view(CortexSettingsView::new);
        Self::from_view(view, ctx)
    }

    pub fn cortex_settings_view(&self, ctx: &AppContext) -> ViewHandle<CortexSettingsView> {
        self.view.as_ref(ctx).child(ctx)
    }
}

impl PaneContent for CortexSettingsPane {
    fn id(&self) -> PaneId {
        PaneId::from_cortex_settings_pane_view(&self.view)
    }

    fn attach(
        &self,
        _group: &PaneGroup,
        focus_handle: crate::pane_group::focus_state::PaneFocusHandle,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        self.view
            .update(ctx, |view, ctx| view.set_focus_handle(focus_handle, ctx));

        let cortex_settings_view = self.cortex_settings_view(ctx);
        let pane_id = self.id();
        let pane_group_id = ctx.view_id();
        let window_id = ctx.window_id();

        ctx.subscribe_to_view(&cortex_settings_view, move |pane_group, _, event, ctx| {
            let CortexSettingsViewEvent::Pane(pane_event) = event;
            pane_group.handle_pane_event(pane_id, pane_event, ctx)
        });
        ctx.subscribe_to_view(&self.view, move |group, _, event, ctx| {
            group.handle_pane_view_event(pane_id, event, ctx);
        });

        CortexSettingsPaneManager::handle(ctx).update(ctx, |manager, _ctx| {
            manager.register_pane(
                window_id,
                PaneViewLocator {
                    pane_group_id,
                    pane_id,
                },
            );
        });
    }

    fn detach(
        &self,
        _group: &PaneGroup,
        _detach_type: DetachType,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        let cortex_settings_view = self.cortex_settings_view(ctx);
        ctx.unsubscribe_to_view(&cortex_settings_view);
        ctx.unsubscribe_to_view(&self.view);

        let window_id = ctx.window_id();
        CortexSettingsPaneManager::handle(ctx).update(ctx, |manager, _| {
            manager.deregister_pane(&window_id);
        });
    }

    fn snapshot(&self, _app: &AppContext) -> LeafContents {
        // Cortex Settings panes are intentionally not restored across launches:
        // the pane has no transient state worth preserving (selected section
        // resets to default; setting values are persisted by the settings
        // system independently of pane lifetime). See
        // `LeafContents::is_persisted` in `app/src/app_state.rs`.
        LeafContents::CortexSettings
    }

    fn has_application_focus(&self, ctx: &mut ViewContext<PaneGroup>) -> bool {
        self.view.is_self_or_child_focused(ctx)
    }

    fn focus(&self, ctx: &mut ViewContext<PaneGroup>) {
        self.cortex_settings_view(ctx)
            .update(ctx, |view, ctx| view.focus(ctx));
    }

    fn shareable_link(
        &self,
        _ctx: &mut ViewContext<PaneGroup>,
    ) -> Result<ShareableLink, ShareableLinkError> {
        Ok(ShareableLink::Base)
    }

    fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    fn is_pane_being_dragged(&self, ctx: &AppContext) -> bool {
        self.view.as_ref(ctx).is_being_dragged()
    }
}
