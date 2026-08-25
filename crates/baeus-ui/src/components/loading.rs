use gpui::{Context, Rgba, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Visual variant for a loading indicator.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingVariant {
    Spinner,
    Skeleton,
    Dots,
    Bar,
}

/// State for a loading indicator.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadingState {
    pub variant: LoadingVariant,
    pub message: Option<String>,
    pub progress: Option<f32>, // 0.0 to 1.0 for Bar variant
}

impl LoadingState {
    /// Creates a spinner loading state with an optional message.
    pub fn spinner(message: Option<String>) -> Self {
        Self { variant: LoadingVariant::Spinner, message, progress: None }
    }

    /// Creates a skeleton loading state (for table-like placeholders).
    pub fn skeleton(_rows: usize, _columns: usize) -> Self {
        Self { variant: LoadingVariant::Skeleton, message: None, progress: None }
    }

    /// Creates a progress bar loading state with progress clamped to 0.0..=1.0.
    pub fn progress_bar(progress: f32, message: Option<String>) -> Self {
        Self { variant: LoadingVariant::Bar, message, progress: Some(progress.clamp(0.0, 1.0)) }
    }

    /// Creates a dots loading state with an optional message.
    pub fn dots(message: Option<String>) -> Self {
        Self { variant: LoadingVariant::Dots, message, progress: None }
    }
}

/// Configuration for a skeleton loading placeholder.
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonConfig {
    pub rows: usize,
    pub columns: usize,
    pub row_height: f32,
}

impl SkeletonConfig {
    /// Creates a new skeleton config.
    pub fn new(rows: usize, columns: usize, row_height: f32) -> Self {
        Self { rows, columns, row_height }
    }

    /// Default configuration for a table skeleton (10 rows, 4 columns, 32px).
    pub fn table_default() -> Self {
        Self { rows: 10, columns: 4, row_height: 32.0 }
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering loading indicators.
#[allow(dead_code)]
struct LoadingColors {
    surface: Rgba,
    surface_hover: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    border: Rgba,
}

/// View component that renders a loading indicator.
pub struct LoadingViewComponent {
    pub state: LoadingState,
    pub skeleton_config: Option<SkeletonConfig>,
    pub theme: Theme,
}

impl LoadingViewComponent {
    pub fn new(state: LoadingState, theme: Theme) -> Self {
        Self { state, skeleton_config: None, theme }
    }

    pub fn with_skeleton_config(mut self, config: SkeletonConfig) -> Self {
        self.skeleton_config = Some(config);
        self
    }

    /// Render a spinner indicator.
    fn render_spinner(&self, colors: &LoadingColors) -> gpui::Div {
        let mut container = div().flex().flex_col().items_center().justify_center().gap(px(12.0));

        // Spinner circle (static representation of a rotating indicator)
        container = container.child(
            div()
                .w(px(32.0))
                .h(px(32.0))
                .rounded_full()
                .border_2()
                .border_color(colors.surface_hover)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(px(32.0))
                        .h(px(32.0))
                        .rounded_full()
                        .border_2()
                        .border_color(colors.accent),
                ),
        );

        if let Some(msg) = &self.state.message {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(msg.clone())),
            );
        }

        container
    }

    /// Render skeleton placeholder rows.
    fn render_skeleton(&self, colors: &LoadingColors) -> gpui::Div {
        let config =
            self.skeleton_config.as_ref().cloned().unwrap_or_else(SkeletonConfig::table_default);

        let mut container = div().flex().flex_col().gap(px(4.0)).w_full();

        for _row in 0..config.rows {
            let mut row_div = div().flex().flex_row().gap(px(8.0)).h(px(config.row_height));

            for _col in 0..config.columns {
                row_div = row_div
                    .child(div().flex_1().h_full().rounded(px(4.0)).bg(colors.surface_hover));
            }

            container = container.child(row_div);
        }

        container
    }

    /// Render animated dots (static "..." representation).
    fn render_dots(&self, colors: &LoadingColors) -> gpui::Div {
        let msg = self.state.message.as_deref().unwrap_or("Loading");

        let display_text = format!("{}...", msg);

        div().flex().items_center().justify_center().child(
            div()
                .text_sm()
                .text_color(colors.text_secondary)
                .child(SharedString::from(display_text)),
        )
    }

    /// Render a progress bar.
    fn render_bar(&self, colors: &LoadingColors) -> gpui::Div {
        let progress = self.state.progress.unwrap_or(0.0);
        let bar_width: f32 = 240.0;
        let fill_width = bar_width * progress;

        let mut container = div().flex().flex_col().items_center().gap(px(8.0));

        // Bar track
        let bar = div()
            .w(px(bar_width))
            .h(px(8.0))
            .rounded(px(4.0))
            .bg(colors.surface_hover)
            .child(div().h_full().w(px(fill_width)).rounded(px(4.0)).bg(colors.accent));

        container = container.child(bar);

        // Percentage text
        let pct_text = format!("{}%", (progress * 100.0).round() as u32);
        container = container.child(
            div().text_xs().text_color(colors.text_muted).child(SharedString::from(pct_text)),
        );

        if let Some(msg) = &self.state.message {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(msg.clone())),
            );
        }

        container
    }
}

impl Render for LoadingViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = LoadingColors {
            surface: self.theme.colors.surface.to_gpui(),
            surface_hover: self.theme.colors.surface_hover.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
        };

        match &self.state.variant {
            LoadingVariant::Spinner => self.render_spinner(&colors),
            LoadingVariant::Skeleton => self.render_skeleton(&colors),
            LoadingVariant::Dots => self.render_dots(&colors),
            LoadingVariant::Bar => self.render_bar(&colors),
        }
    }
}
