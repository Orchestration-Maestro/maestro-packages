//! Real browser storage, a recording fetch and platform prompts for the dialog exports.
#![cfg(all(test, target_arch = "wasm32"))]
use std::{cell::RefCell, collections::VecDeque, fmt::Write as _, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, future_to_promise};

/// One request seen by the recording fetch.
#[derive(Clone, Debug)]
pub(crate) struct Request {
    /// Requested address.
    pub(crate) url: String,
    /// Request body text.
    pub(crate) body: String,
}
/// A canned response: status, content type and body.
pub(crate) type Reply = (u16, &'static str, String);
/// The page fetch replaced by queued replies, recording each request.
pub(crate) struct Fetch {
    /// Requests in arrival order.
    pub(crate) requests: Rc<RefCell<Vec<Request>>>,
    /// The page's own fetch, restored on drop.
    original: JsValue,
    /// The installed replacement.
    _replacement: Closure<dyn FnMut(JsValue, JsValue) -> js_sys::Promise>,
}
impl Fetch {
    /// Answer requests with `replies` in order, then with network failures.
    pub(crate) fn install(replies: Vec<Reply>) -> Self {
        let window = web_sys::window().unwrap();
        let original = js_sys::Reflect::get(&window, &"fetch".into()).unwrap();
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let replies = Rc::new(RefCell::new(VecDeque::from(replies)));
        let seen = Rc::clone(&requests);
        let fetch = Closure::<dyn FnMut(JsValue, JsValue) -> js_sys::Promise>::new(
            move |input: JsValue, init: JsValue| {
                let seen = Rc::clone(&seen);
                let reply = replies.borrow_mut().pop_front();
                future_to_promise(answer(input, init, seen, reply))
            },
        );
        js_sys::Reflect::set(&window, &"fetch".into(), fetch.as_ref()).unwrap();
        Self {
            requests,
            original,
            _replacement: fetch,
        }
    }
    /// The requests seen so far.
    pub(crate) fn requests(&self) -> Vec<Request> {
        self.requests.borrow().clone()
    }
}
impl Drop for Fetch {
    fn drop(&mut self) {
        let window = web_sys::window().unwrap();
        js_sys::Reflect::set(&window, &"fetch".into(), &self.original).unwrap();
    }
}
/// Record one request and answer it with `reply`, or fail it like a refused connection.
async fn answer(
    input: JsValue,
    init: JsValue,
    seen: Rc<RefCell<Vec<Request>>>,
    reply: Option<Reply>,
) -> Result<JsValue, JsValue> {
    let request =
        js_sys::Reflect::construct(&global("Request"), &js_sys::Array::of2(&input, &init))?;
    let request = read(&request).await?;
    let url = request.url.clone();
    seen.borrow_mut().push(request);
    let Some((status, kind, body)) = reply else {
        return Err(js_sys::TypeError::new("Failed to fetch").into());
    };
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"status".into(), &status.into())?;
    let headers = js_sys::Object::new();
    js_sys::Reflect::set(&headers, &"content-type".into(), &kind.into())?;
    js_sys::Reflect::set(&options, &"headers".into(), &headers)?;
    let response = js_sys::Reflect::construct(
        &global("Response"),
        &js_sys::Array::of2(&body.into(), &options),
    )?;
    // A constructed response has no address; a fetched one reports the requested address.
    let address = js_sys::Object::new();
    js_sys::Reflect::set(&address, &"value".into(), &url.into())?;
    js_sys::Reflect::define_property(
        response.unchecked_ref::<js_sys::Object>(),
        &"url".into(),
        &address,
    )?;
    Ok(response)
}
/// A global constructor.
fn global(name: &str) -> js_sys::Function {
    js_sys::Reflect::get(&js_sys::global(), &name.into())
        .unwrap()
        .unchecked_into()
}
/// The address and body of a native request.
async fn read(request: &JsValue) -> Result<Request, JsValue> {
    let url = js_sys::Reflect::get(request, &"url".into())?
        .as_string()
        .unwrap_or_default();
    let text: js_sys::Function = js_sys::Reflect::get(request, &"text".into())?.unchecked_into();
    let body = JsFuture::from(text.call0(request)?.unchecked_into::<js_sys::Promise>())
        .await?
        .as_string()
        .unwrap_or_default();
    Ok(Request { url, body })
}
/// Platform alert and confirmation replaced by recorders; restored on drop.
pub(crate) struct Prompts {
    /// Alert and confirmation messages in order, confirmations prefixed with `?`.
    pub(crate) messages: Rc<RefCell<Vec<String>>>,
    /// The page's own alert and confirm.
    originals: [JsValue; 2],
    /// Installed replacements.
    _replacements: [Closure<dyn Fn(String) -> JsValue>; 2],
}
impl Prompts {
    /// Record alerts and answer confirmations from `answers`.
    pub(crate) fn install(answers: Vec<bool>) -> Self {
        let window = web_sys::window().unwrap();
        let messages: Rc<RefCell<Vec<String>>> = Rc::default();
        let answers = Rc::new(RefCell::new(VecDeque::from(answers)));
        let alerted = Rc::clone(&messages);
        let alert = Closure::<dyn Fn(String) -> JsValue>::new(move |message: String| {
            alerted.borrow_mut().push(message);
            JsValue::UNDEFINED
        });
        let asked = Rc::clone(&messages);
        let queue = Rc::clone(&answers);
        let confirm = Closure::<dyn Fn(String) -> JsValue>::new(move |message: String| {
            asked.borrow_mut().push(format!("?{message}"));
            queue.borrow_mut().pop_front().unwrap_or(false).into()
        });
        let originals =
            ["alert", "confirm"].map(|name| js_sys::Reflect::get(&window, &name.into()).unwrap());
        js_sys::Reflect::set(&window, &"alert".into(), alert.as_ref()).unwrap();
        js_sys::Reflect::set(&window, &"confirm".into(), confirm.as_ref()).unwrap();
        Self {
            messages,
            originals,
            _replacements: [alert, confirm],
        }
    }
    /// Messages seen so far.
    pub(crate) fn messages(&self) -> Vec<String> {
        self.messages.borrow().clone()
    }
}
impl Drop for Prompts {
    fn drop(&mut self) {
        let window = web_sys::window().unwrap();
        for (name, original) in ["alert", "confirm"].into_iter().zip(&self.originals) {
            js_sys::Reflect::set(&window, &name.into(), original).unwrap();
        }
    }
}
/// Press one key through the real keyboard.
pub(crate) async fn press(key: &str) {
    crate::contributions_support::driver("driverPress", key).await;
}
/// The open dialogs' panels in mounting order.
pub(crate) fn panels() -> Vec<web_sys::Element> {
    let nodes = crate::contributions_support::document()
        .query_selector_all(".maestro-dialog")
        .unwrap();
    (0..nodes.length())
        .map(|index| nodes.item(index).unwrap().unchecked_into())
        .collect()
}
/// The one open dialog's panel.
pub(crate) fn panel() -> web_sys::Element {
    let mut open = panels();
    assert_eq!(open.len(), 1, "exactly one dialog is open");
    open.remove(0)
}
/// Wait, by mutation signals on the body, until `ready` holds.
pub(crate) async fn until(ready: impl Fn() -> bool) {
    let body = crate::contributions_support::body();
    crate::contributions_support::until(&body, ready).await;
}
/// The anthropic event stream of one completed `ok` reply.
pub(crate) fn completed_reply() -> String {
    [
        r#"{"type":"message_start","message":{"id":"m","usage":{"input_tokens":3,"output_tokens":0}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ok"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#,
        r#"{"type":"message_stop"}"#,
    ]
    .iter()
    .fold(String::new(), |mut stream, data| {
        let kind: serde_json::Value = serde_json::from_str(data).unwrap();
        let _ = write!(stream, "event: {}\ndata: {data}\n\n", kind["type"].as_str().unwrap());
        stream
    })
}
