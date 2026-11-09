//! Page hooks and dynamic page calls shared by browser witnesses.
#![cfg(target_arch = "wasm32")]
use std::any::Any;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

/// A replaced page property, restored to its own previous definition when dropped.
pub(crate) struct Patch {
    /// The object that owns the replaced property.
    target: js_sys::Object,
    /// The replaced property name.
    key: JsValue,
    /// The target's own descriptor before the patch, or undefined when it was inherited.
    previous: JsValue,
    /// The callbacks the replacement calls, kept alive while it is installed.
    _hooks: Vec<Box<dyn Any>>,
}

/// A replacement method: the original function, the receiver and the arguments.
type Hook = dyn Fn(js_sys::Function, JsValue, js_sys::Array) -> Result<JsValue, JsValue>;

impl Patch {
    /// Replace the method `name` of `target` with `hook`, assigned as the page itself would.
    pub(crate) fn method(
        target: &JsValue,
        name: &str,
        hook: impl Fn(js_sys::Function, JsValue, js_sys::Array) -> Result<JsValue, JsValue> + 'static,
    ) -> Self {
        let (target, key, previous) = own(target, name);
        let original = js_sys::Reflect::get(&target, &key).unwrap();
        let apply = Closure::<Hook>::new(hook);
        let handler = object(&[("apply", apply.as_ref())]);
        let proxy = js_sys::Proxy::new(original.unchecked_ref(), &handler);
        assert!(
            js_sys::Reflect::set(&target, &key, &proxy).unwrap(),
            "the page refused the {name} replacement"
        );
        Self {
            target,
            key,
            previous,
            _hooks: vec![Box::new(apply)],
        }
    }
}

impl Drop for Patch {
    fn drop(&mut self) {
        if self.previous.is_undefined() {
            js_sys::Reflect::delete_property(&self.target, &self.key).unwrap();
        } else {
            js_sys::Reflect::define_property(
                &self.target,
                &self.key,
                self.previous.unchecked_ref(),
            )
            .unwrap();
        }
    }
}

/// A page promise and the function that settles it.
pub(crate) fn signal() -> (js_sys::Promise, js_sys::Function) {
    let mut settle = None;
    let promise = js_sys::Promise::new(&mut |resolve, _| settle = Some(resolve));
    (promise, settle.unwrap())
}

/// The target object, its property key and its own descriptor before any change.
fn own(target: &JsValue, name: &str) -> (js_sys::Object, JsValue, JsValue) {
    let target: js_sys::Object = target.clone().unchecked_into();
    let key = JsValue::from(name);
    let previous = js_sys::Reflect::get_own_property_descriptor(&target, &key).unwrap();
    (target, key, previous)
}

/// The page global `name`.
pub(crate) fn global(name: &str) -> JsValue {
    get(&js_sys::global(), name)
}

/// The property `key` of a page value.
pub(crate) fn get(target: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(target, &key.into()).unwrap()
}

/// Set the property `key` of a page value.
pub(crate) fn set(target: &JsValue, key: &str, value: &JsValue) {
    js_sys::Reflect::set(target, &key.into(), value).unwrap();
}

/// Call the method `name` of `target` with `args`, returning what it returns or throws.
pub(crate) fn invoke(target: &JsValue, name: &str, args: &[&JsValue]) -> Result<JsValue, JsValue> {
    let method: js_sys::Function = get(target, name).dyn_into()?;
    method.apply(target, &args.iter().copied().collect::<js_sys::Array>())
}

/// Construct the page class `name` with `args`.
pub(crate) fn construct(name: &str, args: &[&JsValue]) -> JsValue {
    js_sys::Reflect::construct(
        global(name).unchecked_ref::<js_sys::Function>(),
        &args.iter().copied().collect::<js_sys::Array>(),
    )
    .unwrap()
}

/// A plain page object with `entries`.
pub(crate) fn object(entries: &[(&str, &JsValue)]) -> js_sys::Object {
    let object = js_sys::Object::new();
    for (key, value) in entries {
        set(&object, key, value);
    }
    object
}

/// A page `DOMException` carrying `message` and `name`.
pub(crate) fn dom_exception(message: &str, name: &str) -> JsValue {
    web_sys::DomException::new_with_message_and_name(message, name)
        .unwrap()
        .into()
}
