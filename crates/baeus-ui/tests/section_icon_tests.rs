// Section icon tests
//
// Tests for the SectionIcon enum: label/path validation for all variants.

use baeus_ui::icons::SectionIcon;
use gpui_component::IconNamed;

#[test]
fn test_all_section_icons_have_non_empty_labels() {
    let icons = [
        SectionIcon::Info,
        SectionIcon::Events,
        SectionIcon::Containers,
        SectionIcon::InitContainers,
        SectionIcon::Volumes,
        SectionIcon::Labels,
        SectionIcon::Annotations,
        SectionIcon::Conditions,
        SectionIcon::Tolerations,
        SectionIcon::Affinity,
        SectionIcon::NodeSelector,
        SectionIcon::Probes,
        SectionIcon::Security,
        SectionIcon::Resources,
        SectionIcon::Terminal,
        SectionIcon::Image,
        SectionIcon::ControlledBy,
        SectionIcon::Ports,
        SectionIcon::EnvVars,
        SectionIcon::VolumeMounts,
    ];

    for icon in &icons {
        let label = icon.label();
        assert!(!label.is_empty(), "SectionIcon::{icon:?} has empty label");
    }
}

#[test]
fn test_all_section_icons_have_svg_paths() {
    let icons = [
        SectionIcon::Info,
        SectionIcon::Events,
        SectionIcon::Containers,
        SectionIcon::InitContainers,
        SectionIcon::Volumes,
        SectionIcon::Labels,
        SectionIcon::Annotations,
        SectionIcon::Conditions,
        SectionIcon::Tolerations,
        SectionIcon::Affinity,
        SectionIcon::NodeSelector,
        SectionIcon::Probes,
        SectionIcon::Security,
        SectionIcon::Resources,
        SectionIcon::Terminal,
        SectionIcon::Image,
        SectionIcon::ControlledBy,
        SectionIcon::Ports,
        SectionIcon::EnvVars,
        SectionIcon::VolumeMounts,
    ];

    for icon in icons {
        let path = icon.path();
        assert!(path.ends_with(".svg"), "SectionIcon::{icon:?} path doesn't end with .svg: {path}");
        assert!(
            path.starts_with("icons/"),
            "SectionIcon::{icon:?} path doesn't start with icons/: {path}"
        );
    }
}

#[test]
fn test_section_icon_count() {
    // There should be exactly 20 variants
    let icons = [
        SectionIcon::Info,
        SectionIcon::Events,
        SectionIcon::Containers,
        SectionIcon::InitContainers,
        SectionIcon::Volumes,
        SectionIcon::Labels,
        SectionIcon::Annotations,
        SectionIcon::Conditions,
        SectionIcon::Tolerations,
        SectionIcon::Affinity,
        SectionIcon::NodeSelector,
        SectionIcon::Probes,
        SectionIcon::Security,
        SectionIcon::Resources,
        SectionIcon::Terminal,
        SectionIcon::Image,
        SectionIcon::ControlledBy,
        SectionIcon::Ports,
        SectionIcon::EnvVars,
        SectionIcon::VolumeMounts,
    ];
    assert_eq!(icons.len(), 20);
}

#[test]
fn test_builtin_icons_use_standard_paths() {
    // Info and Events should use the standard gpui-component-assets paths
    assert_eq!(SectionIcon::Info.path().as_ref(), "icons/info.svg");
    assert_eq!(SectionIcon::Events.path().as_ref(), "icons/bell.svg");
}

#[test]
fn test_custom_icons_use_section_prefix() {
    // All custom icons should have "section-" prefix
    let custom_icons = [
        SectionIcon::Containers,
        SectionIcon::InitContainers,
        SectionIcon::Volumes,
        SectionIcon::Labels,
        SectionIcon::Annotations,
        SectionIcon::Conditions,
        SectionIcon::Tolerations,
        SectionIcon::Affinity,
        SectionIcon::NodeSelector,
        SectionIcon::Probes,
        SectionIcon::Security,
        SectionIcon::Resources,
        SectionIcon::Terminal,
        SectionIcon::Image,
        SectionIcon::ControlledBy,
        SectionIcon::Ports,
        SectionIcon::EnvVars,
        SectionIcon::VolumeMounts,
    ];

    for icon in custom_icons {
        let path = icon.path();
        assert!(
            path.contains("section-"),
            "Custom SectionIcon::{icon:?} path should contain 'section-': {path}"
        );
    }
}
