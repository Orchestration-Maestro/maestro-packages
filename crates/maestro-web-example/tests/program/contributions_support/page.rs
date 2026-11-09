//! Browser observations read by the contribution exports, which run without eval.
use super::settle;
use crate::browser_fixture_support::{construct, invoke, object};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::Node;

/// Mutation kinds a page observer watches below its target.
fn watched(old_values: bool) -> js_sys::Object {
    let on = JsValue::TRUE;
    let old = JsValue::from(old_values);
    object(&[
        ("subtree", &on),
        ("childList", &on),
        ("characterData", &on),
        ("attributes", &on),
        ("attributeOldValue", &old),
        ("characterDataOldValue", &old),
    ])
}

/// Settle at the next change below `target`.
pub(crate) async fn changed(target: &Node) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let callback = Closure::once_into_js(move |_: JsValue, observer: JsValue| {
            invoke(&observer, "disconnect", &[]).unwrap();
            resolve.call0(&JsValue::NULL).unwrap();
        });
        let observer = construct("MutationObserver", &[&callback]);
        invoke(&observer, "observe", &[target, &watched(false)]).unwrap();
    });
    settle(promise.into()).await.unwrap();
}

/// The time of the next animation frame, in milliseconds.
async fn frame() -> f64 {
    let window = web_sys::window().unwrap();
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        window.request_animation_frame(&resolve).unwrap();
    });
    settle(promise.into()).await.unwrap().as_f64().unwrap()
}

/// Intervals between the next `count` animation frames, in milliseconds.
pub(crate) async fn frames(count: u32) -> Vec<f64> {
    let mut previous = frame().await;
    let mut intervals = Vec::new();
    for _ in 0..count {
        let time = frame().await;
        intervals.push(time - previous);
        previous = time;
    }
    intervals
}
