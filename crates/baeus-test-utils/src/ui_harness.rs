// GPUI test harness helpers
//
// Note: Full GPUI test harness integration requires the gpui crate dependency.
// This module provides placeholder utilities that will be expanded when GPUI
// is added to the project dependencies. For now, it provides basic test
// assertion helpers for UI-related logic that doesn't require the GPUI runtime.

pub struct TestContext {
    pub window_width: f32,
    pub window_height: f32,
}

impl Default for TestContext {
    fn default() -> Self {
        Self { window_width: 1280.0, window_height: 720.0 }
    }
}

impl TestContext {
    pub fn new(width: f32, height: f32) -> Self {
        Self { window_width: width, window_height: height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_context() {
        let ctx = TestContext::default();
        assert_eq!(ctx.window_width, 1280.0);
        assert_eq!(ctx.window_height, 720.0);
    }

    #[test]
    fn test_custom_context() {
        let ctx = TestContext::new(1920.0, 1080.0);
        assert_eq!(ctx.window_width, 1920.0);
        assert_eq!(ctx.window_height, 1080.0);
    }
}
