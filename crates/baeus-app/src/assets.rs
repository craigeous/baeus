use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// Embedded custom assets for Baeus (section icons, etc.).
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
pub struct BaeusCustomAssets;

/// Combined asset source: tries gpui-component-assets first, falls back to Baeus custom assets.
pub struct BaeusAssets;

impl AssetSource for BaeusAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        // Try gpui-component-assets first (built-in Lucide icons).
        if let Ok(Some(data)) = gpui_component_assets::Assets.load(path) {
            return Ok(Some(data));
        }

        // Fall back to Baeus custom assets.
        BaeusCustomAssets::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow::anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut items: Vec<SharedString> =
            gpui_component_assets::Assets.list(path).unwrap_or_default();

        for entry in BaeusCustomAssets::iter() {
            if entry.starts_with(path) {
                let s: SharedString = entry.into();
                if !items.contains(&s) {
                    items.push(s);
                }
            }
        }

        Ok(items)
    }
}
