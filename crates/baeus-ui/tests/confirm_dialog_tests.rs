// Tests extracted from crates/baeus-ui/src/components/confirm_dialog.rs

use baeus_ui::components::confirm_dialog::*;
use baeus_ui::theme::Theme;

// --- new() ---

#[test]
fn test_new_creates_hidden_dialog() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    assert!(!dialog.visible);
}

#[test]
fn test_new_sets_title_and_message() {
    let dialog = ConfirmDialogState::new("My Title", "My Message", DialogSeverity::Warning);
    assert_eq!(dialog.title, "My Title");
    assert_eq!(dialog.message, "My Message");
}

#[test]
fn test_new_sets_severity() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Destructive);
    assert_eq!(dialog.severity, DialogSeverity::Destructive);
}

#[test]
fn test_new_has_default_labels() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    assert_eq!(dialog.confirm_label, "Confirm");
    assert_eq!(dialog.cancel_label, "Cancel");
}

#[test]
fn test_new_has_no_resource_name() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    assert!(dialog.resource_name.is_none());
}

// --- show() / hide() ---

#[test]
fn test_show_makes_dialog_visible() {
    let mut dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    assert!(!dialog.visible);
    dialog.show();
    assert!(dialog.visible);
}

#[test]
fn test_hide_makes_dialog_invisible() {
    let mut dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    dialog.show();
    assert!(dialog.visible);
    dialog.hide();
    assert!(!dialog.visible);
}

#[test]
fn test_show_is_idempotent() {
    let mut dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    dialog.show();
    dialog.show();
    assert!(dialog.visible);
}

#[test]
fn test_hide_is_idempotent() {
    let mut dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    dialog.hide();
    dialog.hide();
    assert!(!dialog.visible);
}

// --- with_confirm_label() ---

#[test]
fn test_with_confirm_label() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info)
        .with_confirm_label("Yes, do it");
    assert_eq!(dialog.confirm_label, "Yes, do it");
}

// --- with_cancel_label() ---

#[test]
fn test_with_cancel_label() {
    let dialog =
        ConfirmDialogState::new("Title", "Message", DialogSeverity::Info).with_cancel_label("Nope");
    assert_eq!(dialog.cancel_label, "Nope");
}

// --- with_resource_name() ---

#[test]
fn test_with_resource_name() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info)
        .with_resource_name("my-pod");
    assert_eq!(dialog.resource_name.as_deref(), Some("my-pod"));
}

// --- is_destructive() ---

#[test]
fn test_is_destructive_true_for_destructive_severity() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Destructive);
    assert!(dialog.is_destructive());
}

#[test]
fn test_is_destructive_false_for_info() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    assert!(!dialog.is_destructive());
}

#[test]
fn test_is_destructive_false_for_warning() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Warning);
    assert!(!dialog.is_destructive());
}

// --- delete_resource() ---

#[test]
fn test_delete_resource_title() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    assert_eq!(dialog.title, "Delete Pod");
}

#[test]
fn test_delete_resource_message() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    assert!(dialog.message.contains("Pod"));
    assert!(dialog.message.contains("\"nginx\""));
    assert!(dialog.message.contains("cannot be undone"));
}

#[test]
fn test_delete_resource_is_destructive() {
    let dialog = ConfirmDialogState::delete_resource("Deployment", "app");
    assert!(dialog.is_destructive());
    assert_eq!(dialog.severity, DialogSeverity::Destructive);
}

#[test]
fn test_delete_resource_confirm_label() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    assert_eq!(dialog.confirm_label, "Delete");
}

#[test]
fn test_delete_resource_has_resource_name() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    assert_eq!(dialog.resource_name.as_deref(), Some("nginx"));
}

#[test]
fn test_delete_resource_starts_hidden() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    assert!(!dialog.visible);
}

// --- scale_resource() ---

#[test]
fn test_scale_resource_title() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 5);
    assert_eq!(dialog.title, "Scale Deployment");
}

#[test]
fn test_scale_resource_message() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 5);
    assert!(dialog.message.contains("Deployment"));
    assert!(dialog.message.contains("\"app\""));
    assert!(dialog.message.contains("5 replicas"));
}

#[test]
fn test_scale_resource_severity_is_warning() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 3);
    assert_eq!(dialog.severity, DialogSeverity::Warning);
    assert!(!dialog.is_destructive());
}

#[test]
fn test_scale_resource_confirm_label() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 3);
    assert_eq!(dialog.confirm_label, "Scale");
}

#[test]
fn test_scale_resource_has_resource_name() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 3);
    assert_eq!(dialog.resource_name.as_deref(), Some("app"));
}

#[test]
fn test_scale_resource_starts_hidden() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 3);
    assert!(!dialog.visible);
}

#[test]
fn test_scale_resource_zero_replicas() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 0);
    assert!(dialog.message.contains("0 replicas"));
}

// --- restart_resource() ---

#[test]
fn test_restart_resource_title() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api-server");
    assert_eq!(dialog.title, "Restart Deployment");
}

#[test]
fn test_restart_resource_message() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api-server");
    assert!(dialog.message.contains("Deployment"));
    assert!(dialog.message.contains("\"api-server\""));
    assert!(dialog.message.contains("downtime"));
}

#[test]
fn test_restart_resource_severity_is_warning() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api-server");
    assert_eq!(dialog.severity, DialogSeverity::Warning);
    assert!(!dialog.is_destructive());
}

#[test]
fn test_restart_resource_confirm_label() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api-server");
    assert_eq!(dialog.confirm_label, "Restart");
}

#[test]
fn test_restart_resource_has_resource_name() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api-server");
    assert_eq!(dialog.resource_name.as_deref(), Some("api-server"));
}

#[test]
fn test_restart_resource_starts_hidden() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api-server");
    assert!(!dialog.visible);
}

// --- builder chaining ---

#[test]
fn test_builder_chaining_all_methods() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info)
        .with_confirm_label("OK")
        .with_cancel_label("Abort")
        .with_resource_name("my-resource");
    assert_eq!(dialog.confirm_label, "OK");
    assert_eq!(dialog.cancel_label, "Abort");
    assert_eq!(dialog.resource_name.as_deref(), Some("my-resource"));
}

#[test]
fn test_default_cancel_label_preserved_by_factory_methods() {
    let delete = ConfirmDialogState::delete_resource("Pod", "nginx");
    assert_eq!(delete.cancel_label, "Cancel");

    let scale = ConfirmDialogState::scale_resource("Deployment", "app", 3);
    assert_eq!(scale.cancel_label, "Cancel");

    let restart = ConfirmDialogState::restart_resource("Deployment", "api");
    assert_eq!(restart.cancel_label, "Cancel");
}

// --- DialogSeverity serialization ---

#[test]
fn test_dialog_severity_serialization_roundtrip() {
    for severity in [DialogSeverity::Info, DialogSeverity::Warning, DialogSeverity::Destructive] {
        let json = serde_json::to_string(&severity).unwrap();
        let deserialized: DialogSeverity = serde_json::from_str(&json).unwrap();
        assert_eq!(severity, deserialized);
    }
}

// ========================================================================
// T032: Render-related state tests for ConfirmDialog
// ========================================================================

#[test]
fn test_view_confirm_button_color_destructive() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert_eq!(view.confirm_button_color(), Theme::dark().colors.error);
}

#[test]
fn test_view_confirm_button_color_warning() {
    let dialog = ConfirmDialogState::scale_resource("Deployment", "app", 3);
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert_eq!(view.confirm_button_color(), Theme::dark().colors.warning);
}

#[test]
fn test_view_confirm_button_color_info() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert_eq!(view.confirm_button_color(), Theme::dark().colors.accent);
}

#[test]
fn test_view_confirm_button_color_light_theme() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    let view = ConfirmDialogView::new(dialog, Theme::light());
    assert_eq!(view.confirm_button_color(), Theme::light().colors.error);
}

#[test]
fn test_view_hidden_dialog_not_rendered() {
    let dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert!(!view.state.visible);
}

#[test]
fn test_view_visible_dialog_rendered() {
    let mut dialog = ConfirmDialogState::new("Title", "Message", DialogSeverity::Info);
    dialog.show();
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert!(view.state.visible);
}

#[test]
fn test_view_destructive_uses_error_color() {
    let dialog = ConfirmDialogState::delete_resource("Deployment", "nginx");
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    let color = view.confirm_button_color();
    let theme = Theme::dark();
    assert_eq!(color, theme.colors.error);
}

#[test]
fn test_view_warning_uses_warning_color() {
    let dialog = ConfirmDialogState::restart_resource("Deployment", "api");
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    let color = view.confirm_button_color();
    let theme = Theme::dark();
    assert_eq!(color, theme.colors.warning);
}

#[test]
fn test_view_severity_colors_differ() {
    let dark = Theme::dark();

    let info_dialog = ConfirmDialogState::new("T", "M", DialogSeverity::Info);
    let info_view = ConfirmDialogView::new(info_dialog, dark.clone());

    let warn_dialog = ConfirmDialogState::new("T", "M", DialogSeverity::Warning);
    let warn_view = ConfirmDialogView::new(warn_dialog, dark.clone());

    let dest_dialog = ConfirmDialogState::new("T", "M", DialogSeverity::Destructive);
    let dest_view = ConfirmDialogView::new(dest_dialog, dark);

    assert_ne!(info_view.confirm_button_color(), warn_view.confirm_button_color());
    assert_ne!(warn_view.confirm_button_color(), dest_view.confirm_button_color());
    assert_ne!(info_view.confirm_button_color(), dest_view.confirm_button_color());
}

#[test]
fn test_view_title_and_message_accessible() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx-pod");
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert_eq!(view.state.title, "Delete Pod");
    assert!(view.state.message.contains("nginx-pod"));
}

#[test]
fn test_view_button_labels_accessible() {
    let dialog = ConfirmDialogState::delete_resource("Pod", "nginx");
    let view = ConfirmDialogView::new(dialog, Theme::dark());
    assert_eq!(view.state.confirm_label, "Delete");
    assert_eq!(view.state.cancel_label, "Cancel");
}
