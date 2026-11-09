//! Page operands, the fixture package and browser observations shared by contribution exports.
#![cfg(all(test, target_arch = "wasm32"))]
use maestro_app::{SandboxUrlProvider, SharedHandle};
use maestro_web::{I18n, contributions::ContributionHost, dom::DomView};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlElement, Node};

/// Browser observations read by the contribution exports.
pub(crate) mod page;

/// The page document.
pub(crate) fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}
/// The page body.
pub(crate) fn body() -> HtmlElement {
    document().body().unwrap()
}
/// The page origin.
pub(crate) fn origin() -> String {
    web_sys::window().unwrap().location().origin().unwrap()
}
/// Page operands with the shared brand pack and syntax engine; panics report their message.
pub(crate) fn view() -> Rc<DomView> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    Rc::new(DomView {
        document: document(),
        i18n: Rc::new(I18n::new(Rc::new(crate::support::Host::default())).unwrap()),
        highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        pack: Rc::new(crate::brand::pack("brand.json").unwrap()),
        mode: maestro_theme::BrandMode::Dark,
        tool_renderers: Rc::default(),
    })
}
/// The isolated-frame document served beside the page, which carries no page policy.
pub(crate) fn sandbox_url() -> SandboxUrlProvider {
    SharedHandle::new(|| Ok(format!("{}/sandbox.html", origin())))
}
/// A host for `view` that imports modules through the page's `import_module`.
pub(crate) fn host(view: &Rc<DomView>, import_module: &js_sys::Function) -> ContributionHost {
    ContributionHost::new(view, Some(sandbox_url()), import_module.clone()).unwrap()
}
/// Await a promise, or take a plain value as settled.
pub(crate) async fn settle(value: JsValue) -> Result<JsValue, JsValue> {
    JsFuture::from(js_sys::Promise::resolve(&value)).await
}
/// Wait, by mutation signals, until `ready` holds below `target`.
pub(crate) async fn until(target: &Node, ready: impl Fn() -> bool) {
    while !ready() {
        page::changed(target).await;
    }
}
/// Call a driver binding the harness exposes on the page.
pub(crate) async fn driver(name: &str, argument: &str) -> JsValue {
    let window = web_sys::window().unwrap();
    let function: js_sys::Function = js_sys::Reflect::get(&window, &name.into())
        .unwrap()
        .dyn_into()
        .unwrap();
    settle(function.call1(&window, &argument.into()).unwrap())
        .await
        .unwrap()
}
/// Text below a node.
pub(crate) fn text(node: &Node) -> String {
    node.text_content().unwrap_or_default()
}
/// A new connected container.
pub(crate) fn container() -> Element {
    let element = document().create_element("section").unwrap();
    body().append_child(&element).unwrap();
    element
}

/// One required descendant, reporting the missing selector.
pub(crate) fn select(node: &Node, selector: &str) -> Element {
    node.unchecked_ref::<Element>()
        .query_selector(selector)
        .unwrap()
        .unwrap_or_else(|| panic!("missing {selector}"))
}
/// Whether `element` holds the page focus.
pub(crate) fn focused(element: &Element) -> bool {
    document()
        .active_element()
        .is_some_and(|active| active.is_same_node(Some(element)))
}
