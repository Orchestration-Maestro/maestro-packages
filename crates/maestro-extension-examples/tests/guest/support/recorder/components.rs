//! Showing the custom components an extension asks for: run to completion on scripted input, or
//! kept open for the test to drive.
use super::{Recorder, keybindings};
use crate::guest::support::rows::plain_rows;
use maestro_extensions::{
    ComponentHandle, EventError,
    types::{
        CustomClose, CustomComponentFactory, CustomFactoryOutcome, ExtensionComponent,
        ExtensionUIFuture,
    },
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

impl Recorder {
    /// Gives the next custom component these inputs, in order, and runs it to completion.
    ///
    /// Without a script the component stays open for the test to drive with [`Recorder::press`].
    pub fn script(&self, inputs: &[&str]) {
        let inputs = inputs.iter().map(|input| (*input).to_owned()).collect();
        *self.views.script.borrow_mut() = Some(inputs);
    }

    /// Sets the width that scripted components are drawn at.
    pub fn frame_width(&self, width: usize) {
        self.views.width.set(width);
    }

    /// The rows of each frame the last scripted component drew: before the first input and
    /// after each one, with their control sequences.
    pub fn frames(&self) -> Vec<Vec<String>> {
        self.views.frames.take()
    }

    /// Waits until `count` custom components have been shown.
    pub async fn shown(&self, count: usize) {
        while self.views.shown.borrow().len() < count {
            self.views.opened.notified().await;
        }
    }

    /// How many custom components have been shown.
    pub fn opened(&self) -> usize {
        self.views.shown.borrow().len()
    }

    /// The `index`th custom component shown.
    pub fn component(&self, index: usize) -> ComponentHandle {
        self.views.shown.borrow()[index].component.clone()
    }

    /// The `index`th custom component's rows at `width`, without control sequences.
    pub fn rows(&self, index: usize, width: usize) -> Vec<String> {
        plain_rows(&self.component(index).try_render(width).unwrap())
    }

    /// Delivers terminal input to the `index`th custom component.
    pub fn press(&self, index: usize, data: &str) {
        self.component(index)
            .input_handler()
            .expect("the component takes input")
            .handle_input(data);
    }

    /// Delivers terminal input to the `index`th custom component and returns the failure its
    /// handler reported.
    pub fn try_press(&self, index: usize, data: &str) -> Result<(), String> {
        self.component(index)
            .input_handler()
            .expect("the component takes input")
            .try_handle_input(data)
            .map_err(|error| error.to_string())
    }

    /// Builds the component, then either runs the scripted input or waits for it to close.
    pub(super) fn show(&self, factory: &CustomComponentFactory) -> ExtensionUIFuture<()> {
        let Some(stage) = &self.stage else {
            return Box::pin(std::future::ready(Ok(())));
        };
        if let Err(error) = self.record("custom", json!([])) {
            return Box::pin(std::future::ready(Err(error)));
        }
        let closed = Rc::new(Cell::new(false));
        let flag = closed.clone();
        let (settle, settled) = tokio::sync::oneshot::channel();
        let settle = RefCell::new(Some(settle));
        let kept: Rc<RefCell<Option<Rc<ExtensionComponent>>>> = Rc::default();
        let closing = kept.clone();
        let close: CustomClose = Rc::new(move || {
            if flag.replace(true) {
                return Ok(false);
            }
            if let Some(settle) = settle.borrow_mut().take() {
                let _ = settle.send(());
            }
            let component = closing.borrow_mut().take();
            if let Some(dispose) = component.and_then(|component| component.dispose.clone()) {
                dispose.call(())?;
            }
            Ok(true)
        });
        let built = factory.call((stage.tui.clone(), self.theme.clone(), keybindings(), close));
        match self.views.script.take() {
            Some(inputs) => Box::pin(drive(
                built,
                inputs,
                closed,
                (self.views.frames.clone(), self.views.width.get()),
            )),
            None => self.keep(built, &kept, settled),
        }
    }

    /// Keeps a built component open until the extension closes it.
    fn keep(
        &self,
        built: Result<CustomFactoryOutcome, EventError>,
        kept: &Rc<RefCell<Option<Rc<ExtensionComponent>>>>,
        settled: tokio::sync::oneshot::Receiver<()>,
    ) -> ExtensionUIFuture<()> {
        match built {
            Ok(CustomFactoryOutcome::Ready(component)) => {
                let component = Rc::new(component);
                kept.replace(Some(component.clone()));
                self.views.shown.borrow_mut().push(component);
                self.views.opened.notify_one();
                Box::pin(async move {
                    settled
                        .await
                        .map_err(|error| EventError::from(error.to_string()))
                })
            }
            Ok(CustomFactoryOutcome::Async(_)) => Box::pin(std::future::ready(Err(
                "an asynchronous view is not shown".into(),
            ))),
            Err(error) => Box::pin(std::future::ready(Err(error))),
        }
    }
}

/// Opens the built custom component, draws it before and after each input and disposes it.
async fn drive(
    built: Result<CustomFactoryOutcome, EventError>,
    inputs: Vec<String>,
    closed: Rc<Cell<bool>>,
    (frames, width): (Rc<RefCell<Vec<Vec<String>>>>, usize),
) -> Result<(), EventError> {
    let component = match built? {
        CustomFactoryOutcome::Ready(component) => component,
        CustomFactoryOutcome::Async(pending) => pending.await?,
    };
    let handle = component.component.clone();
    let draw = || handle.try_render(width).map_err(|error| error.to_string());
    frames.borrow_mut().push(draw()?);
    for input in inputs.iter().take_while(|_| !closed.get()) {
        if let Some(handler) = handle.input_handler() {
            handler
                .try_handle_input(input)
                .map_err(|error| error.to_string())?;
        }
        frames.borrow_mut().push(draw()?);
    }
    component.dispose.map_or(Ok(()), |dispose| dispose.call(()))
}
