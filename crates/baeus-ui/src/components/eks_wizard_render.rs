//! EKS Wizard rendering — `impl AppShell` methods for the wizard modal overlay.
//!
//! Follows the pod_detail_render.rs pattern: rendering methods live in this module
//! but are `impl AppShell` so they can access all view state.

use crate::components::eks_wizard::EksWizardStep;
use crate::layout::app_shell::AppShell;
use baeus_core::aws_eks::{AwsAuthMethod, DEFAULT_EKS_REGIONS};
use gpui::{Context, ElementId, FontWeight, Rgba, SharedString, div, prelude::*, px};
use gpui_component::Sizable;
use gpui_component::input::Input;

impl AppShell {
    /// Render the EKS wizard modal overlay, if active.
    /// Returns `None` if the wizard is not open.
    pub(crate) fn render_eks_wizard(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let wizard = self.eks_wizard.as_ref()?;

        let backdrop_color = crate::theme::Color::rgba(0, 0, 0, 128).to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let border = self.theme.colors.border.to_gpui();
        let text_primary = self.theme.colors.text_primary.to_gpui();
        let text_secondary = self.theme.colors.text_secondary.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let error_color = self.theme.colors.error.to_gpui();

        let step_content = match wizard.step {
            EksWizardStep::ChooseAuthMethod => {
                self.render_eks_auth_method_step(cx, text_primary, text_secondary, accent, border)
            }
            EksWizardStep::SsoConfig => {
                self.render_eks_sso_config_step(cx, text_primary, text_secondary, border)
            }
            EksWizardStep::SsoDeviceAuth => {
                self.render_eks_device_auth_step(cx, text_primary, text_secondary, accent)
            }
            EksWizardStep::SsoAccountSelection => self.render_eks_account_selection_step(
                cx,
                text_primary,
                text_secondary,
                accent,
                border,
            ),
            EksWizardStep::SsoRoleSelection => self.render_eks_role_selection_step(
                cx,
                text_primary,
                text_secondary,
                accent,
                border,
            ),
            EksWizardStep::AssumeIamRole => {
                self.render_eks_assume_iam_role_step(cx, text_primary, text_secondary, border)
            }
            EksWizardStep::AccessKeyConfig => {
                self.render_eks_access_key_step(cx, text_primary, text_secondary, border)
            }
            EksWizardStep::AssumeRoleConfig => {
                self.render_eks_assume_role_step(cx, text_primary, text_secondary, border)
            }
            EksWizardStep::RegionSelection => self.render_eks_region_selection_step(
                cx,
                text_primary,
                text_secondary,
                accent,
                border,
            ),
            EksWizardStep::Discovering => {
                self.render_eks_discovering_step(text_primary, text_secondary, accent)
            }
            EksWizardStep::ClusterResults => self.render_eks_cluster_results_step(
                cx,
                text_primary,
                text_secondary,
                accent,
                border,
            ),
        };

        // Step title
        let title = match wizard.step {
            EksWizardStep::ChooseAuthMethod => "Add EKS Clusters",
            EksWizardStep::SsoConfig => "AWS SSO Configuration",
            EksWizardStep::SsoDeviceAuth => "Browser Authorization",
            EksWizardStep::SsoAccountSelection => "Select Account",
            EksWizardStep::SsoRoleSelection => "Select Role",
            EksWizardStep::AssumeIamRole => "Assume IAM Role",
            EksWizardStep::AccessKeyConfig => "Access Key Configuration",
            EksWizardStep::AssumeRoleConfig => "IAM Role Configuration",
            EksWizardStep::RegionSelection => "Select Regions",
            EksWizardStep::Discovering => "Discovering Clusters",
            EksWizardStep::ClusterResults => "Discovered Clusters",
        };

        let can_advance = wizard.can_advance();
        let has_back = !matches!(wizard.step, EksWizardStep::ChooseAuthMethod);
        let next_label = match wizard.step {
            EksWizardStep::ClusterResults => "Connect",
            EksWizardStep::RegionSelection => "Discover",
            EksWizardStep::SsoConfig => "Authenticate",
            EksWizardStep::AccessKeyConfig => "Validate",
            EksWizardStep::AssumeRoleConfig => "Assume Role",
            EksWizardStep::AssumeIamRole => "Assume Role",
            _ => "Next",
        };

        let error_div = if let Some(ref err) = wizard.error {
            div()
                .px_4()
                .py_2()
                .bg(Rgba { r: error_color.r, g: error_color.g, b: error_color.b, a: 0.1 })
                .rounded(px(4.0))
                .mx_4()
                .mb_2()
                .child(
                    div().text_xs().text_color(error_color).child(SharedString::from(err.clone())),
                )
        } else {
            div()
        };

        let modal = div()
            .id(ElementId::Name(SharedString::from("eks-wizard-overlay")))
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .flex()
            .justify_center()
            .items_center()
            // Backdrop — visual only, no click handler (close via X or Cancel)
            .child(div().absolute().top_0().left_0().w_full().h_full().bg(backdrop_color))
            // Dialog box
            .child(
                div()
                    .id("eks-wizard-dialog")
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_this, _event, _window, _cx| {
                            // Consume mouse events so they don't reach anything behind
                        }),
                    )
                    .flex()
                    .flex_col()
                    .w(px(520.0))
                    .max_h(px(640.0))
                    .bg(surface)
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(border)
                    .overflow_hidden()
                    // Header
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_3()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(gpui::rgb(0xFF9900))
                                            .font_weight(FontWeight::BOLD)
                                            .child("AWS"),
                                    )
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(text_primary)
                                            .child(title),
                                    ),
                            )
                            .child(
                                div()
                                    .id("eks-wizard-close")
                                    .px_2()
                                    .py_1()
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .text_color(text_secondary)
                                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.1 }))
                                    .child("X")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.eks_wizard = None;
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Error display
                    .child(error_div)
                    // Body (scrollable)
                    .child(
                        div()
                            .id("eks-wizard-body")
                            .flex_1()
                            .overflow_y_scroll()
                            .px_4()
                            .py_3()
                            .child(step_content),
                    )
                    // Footer buttons
                    .when(
                        !matches!(
                            wizard.step,
                            EksWizardStep::SsoDeviceAuth | EksWizardStep::Discovering
                        ),
                        |el| {
                            el.child(self.render_eks_wizard_footer(
                                cx,
                                text_primary,
                                accent,
                                border,
                                has_back,
                                can_advance,
                                next_label,
                            ))
                        },
                    ),
            );

        Some(modal)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_eks_wizard_footer(
        &self,
        cx: &mut Context<Self>,
        text_primary: Rgba,
        accent: Rgba,
        border: Rgba,
        has_back: bool,
        can_advance: bool,
        next_label: &str,
    ) -> gpui::Div {
        let next_label = SharedString::from(next_label.to_string());
        let btn_opacity = if can_advance { 1.0 } else { 0.4 };

        div()
            .flex()
            .flex_row()
            .justify_between()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(border)
            .child(
                // Back button (or empty spacer)
                if has_back {
                    div()
                        .id("eks-wizard-back")
                        .px_4()
                        .py_2()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(border)
                        .cursor_pointer()
                        .text_sm()
                        .text_color(text_primary)
                        .child("Back")
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.eks_wizard_go_back(window, cx);
                            cx.notify();
                        }))
                } else {
                    div().id("eks-wizard-back-spacer")
                },
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id("eks-wizard-cancel")
                            .px_4()
                            .py_2()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_sm()
                            .text_color(text_primary)
                            .child("Cancel")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.eks_wizard = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("eks-wizard-next")
                            .px_4()
                            .py_2()
                            .rounded(px(6.0))
                            .bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: btn_opacity })
                            .cursor(if can_advance {
                                gpui::CursorStyle::PointingHand
                            } else {
                                gpui::CursorStyle::Arrow
                            })
                            .text_sm()
                            .text_color(gpui::rgb(0xFFFFFF))
                            .child(next_label)
                            .when(can_advance, |el| {
                                el.on_click(cx.listener(|this, _event, window, cx| {
                                    this.eks_wizard_advance(window, cx);
                                }))
                            }),
                    ),
            )
    }

    // -----------------------------------------------------------------------
    // Per-step renderers
    // -----------------------------------------------------------------------

    fn render_eks_auth_method_step(
        &self,
        cx: &mut Context<Self>,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();
        let current = &wizard.auth_method;

        let methods = [
            (
                AwsAuthMethod::Sso,
                "AWS SSO (IAM Identity Center)",
                "Device code flow — opens browser for authentication. Recommended for organizations using AWS SSO.",
            ),
            (
                AwsAuthMethod::AccessKey,
                "Access Keys",
                "Static IAM access key ID and secret. Suitable for programmatic access.",
            ),
            (
                AwsAuthMethod::AssumeRole,
                "IAM Role (Assume Role)",
                "Assume a role using existing credentials. For cross-account or role-based access.",
            ),
        ];

        let mut col = div().flex().flex_col().gap(px(8.0));
        col = col.child(
            div()
                .text_sm()
                .text_color(text_secondary)
                .mb_2()
                .child("Choose how to authenticate with AWS:"),
        );

        for (method, label, description) in &methods {
            let is_selected = current == method;
            let method_clone = method.clone();
            let outline = if is_selected { accent } else { border };

            col = col.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("eks-auth-{label}"))))
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(12.0))
                    .px_3()
                    .py_2p5()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(outline)
                    .overflow_hidden()
                    .cursor_pointer()
                    .when(is_selected, |el| {
                        el.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 })
                    })
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.auth_method = method_clone.clone();
                        }
                        cx.notify();
                    }))
                    .child(
                        // Radio indicator
                        div()
                            .flex_shrink_0()
                            .mt(px(2.0))
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_full()
                            .border_2()
                            .border_color(outline)
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_selected, |el| {
                                el.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(accent))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_primary)
                                    .child(SharedString::from(*label)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(text_secondary)
                                    .child(SharedString::from(*description)),
                            ),
                    ),
            );
        }

        col
    }

    fn render_eks_sso_config_step(
        &self,
        _cx: &mut Context<Self>,
        _text_primary: Rgba,
        text_secondary: Rgba,
        _border: Rgba,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("Enter your AWS SSO portal URL and the region where SSO is configured."),
            )
            .child(self.render_eks_input_field("SSO Start URL", "sso_start_url", text_secondary))
            .child(self.render_eks_input_field("SSO Region", "sso_region", text_secondary))
    }

    fn render_eks_device_auth_step(
        &self,
        _cx: &mut Context<Self>,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();

        let mut col = div().flex().flex_col().gap(px(16.0)).items_center();

        if let Some(ref device_auth) = wizard.sso_device_auth {
            col = col
                .child(
                    div()
                        .text_sm()
                        .text_color(text_secondary)
                        .text_center()
                        .child("A browser window has been opened. Enter this code when prompted:"),
                )
                .child(
                    div()
                        .px_6()
                        .py_3()
                        .rounded(px(8.0))
                        .bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.1 })
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(text_primary)
                                .text_center()
                                .child(SharedString::from(device_auth.user_code.clone())),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(text_secondary)
                        .text_center()
                        .child("Waiting for authorization..."),
                );
        } else {
            col = col.child(
                div().text_sm().text_color(text_secondary).child("Starting SSO device flow..."),
            );
        }

        col
    }

    fn render_eks_account_selection_step(
        &self,
        cx: &mut Context<Self>,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();

        let mut col = div().flex().flex_col().gap(px(8.0));
        col =
            col.child(div().text_sm().text_color(text_secondary).mb_1().child(SharedString::from(
                format!("Select an AWS account ({} available):", wizard.sso_accounts.len()),
            )));

        // Filter input
        col = col.child(self.render_eks_input_field("", "account_filter", text_secondary));

        let filter = wizard.filter_text.to_lowercase();
        let filtered: Vec<_> = wizard
            .sso_accounts
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                if filter.is_empty() {
                    return true;
                }
                let name = a.account_name.as_deref().unwrap_or("").to_lowercase();
                let id = a.account_id.to_lowercase();
                name.contains(&filter) || id.contains(&filter)
            })
            .collect();

        for (idx, account) in filtered {
            let is_selected = wizard
                .sso_selected_account
                .as_ref()
                .map(|a| a.account_id == account.account_id)
                .unwrap_or(false);
            let account_clone = account.clone();
            let outline = if is_selected { accent } else { border };
            let display = format!(
                "{} ({})",
                account.account_name.as_deref().unwrap_or("Unknown"),
                account.account_id,
            );

            col = col.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("eks-acct-{idx}"))))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px_3()
                    .py_2()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(outline)
                    .cursor_pointer()
                    .when(is_selected, |el| {
                        el.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 })
                    })
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.sso_selected_account = Some(account_clone.clone());
                        }
                        cx.notify();
                    }))
                    .child(
                        div().text_sm().text_color(text_primary).child(SharedString::from(display)),
                    ),
            );
        }

        col
    }

    fn render_eks_role_selection_step(
        &self,
        cx: &mut Context<Self>,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();

        let mut col = div().flex().flex_col().gap(px(8.0));
        col = col.child(div().text_sm().text_color(text_secondary).mb_1().child("Select a role:"));

        // Filter input
        col = col.child(self.render_eks_input_field("", "role_filter", text_secondary));

        let filter = wizard.filter_text.to_lowercase();
        let filtered: Vec<_> = wizard
            .sso_roles
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if filter.is_empty() {
                    return true;
                }
                r.role_name.to_lowercase().contains(&filter)
            })
            .collect();

        for (idx, role) in filtered {
            let is_selected = wizard
                .sso_selected_role
                .as_ref()
                .map(|r| r.role_name == role.role_name)
                .unwrap_or(false);
            let role_clone = role.clone();
            let outline = if is_selected { accent } else { border };

            col = col.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("eks-role-{idx}"))))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px_3()
                    .py_2()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(outline)
                    .cursor_pointer()
                    .when(is_selected, |el| {
                        el.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 })
                    })
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.sso_selected_role = Some(role_clone.clone());
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .text_color(text_primary)
                            .child(SharedString::from(role.role_name.clone())),
                    ),
            );
        }

        col
    }

    fn render_eks_assume_iam_role_step(
        &self,
        _cx: &mut Context<Self>,
        _text_primary: Rgba,
        text_secondary: Rgba,
        _border: Rgba,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div().text_sm().text_color(text_secondary)
                    .child("Your SSO role may not have direct Kubernetes access. Enter the IAM role ARN to assume for EKS cluster access (e.g. avengers, defenders, watchers)."),
            )
            .child(self.render_eks_input_field("IAM Role ARN", "iam_role_arn", text_secondary))
            .child(
                div().text_xs().text_color(Rgba { r: text_secondary.r, g: text_secondary.g, b: text_secondary.b, a: 0.6 })
                    .child("Leave empty and click Skip to use SSO credentials directly."),
            )
    }

    fn render_eks_access_key_step(
        &self,
        _cx: &mut Context<Self>,
        _text_primary: Rgba,
        text_secondary: Rgba,
        _border: Rgba,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("Enter your IAM access key credentials."),
            )
            .child(self.render_eks_input_field("Access Key ID", "access_key_id", text_secondary))
            .child(self.render_eks_input_field(
                "Secret Access Key",
                "secret_access_key",
                text_secondary,
            ))
            .child(self.render_eks_input_field(
                "Session Token (optional)",
                "session_token",
                text_secondary,
            ))
            .child(self.render_eks_input_field("Region", "access_key_region", text_secondary))
    }

    fn render_eks_assume_role_step(
        &self,
        _cx: &mut Context<Self>,
        _text_primary: Rgba,
        text_secondary: Rgba,
        _border: Rgba,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("Enter the IAM role to assume. Requires valid source credentials."),
            )
            .child(self.render_eks_input_field("Role ARN", "role_arn", text_secondary))
            .child(self.render_eks_input_field(
                "External ID (optional)",
                "external_id",
                text_secondary,
            ))
            .child(self.render_eks_input_field("Region", "assume_role_region", text_secondary))
    }

    fn render_eks_region_selection_step(
        &self,
        cx: &mut Context<Self>,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();

        let mut col = div().flex().flex_col().gap(px(8.0));
        col = col.child(
            div()
                .text_sm()
                .text_color(text_secondary)
                .mb_2()
                .child("Select regions to scan for EKS clusters:"),
        );

        // Toggle all button
        let all_selected = DEFAULT_EKS_REGIONS.iter().all(|r| wizard.selected_regions.contains(*r));
        col = col.child(
            div()
                .id("eks-toggle-all-regions")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px_2()
                .py_1()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    if let Some(ref mut w) = this.eks_wizard {
                        if all_selected {
                            w.selected_regions.clear();
                        } else {
                            for r in DEFAULT_EKS_REGIONS {
                                w.selected_regions.insert((*r).to_string());
                            }
                        }
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(accent)
                        .child(if all_selected { "Deselect All" } else { "Select All" }),
                ),
        );

        // Region grid (2 columns)
        let mut grid = div().flex().flex_row().flex_wrap().gap(px(4.0));

        for region in DEFAULT_EKS_REGIONS {
            let is_selected = wizard.selected_regions.contains(*region);
            let region_str = region.to_string();
            let region_click = region_str.clone();

            grid = grid.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("eks-region-{region}"))))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .w(px(200.0))
                    .px_2()
                    .py_1p5()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            if w.selected_regions.contains(&region_click) {
                                w.selected_regions.remove(&region_click);
                            } else {
                                w.selected_regions.insert(region_click.clone());
                            }
                        }
                        cx.notify();
                    }))
                    .child(
                        // Checkbox
                        div()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded(px(3.0))
                            .border_1()
                            .border_color(if is_selected { accent } else { border })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_selected, |el| {
                                el.bg(accent).child(
                                    div().text_xs().text_color(gpui::rgb(0xFFFFFF)).child("✓"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_primary)
                            .child(SharedString::from(region_str)),
                    ),
            );
        }

        col = col.child(grid);
        col
    }

    fn render_eks_discovering_step(
        &self,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();
        let (done, total) = wizard.discovery_progress;
        let progress_text = if total > 0 {
            format!("Scanning region {done}/{total}...")
        } else {
            "Starting discovery...".to_string()
        };
        let pct = if total > 0 { (done as f32 / total as f32).clamp(0.0, 1.0) } else { 0.0 };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.0))
            .py_8()
            .child(
                div().text_sm().text_color(text_secondary).child(SharedString::from(progress_text)),
            )
            // Progress bar
            .child(
                div()
                    .w(px(300.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.2 })
                    .child(div().h_full().rounded_full().bg(accent).w(px(300.0 * pct))),
            )
            .when(done == total && total > 0, |el| {
                el.child(
                    div().text_sm().font_weight(FontWeight::MEDIUM).text_color(text_primary).child(
                        SharedString::from(format!(
                            "Found {} cluster{}",
                            wizard.discovered_clusters.len(),
                            if wizard.discovered_clusters.len() == 1 { "" } else { "s" }
                        )),
                    ),
                )
            })
    }

    fn render_eks_cluster_results_step(
        &self,
        cx: &mut Context<Self>,
        text_primary: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
        border: Rgba,
    ) -> gpui::Div {
        let wizard = self.eks_wizard.as_ref().unwrap();

        if wizard.discovered_clusters.is_empty() {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .py_8()
                .gap(px(8.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(text_secondary)
                        .child("No EKS clusters found in the selected regions."),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(text_secondary)
                        .child("Try selecting more regions or check your permissions."),
                );
        }

        let mut col = div().flex().flex_col().gap(px(8.0));

        let total_count = wizard.discovered_clusters.len();
        let selected_count = wizard.selected_cluster_indices.len();

        col = col.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(div().text_sm().text_color(text_secondary).child(SharedString::from(
                    format!(
                        "Found {} cluster{} — {} selected",
                        total_count,
                        if total_count == 1 { "" } else { "s" },
                        selected_count,
                    ),
                )))
                .child(
                    div()
                        .id("eks-select-all-clusters")
                        .px_2()
                        .py_1()
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(accent)
                        .hover(|s| s.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.1 }))
                        .child(if selected_count == total_count {
                            "Deselect All"
                        } else {
                            "Select All"
                        })
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            if let Some(ref mut w) = this.eks_wizard {
                                if w.selected_cluster_indices.len() == w.discovered_clusters.len() {
                                    w.selected_cluster_indices.clear();
                                } else {
                                    for i in 0..w.discovered_clusters.len() {
                                        w.selected_cluster_indices.insert(i);
                                    }
                                }
                            }
                            cx.notify();
                        })),
                ),
        );

        // Filter input
        col = col.child(self.render_eks_input_field("", "cluster_filter", text_secondary));

        // Show the default role as context
        let default_role = &wizard.iam_role_arn;
        if !default_role.is_empty() {
            col = col.child(
                div()
                    .text_xs()
                    .text_color(text_secondary)
                    .child(SharedString::from(format!("Default role: {default_role}"))),
            );
        }

        // Filter clusters
        let filter = wizard.filter_text.to_lowercase();
        let filtered: Vec<(usize, &baeus_core::aws_eks::EksCluster)> = wizard
            .discovered_clusters
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if filter.is_empty() {
                    return true;
                }
                c.name.to_lowercase().contains(&filter) || c.region.to_lowercase().contains(&filter)
            })
            .collect();

        for (idx, cluster) in filtered {
            let is_selected = wizard.selected_cluster_indices.contains(&idx);
            let outline = if is_selected { accent } else { border };
            let version_label = cluster.version.as_deref().unwrap_or("?");
            let status_label = cluster.status.as_deref().unwrap_or("UNKNOWN");
            let _per_role = wizard.per_cluster_roles.get(&idx).cloned().unwrap_or_default();

            let mut card = div()
                .flex()
                .flex_col()
                .rounded(px(6.0))
                .border_1()
                .border_color(outline)
                .when(is_selected, |el| {
                    el.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 })
                });

            // Top row: checkbox + cluster info (clickable)
            card = card.child(
                div()
                    .id(ElementId::Name(SharedString::from(format!("eks-cluster-{idx}"))))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.05 }))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            if w.selected_cluster_indices.contains(&idx) {
                                w.selected_cluster_indices.remove(&idx);
                            } else {
                                w.selected_cluster_indices.insert(idx);
                            }
                        }
                        cx.notify();
                    }))
                    // Checkbox
                    .child(
                        div()
                            .flex_shrink_0()
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded(px(3.0))
                            .border_1()
                            .border_color(if is_selected { accent } else { border })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_selected, |el| {
                                el.bg(accent).child(
                                    div().text_xs().text_color(gpui::rgb(0xFFFFFF)).child("✓"),
                                )
                            }),
                    )
                    // Cluster info
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_primary)
                                    .child(SharedString::from(cluster.name.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(text_secondary)
                                            .child(SharedString::from(cluster.region.clone())),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(text_secondary)
                                            .child(SharedString::from(format!("v{version_label}"))),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(if status_label == "ACTIVE" {
                                                accent
                                            } else {
                                                text_secondary
                                            })
                                            .child(SharedString::from(status_label.to_string())),
                                    ),
                            ),
                    ),
            );

            // Per-cluster role input (shown when selected)
            if is_selected {
                let role_field_name = format!("cluster_role_{idx}");
                card = card.child(
                    div()
                        .px_3()
                        .pb_2()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(text_secondary)
                                .flex_shrink_0()
                                .child("Role:"),
                        )
                        .child(if let Some(input_entity) = wizard.inputs.get(&role_field_name) {
                            div().flex_1().child(
                                Input::new(input_entity)
                                    .appearance(true)
                                    .cleanable(false)
                                    .text_sm()
                                    .small(),
                            )
                        } else {
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(Rgba {
                                    r: text_secondary.r,
                                    g: text_secondary.g,
                                    b: text_secondary.b,
                                    a: 0.5,
                                })
                                .child(if default_role.is_empty() {
                                    "No role (uses SSO credentials directly)"
                                } else {
                                    "Loading..."
                                })
                        }),
                );
            }

            col = col.child(card);
        }

        col
    }

    // -----------------------------------------------------------------------
    // Reusable text field helper
    // -----------------------------------------------------------------------

    /// Render a labelled input field for the EKS wizard.
    /// `field_name` must match a key in `wizard.inputs` (created by `ensure_eks_inputs`).
    fn render_eks_input_field(
        &self,
        label: &str,
        field_name: &str,
        text_secondary: Rgba,
    ) -> gpui::Div {
        let label_str = SharedString::from(label.to_string());
        let wizard = self.eks_wizard.as_ref().unwrap();

        let mut col = div().flex().flex_col().gap(px(4.0)).child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_secondary)
                .child(label_str),
        );

        if let Some(input_entity) = wizard.inputs.get(field_name) {
            col = col.child(div().child(
                Input::new(input_entity).appearance(true).cleanable(false).text_sm().small(),
            ));
        } else {
            // Fallback: show static text (inputs not yet created)
            col = col.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(px(6.0))
                    .text_sm()
                    .text_color(text_secondary)
                    .child("Loading..."),
            );
        }

        col
    }
}
