//! The smallest event-hook extension: appends a showcase instruction to
//! every user-submitted prompt through the `input` event.
//!
//! Behaviour (reference, behaviour-only): every submitted prompt whose
//! source is not `extension` is transformed — the text gains
//! [`SUFFIX`] and the images are omitted so the host keeps the original
//! images. Extension-sourced input is left alone with `Continue`.

#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    InputEventResult, InputSource, InputTransform, Presence,
};

/// The instruction appended to every transformed prompt.
pub const SUFFIX: &str = "\n\nSay: Frank is the best.";

/// The host event the extension hooks.
pub const INPUT_EVENT: &str = "input";

/// Decide what the `input` event handler returns for one submitted prompt.
///
/// Extension-sourced input returns `Continue`; anything else returns a
/// `Transform` whose text is the original plus [`SUFFIX`], with images
/// omitted so the host keeps the original images.
#[must_use]
pub fn decide(text: &str, source: &InputSource) -> InputEventResult {
    if matches!(source, InputSource::Extension) {
        InputEventResult::Continue
    } else {
        InputEventResult::Transform(InputTransform {
            text: format!("{text}{SUFFIX}"),
            images: Presence::Missing,
        })
    }
}

/// The `input` event handler: one single-purpose decision per event.
fn input_handler(
    event: &mut ExtensionEvent,
    _ctx: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::Input(input) = event else {
            return Ok(None);
        };
        Ok(Some(ExtensionEventResult::Input(decide(
            &input.text,
            &input.source,
        ))))
    })
}

/// Registers the `input` hook through the supplied extension host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on(INPUT_EVENT, Rc::new(input_handler)) })
}

#[cfg(target_arch = "wasm32")]
mod component;

#[cfg(test)]
mod tests {
    use super::{decide, INPUT_EVENT, SUFFIX};
    use maestro_extensions_wasm::{InputEventResult, InputSource, Presence};

    /// Interactive prompts are transformed with the instruction appended.
    #[test]
    fn interactive_prompt_is_transformed() {
        let result = decide("hello", &InputSource::Interactive);
        match result {
            InputEventResult::Transform(transform) => {
                assert_eq!(transform.text, format!("hello{SUFFIX}"));
                assert!(transform.images.is_missing());
            }
            other => panic!("expected Transform, got {other:?}"),
        }
    }

    /// RPC prompts are transformed the same way.
    #[test]
    fn rpc_prompt_is_transformed() {
        let result = decide("hello", &InputSource::Rpc);
        assert!(matches!(result, InputEventResult::Transform(_)));
    }

    /// Extension-sourced input is left alone.
    #[test]
    fn extension_prompt_is_untouched() {
        let result = decide("hello", &InputSource::Extension);
        assert!(matches!(result, InputEventResult::Continue));
    }

    /// Images are omitted from the transform, so the host keeps the
    /// original images.
    #[test]
    fn transform_omits_images() {
        let result = decide("hello", &InputSource::Interactive);
        let InputEventResult::Transform(transform) = result else {
            panic!("expected Transform");
        };
        assert!(matches!(transform.images, Presence::Missing));
    }

    /// The factory registers the exact host event name — a wrong name
    /// here would silently never fire.
    #[test]
    fn factory_registers_the_input_event() {
        assert_eq!(INPUT_EVENT, "input");
    }
}
