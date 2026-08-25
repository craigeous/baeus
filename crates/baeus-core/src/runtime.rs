// T025: Shared Tokio runtime handle for use across crates.
//
// The GPUI event loop owns the main thread. Tokio runs on a background
// thread pool. This wrapper lets any GPUI view spawn async work on the
// Tokio runtime by retrieving it from the GPUI global context.

/// Global wrapper for the Tokio runtime handle, accessible from any GPUI context.
///
/// Stored via `cx.set_global(TokioHandle(handle))` at application startup and
/// retrieved in views via `cx.global::<TokioHandle>()`.
#[derive(Clone)]
pub struct TokioHandle(pub tokio::runtime::Handle);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokio_handle_clone() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let handle = TokioHandle(rt.handle().clone());
        let _cloned = handle.clone();
        // Just verify Clone works without panicking
    }
}
