//! Pod-specific detail rendering methods for `AppShell`.
//!
//! Keeps the main `app_shell.rs` slim by placing all pod-detail render logic
//! in this separate module. Rust allows `impl` blocks in any module within the
//! same crate, so these methods live directly on `AppShell`.

use gpui::*;
use gpui_component::{Icon, IconName, Sizable};

use crate::components::json_extract;
use crate::components::pod_detail::*;
use crate::icons::SectionIcon;
use crate::layout::app_shell::AppShell;

/// Create an Rgba with reduced alpha for badge backgrounds.
fn with_alpha(color: Rgba, alpha: f32) -> Rgba {
    Rgba { r: color.r, g: color.g, b: color.b, a: alpha }
}

impl AppShell {
    /// Toggle a collapsible section in the detail view.
    pub(crate) fn toggle_detail_section(&mut self, id: &str) {
        if !self.detail_collapsed_sections.remove(id) {
            self.detail_collapsed_sections.insert(id.to_string());
        }
    }

    /// Render the full pod detail body with all sections.
    pub(crate) fn render_pod_detail_body(
        &self,
        cx: &mut Context<Self>,
        json: &serde_json::Value,
        text: Rgba,
        text_secondary: Rgba,
        _bg: Rgba,
    ) -> Stateful<Div> {
        let border = self.theme.colors.border.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();

        let pod = json_extract::extract_pod_detail(json);

        let mut body = div()
            .id("pod-detail-body")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .p_4()
            .gap_3();

        // --- Properties (Overview) ---
        let props = json_extract::extract_detail_properties("Pod", json);
        body = body.child(self.render_pod_section(
            cx,
            SectionIcon::Info,
            "pod-overview",
            &props,
            text,
            text_secondary,
            border,
            accent,
            |this, _cx, props, text, text_secondary, border, _accent| {
                this.render_detail_properties_body(props, text, text_secondary, border)
            },
        ));

        // --- Labels ---
        let labels = json_extract::extract_labels(json);
        if !labels.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Labels,
                "pod-labels",
                &labels,
                text,
                text_secondary,
                border,
                accent,
                |this, _cx, labels, _text, _text_secondary, _border, _accent| {
                    this.render_detail_label_badges_body(labels, surface)
                },
            ));
        }

        // --- Annotations ---
        if !pod.annotations.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Annotations,
                "pod-annotations",
                &pod.annotations,
                text,
                text_secondary,
                border,
                accent,
                |_this, _cx, annotations, _text, text_secondary, _border, _accent| {
                    render_kv_badges(annotations, text_secondary, surface)
                },
            ));
        }

        // --- Conditions ---
        let conditions = json_extract::extract_conditions(json);
        if !conditions.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Conditions,
                "pod-conditions",
                &conditions,
                text,
                text_secondary,
                border,
                accent,
                |this, _cx, conditions, _text, text_secondary, _border, _accent| {
                    this.render_detail_conditions_body(conditions, text_secondary, surface, border)
                },
            ));
        }

        // --- Containers ---
        if !pod.containers.is_empty() {
            let container_count = pod.containers.len();
            body = body.child(self.render_pod_section_with_count(
                cx,
                SectionIcon::Containers,
                "pod-containers",
                container_count,
                &pod.containers,
                text,
                text_secondary,
                border,
                accent,
                |_this, cx, containers, text, text_secondary, border, accent| {
                    let mut section = div().flex().flex_col().gap_2();
                    for container in containers {
                        section = section.child(render_container_card(
                            cx,
                            container,
                            text,
                            text_secondary,
                            border,
                            accent,
                            surface,
                        ));
                    }
                    section
                },
            ));
        }

        // --- Init Containers ---
        if !pod.init_containers.is_empty() {
            let count = pod.init_containers.len();
            body = body.child(self.render_pod_section_with_count(
                cx,
                SectionIcon::InitContainers,
                "pod-init-containers",
                count,
                &pod.init_containers,
                text,
                text_secondary,
                border,
                accent,
                |_this, cx, containers, text, text_secondary, border, accent| {
                    let mut section = div().flex().flex_col().gap_2();
                    for container in containers {
                        section = section.child(render_container_card(
                            cx,
                            container,
                            text,
                            text_secondary,
                            border,
                            accent,
                            surface,
                        ));
                    }
                    section
                },
            ));
        }

        // --- Volumes ---
        if !pod.volumes.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Volumes,
                "pod-volumes",
                &pod.volumes,
                text,
                text_secondary,
                border,
                accent,
                |_this, _cx, volumes, text, text_secondary, border, _accent| {
                    render_volumes_table(volumes, text, text_secondary, border)
                },
            ));
        }

        // --- Tolerations ---
        if !pod.tolerations.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Tolerations,
                "pod-tolerations",
                &pod.tolerations,
                text,
                text_secondary,
                border,
                accent,
                |_this, _cx, tolerations, text, text_secondary, border, _accent| {
                    render_tolerations_table(tolerations, text, text_secondary, border)
                },
            ));
        }

        // --- Affinity ---
        if let Some(ref affinity_json) = pod.affinity_json {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Affinity,
                "pod-affinity",
                affinity_json,
                text,
                text_secondary,
                border,
                accent,
                |_this, _cx, json_str, _text, text_secondary, _border, _accent| {
                    div().p_2().rounded_md().bg(surface).child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(text_secondary)
                            .child(SharedString::from(json_str.clone())),
                    )
                },
            ));
        }

        // --- Node Selector ---
        if !pod.node_selector.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::NodeSelector,
                "pod-node-selector",
                &pod.node_selector,
                text,
                text_secondary,
                border,
                accent,
                |_this, _cx, selectors, _text, text_secondary, _border, _accent| {
                    render_kv_badges(selectors, text_secondary, surface)
                },
            ));
        }

        body
    }

    /// Render a collapsible section with icon, label, chevron, and content.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_pod_section<T, F>(
        &self,
        cx: &mut Context<Self>,
        icon: SectionIcon,
        section_id: &str,
        data: &T,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        accent: Rgba,
        render_body: F,
    ) -> Div
    where
        T: Clone,
        F: Fn(&Self, &mut Context<Self>, &T, Rgba, Rgba, Rgba, Rgba) -> Div,
    {
        let expanded = !self.detail_collapsed_sections.contains(section_id);
        let section_id_owned = section_id.to_string();

        let mut section = div().flex().flex_col();

        // Header
        let header = self.render_pod_section_header(
            cx,
            icon,
            &section_id_owned,
            expanded,
            text,
            text_secondary,
            accent,
        );
        section = section.child(header);

        // Body (conditionally rendered)
        if expanded {
            section =
                section.child(render_body(self, cx, data, text, text_secondary, border, accent));
        }

        section
    }

    /// Render a collapsible section with a count badge.
    #[allow(clippy::too_many_arguments)]
    fn render_pod_section_with_count<T, F>(
        &self,
        cx: &mut Context<Self>,
        icon: SectionIcon,
        section_id: &str,
        count: usize,
        data: &T,
        text: Rgba,
        text_secondary: Rgba,
        border: Rgba,
        accent: Rgba,
        render_body: F,
    ) -> Div
    where
        T: Clone,
        F: Fn(&Self, &mut Context<Self>, &T, Rgba, Rgba, Rgba, Rgba) -> Div,
    {
        let expanded = !self.detail_collapsed_sections.contains(section_id);
        let section_id_owned = section_id.to_string();

        let mut section = div().flex().flex_col();

        // Header with count badge
        let chevron = if expanded {
            Icon::new(IconName::ChevronDown).xsmall()
        } else {
            Icon::new(IconName::ChevronRight).xsmall()
        };

        let header = div()
            .id(ElementId::Name(SharedString::from(format!("section-hdr-{section_id_owned}"))))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .py_2()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                this.toggle_detail_section(&section_id_owned);
            }))
            .child(div().text_color(text_secondary).child(chevron))
            .child(div().text_color(accent).child(Icon::new(icon).xsmall()))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text)
                    .text_sm()
                    .child(SharedString::from(icon.label().to_string())),
            )
            .child(
                div()
                    .px_2()
                    .py(px(1.0))
                    .rounded_sm()
                    .bg(with_alpha(accent, 0.15))
                    .text_xs()
                    .text_color(accent)
                    .child(SharedString::from(count.to_string())),
            );

        section = section.child(header);

        if expanded {
            section =
                section.child(render_body(self, cx, data, text, text_secondary, border, accent));
        }

        section
    }

    /// Render a section header with chevron, icon, and label.
    #[allow(clippy::too_many_arguments)]
    fn render_pod_section_header(
        &self,
        cx: &mut Context<Self>,
        icon: SectionIcon,
        section_id: &str,
        expanded: bool,
        text: Rgba,
        text_secondary: Rgba,
        accent: Rgba,
    ) -> Stateful<Div> {
        let section_id_owned = section_id.to_string();

        let chevron = if expanded {
            Icon::new(IconName::ChevronDown).xsmall()
        } else {
            Icon::new(IconName::ChevronRight).xsmall()
        };

        div()
            .id(ElementId::Name(SharedString::from(format!("section-hdr-{section_id}"))))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .py_2()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                this.toggle_detail_section(&section_id_owned);
            }))
            .child(div().text_color(text_secondary).child(chevron))
            .child(div().text_color(accent).child(Icon::new(icon).xsmall()))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text)
                    .text_sm()
                    .child(SharedString::from(icon.label().to_string())),
            )
    }
}

// ---------------------------------------------------------------------------
// Free functions for rendering sub-components
// ---------------------------------------------------------------------------

/// Render a container detail card.
#[allow(clippy::too_many_arguments)]
fn render_container_card(
    _cx: &mut Context<AppShell>,
    container: &ContainerDetail,
    text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    accent: Rgba,
    surface: Rgba,
) -> Div {
    let mut card = div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(surface);

    // Header row: name, status badge, ready dot, restart count
    let state_label = match &container.state {
        ContainerStateDetail::Running { .. } => "Running",
        ContainerStateDetail::Waiting { reason, .. } => {
            if reason.is_empty() {
                "Waiting"
            } else {
                reason.as_str()
            }
        }
        ContainerStateDetail::Terminated { reason, .. } => {
            if reason.is_empty() {
                "Terminated"
            } else {
                reason.as_str()
            }
        }
        ContainerStateDetail::Unknown => "Unknown",
    };

    let state_color = match &container.state {
        ContainerStateDetail::Running { .. } => gpui::rgb(0x22C55E),
        ContainerStateDetail::Waiting { .. } => gpui::rgb(0xF59E0B),
        ContainerStateDetail::Terminated { .. } => gpui::rgb(0xEF4444),
        ContainerStateDetail::Unknown => gpui::rgb(0x6B7280),
    };

    let ready_color = if container.ready { gpui::rgb(0x22C55E) } else { gpui::rgb(0xEF4444) };

    let mut header = div().flex().flex_row().items_center().gap_2();
    header = header.child(
        div()
            .font_weight(FontWeight::SEMIBOLD)
            .text_sm()
            .text_color(text)
            .child(SharedString::from(container.name.clone())),
    );
    header = header.child(
        div()
            .px_2()
            .py(px(1.0))
            .rounded_sm()
            .bg(with_alpha(state_color, 0.15))
            .text_xs()
            .text_color(state_color)
            .child(SharedString::from(state_label.to_string())),
    );
    header = header.child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(ready_color));
    if container.restart_count > 0 {
        header = header.child(
            div()
                .px_2()
                .py(px(1.0))
                .rounded_sm()
                .bg(with_alpha(gpui::rgb(0xF59E0B), 0.15))
                .text_xs()
                .text_color(gpui::rgb(0xF59E0B))
                .child(SharedString::from(format!("{} restarts", container.restart_count))),
        );
    }
    card = card.child(header);

    // Image sub-section
    if !container.image.is_empty() {
        card = card.child(render_mini_section(
            SectionIcon::Image,
            "Image",
            &container.image,
            text,
            text_secondary,
            accent,
        ));
    }

    // Ports
    if !container.ports.is_empty() {
        let ports_text = container
            .ports
            .iter()
            .map(|p| {
                let mut s = format!("{}/{}", p.container_port, p.protocol);
                if !p.name.is_empty() {
                    s = format!("{} ({})", s, p.name);
                }
                if let Some(hp) = p.host_port {
                    s = format!("{s} -> host:{hp}");
                }
                s
            })
            .collect::<Vec<_>>()
            .join(", ");
        card = card.child(render_mini_section(
            SectionIcon::Ports,
            "Ports",
            &ports_text,
            text,
            text_secondary,
            accent,
        ));
    }

    // Env Vars
    if !container.env_vars.is_empty() {
        card =
            card.child(render_env_table(&container.env_vars, text, text_secondary, border, accent));
    }

    // Volume Mounts
    if !container.volume_mounts.is_empty() {
        card = card.child(render_volume_mounts_table(
            &container.volume_mounts,
            text,
            text_secondary,
            border,
            accent,
        ));
    }

    // Resources
    let res = &container.resources;
    if res.requests_cpu != "—"
        || res.requests_memory != "—"
        || res.limits_cpu != "—"
        || res.limits_memory != "—"
    {
        card = card.child(render_resources_section(res, text, text_secondary, border, accent));
    }

    // Probes
    let probes: Vec<&ProbeDetail> = [
        container.liveness_probe.as_ref(),
        container.readiness_probe.as_ref(),
        container.startup_probe.as_ref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !probes.is_empty() {
        card = card.child(render_probes_section(&probes, text, text_secondary, border, accent));
    }

    // Security Context
    if let Some(ref sc) = container.security_context {
        card = card.child(render_security_section(sc, text, text_secondary, border, accent));
    }

    // Command / Args
    if !container.command.is_empty() || !container.args.is_empty() {
        let mut parts = Vec::new();
        if !container.command.is_empty() {
            parts.push(format!("cmd: [{}]", container.command.join(", ")));
        }
        if !container.args.is_empty() {
            parts.push(format!("args: [{}]", container.args.join(", ")));
        }
        if !container.working_dir.is_empty() {
            parts.push(format!("workDir: {}", container.working_dir));
        }
        let cmd_text = parts.join("  ");
        card = card.child(render_mini_section(
            SectionIcon::Terminal,
            "Command",
            &cmd_text,
            text,
            text_secondary,
            accent,
        ));
    }

    card
}

/// A minimal sub-section: icon + label + single-line value.
fn render_mini_section(
    icon: SectionIcon,
    label: &str,
    value: &str,
    _text: Rgba,
    text_secondary: Rgba,
    accent: Rgba,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(div().text_color(accent).child(Icon::new(icon).xsmall()))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_secondary)
                .child(SharedString::from(format!("{label}:"))),
        )
        .child(
            div()
                .text_xs()
                .text_color(gpui::rgb(0xE5E7EB))
                .child(SharedString::from(value.to_string())),
        )
}

/// Render environment variables as a compact table.
fn render_env_table(
    env_vars: &[EnvVarDetail],
    text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    accent: Rgba,
) -> Div {
    let mut section = div().flex().flex_col().gap_1();

    section = section.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().text_color(accent).child(Icon::new(SectionIcon::EnvVars).xsmall()))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_secondary)
                    .child("Env Variables"),
            ),
    );

    for env in env_vars.iter().take(20) {
        let value_display = if !env.value_from.is_empty() {
            env.value_from.clone()
        } else {
            let v = &env.value;
            if v.len() > 60 { format!("{}...", &v[..57]) } else { v.clone() }
        };

        section = section.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .py(px(1.0))
                .child(
                    div()
                        .w(px(140.0))
                        .flex_shrink_0()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(env.name.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(value_display)),
                ),
        );
    }

    if env_vars.len() > 20 {
        section = section.child(
            div()
                .text_xs()
                .text_color(text_secondary)
                .child(SharedString::from(format!("... and {} more", env_vars.len() - 20))),
        );
    }

    section
}

/// Render volume mounts as a compact table.
fn render_volume_mounts_table(
    mounts: &[VolumeMountDetail],
    text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    accent: Rgba,
) -> Div {
    let mut section = div().flex().flex_col().gap_1();

    section = section.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().text_color(accent).child(Icon::new(SectionIcon::VolumeMounts).xsmall()))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_secondary)
                    .child("Volume Mounts"),
            ),
    );

    for mount in mounts {
        let ro_badge = if mount.read_only { " [ro]" } else { "" };
        let sub = if mount.sub_path.is_empty() {
            String::new()
        } else {
            format!(" (sub: {})", mount.sub_path)
        };
        let detail = format!("{}{}{}", mount.mount_path, ro_badge, sub);

        section = section.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .py(px(1.0))
                .child(
                    div()
                        .w(px(120.0))
                        .flex_shrink_0()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(mount.name.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(detail)),
                ),
        );
    }

    section
}

/// Render resource requests/limits.
fn render_resources_section(
    resources: &ContainerResources,
    _text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    accent: Rgba,
) -> Div {
    let mut section = div().flex().flex_col().gap_1();

    section = section.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().text_color(accent).child(Icon::new(SectionIcon::Resources).xsmall()))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_secondary)
                    .child("Resources"),
            ),
    );

    let rows = [
        ("CPU Request", &resources.requests_cpu),
        ("CPU Limit", &resources.limits_cpu),
        ("Memory Request", &resources.requests_memory),
        ("Memory Limit", &resources.limits_memory),
    ];

    for (label, value) in &rows {
        if *value != "—" {
            section = section.child(
                div()
                    .flex()
                    .flex_row()
                    .border_b_1()
                    .border_color(border)
                    .py(px(1.0))
                    .child(
                        div()
                            .w(px(120.0))
                            .flex_shrink_0()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text_secondary)
                            .child(SharedString::from(label.to_string())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(gpui::rgb(0xE5E7EB))
                            .child(SharedString::from((*value).clone())),
                    ),
            );
        }
    }

    section
}

/// Render probes section.
fn render_probes_section(
    probes: &[&ProbeDetail],
    _text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    accent: Rgba,
) -> Div {
    let mut section = div().flex().flex_col().gap_1();

    section = section.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().text_color(accent).child(Icon::new(SectionIcon::Probes).xsmall()))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_secondary)
                    .child("Probes"),
            ),
    );

    for probe in probes {
        let timing = format!(
            "delay={}s period={}s timeout={}s success={} failure={}",
            probe.initial_delay,
            probe.period,
            probe.timeout,
            probe.success_threshold,
            probe.failure_threshold,
        );

        section = section.child(
            div()
                .flex()
                .flex_col()
                .border_b_1()
                .border_color(border)
                .py(px(2.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(accent)
                                .child(SharedString::from(probe.probe_type.clone())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(gpui::rgb(0xE5E7EB))
                                .child(SharedString::from(probe.detail.clone())),
                        ),
                )
                .child(
                    div().text_xs().text_color(text_secondary).child(SharedString::from(timing)),
                ),
        );
    }

    section
}

/// Render security context section.
fn render_security_section(
    sc: &SecurityContextDetail,
    _text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    accent: Rgba,
) -> Div {
    let mut section = div().flex().flex_col().gap_1();

    section = section.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().text_color(accent).child(Icon::new(SectionIcon::Security).xsmall()))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text_secondary)
                    .child("Security Context"),
            ),
    );

    let mut rows: Vec<(String, String)> = Vec::new();
    if let Some(u) = sc.run_as_user {
        rows.push(("runAsUser".to_string(), u.to_string()));
    }
    if let Some(g) = sc.run_as_group {
        rows.push(("runAsGroup".to_string(), g.to_string()));
    }
    if let Some(b) = sc.run_as_non_root {
        rows.push(("runAsNonRoot".to_string(), b.to_string()));
    }
    if let Some(b) = sc.read_only_root_fs {
        rows.push(("readOnlyRootFilesystem".to_string(), b.to_string()));
    }
    if let Some(b) = sc.privileged {
        rows.push(("privileged".to_string(), b.to_string()));
    }
    if !sc.caps_add.is_empty() {
        rows.push(("capabilities.add".to_string(), sc.caps_add.join(", ")));
    }
    if !sc.caps_drop.is_empty() {
        rows.push(("capabilities.drop".to_string(), sc.caps_drop.join(", ")));
    }

    for (label, value) in &rows {
        section = section.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .py(px(1.0))
                .child(
                    div()
                        .w(px(160.0))
                        .flex_shrink_0()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text_secondary)
                        .child(SharedString::from(label.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(gpui::rgb(0xE5E7EB))
                        .child(SharedString::from(value.clone())),
                ),
        );
    }

    section
}

/// Render key-value pairs as inline badges (for annotations, node selectors).
fn render_kv_badges(pairs: &[(String, String)], text_secondary: Rgba, surface: Rgba) -> Div {
    let mut badges_row = div().flex().flex_row().flex_wrap().gap_1();
    for (key, value) in pairs {
        let badge_text = SharedString::from(format!("{key}={value}"));
        let badge = div()
            .px_2()
            .py(px(2.0))
            .rounded_sm()
            .bg(surface)
            .text_xs()
            .text_color(text_secondary)
            .child(badge_text);
        badges_row = badges_row.child(badge);
    }
    badges_row
}

/// Render a volumes table.
fn render_volumes_table(
    volumes: &[VolumeDetail],
    text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
) -> Div {
    let mut table = div().flex().flex_col().gap_0();

    // Header
    table = table.child(
        div()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .w(px(140.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Name"),
            )
            .child(
                div()
                    .w(px(120.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Type"),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Detail"),
            ),
    );

    for vol in volumes {
        table = table.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .w(px(140.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(vol.name.clone())),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(gpui::rgb(0x60A5FA))
                        .child(SharedString::from(vol.volume_type.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(vol.type_detail.clone())),
                ),
        );
    }

    table
}

/// Render a tolerations table.
fn render_tolerations_table(
    tolerations: &[TolerationDetail],
    text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
) -> Div {
    let mut table = div().flex().flex_col().gap_0();

    // Header
    table = table.child(
        div()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .w(px(160.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Key"),
            )
            .child(
                div()
                    .w(px(80.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Operator"),
            )
            .child(
                div()
                    .w(px(100.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Value"),
            )
            .child(
                div()
                    .w(px(100.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Effect"),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Seconds"),
            ),
    );

    for tol in tolerations {
        let secs = tol.toleration_seconds.map(|s| s.to_string()).unwrap_or_else(|| "—".to_string());
        table = table.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .w(px(160.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(tol.key.clone())),
                )
                .child(
                    div()
                        .w(px(80.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(tol.operator.clone())),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(tol.value.clone())),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(tol.effect.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(secs)),
                ),
        );
    }

    table
}
