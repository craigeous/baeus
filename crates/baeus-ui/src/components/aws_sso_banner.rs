//! AWS SSO expired-session banner.
//!
//! Renders a dismissible warning bar above the content area when an AWS SSO
//! token expiry is detected during cluster connection.

use gpui::prelude::*;
use gpui::{ElementId, FontWeight, Rgba, SharedString, StatefulInteractiveElement, div};

use crate::layout::app_shell::AppShell;

impl AppShell {
    /// Render an SSO login banner if `pending_sso_login` is set.
    ///
    /// Returns `Option<Div>` so the caller can use `.children()` to conditionally
    /// include it in the layout.
    pub(crate) fn render_sso_login_banner(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let pending = self.pending_sso_login.as_ref()?;
        let profile = pending.profile.clone();
        let warning_bg = Rgba { r: 0.95, g: 0.85, b: 0.35, a: 1.0 };
        let text_dark = Rgba { r: 0.15, g: 0.15, b: 0.15, a: 1.0 };

        let msg =
            SharedString::from(format!("AWS SSO session expired for profile \"{}\"", profile,));

        Some(
            div()
                .id(ElementId::Name(SharedString::from("aws-sso-banner")))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_4()
                .py_2()
                .bg(warning_bg)
                .text_color(text_dark)
                .text_sm()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(msg)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .id(ElementId::Name(SharedString::from("sso-banner-auth-btn")))
                                .cursor_pointer()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .bg(Rgba { r: 0.2, g: 0.2, b: 0.2, a: 1.0 })
                                .text_color(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 })
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .hover(|el: gpui::StyleRefinement| el.opacity(0.85))
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.run_aws_sso_login(cx);
                                    this.pending_sso_login = None;
                                    cx.notify();
                                }))
                                .child("Authenticate"),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(SharedString::from("sso-banner-dismiss-btn")))
                                .cursor_pointer()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .border_1()
                                .border_color(text_dark)
                                .text_sm()
                                .hover(|el: gpui::StyleRefinement| el.opacity(0.7))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.pending_sso_login = None;
                                    cx.notify();
                                }))
                                .child("Dismiss"),
                        ),
                ),
        )
    }
}
