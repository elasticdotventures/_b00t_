//! WASM-compatible async sleep using `setTimeout` + `Promise`.
//!
//! `tokio::time::sleep` is unavailable in WASM targets, so we
//! implement our own via `js_sys::Promise` + `web_sys::Window`.

use std::time::Duration;

/// Sleep for the given duration.  Works in WASM (uses `setTimeout`
/// under the hood) and in native targets alike.
pub async fn sleep(duration: Duration) {
    let ms = duration.as_millis();
    // Clamp to u32 (about 49 days — more than enough for a UI poll)
    let ms = ms.min(u128::from(u32::MAX)) as u32;

    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let window = web_sys::window().expect("window not available");
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32);
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .expect("setTimeout sleep failed");
}
