use crate::{pipeline::VideoPrimitive, video::Video};
use gstreamer as gst;
pub use iced::advanced::mouse::{Button, ScrollDelta, click::Kind};
#[allow(unused_imports)]
pub use iced::keyboard::{Key, Modifiers, key};
use iced::{
    Element, Event, Point,
    advanced::{
        self, Widget, layout, mouse,
        widget::{self, tree},
    },
    keyboard, window,
};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;
use log::error;
use std::{f32, marker::PhantomData, sync::atomic::Ordering};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Video player widget which displays the current frame of a [`Video`](crate::Video).
pub struct VideoPlayer<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: PrimitiveRenderer,
{
    video: &'a Video,
    content_fit: iced::ContentFit,
    width: iced::Length,
    height: iced::Length,
    on_end_of_stream: Option<Message>,
    on_new_frame: Option<Message>,
    on_subtitle_text: Option<Box<dyn Fn(Option<String>) -> Message + 'a>>,
    on_error: Option<Box<dyn Fn(&glib::Error) -> Message + 'a>>,
    on_keypress: Option<Box<dyn Fn(KeyPress) -> Option<Message> + 'a>>,
    on_click: Option<Box<dyn Fn(MouseClick) -> Option<Message> + 'a>>,
    _phantom: PhantomData<(Theme, Renderer)>,
}

impl<'a, Message, Theme, Renderer> VideoPlayer<'a, Message, Theme, Renderer>
where
    Renderer: PrimitiveRenderer,
{
    /// Creates a new video player widget for a given video.
    pub fn new(video: &'a Video) -> Self {
        VideoPlayer {
            video,
            content_fit: iced::ContentFit::default(),
            width: iced::Length::Shrink,
            height: iced::Length::Shrink,
            on_end_of_stream: None,
            on_new_frame: None,
            on_subtitle_text: None,
            on_error: None,
            on_keypress: None,
            on_click: None,
            _phantom: Default::default(),
        }
    }

    /// Sets the width of the `VideoPlayer` boundaries.
    pub fn width(self, width: impl Into<iced::Length>) -> Self {
        VideoPlayer {
            width: width.into(),
            ..self
        }
    }

    /// Sets the height of the `VideoPlayer` boundaries.
    pub fn height(self, height: impl Into<iced::Length>) -> Self {
        VideoPlayer {
            height: height.into(),
            ..self
        }
    }

    /// Sets the `ContentFit` of the `VideoPlayer`.
    pub fn content_fit(self, content_fit: iced::ContentFit) -> Self {
        VideoPlayer {
            content_fit,
            ..self
        }
    }

    /// Message to send when the video reaches the end of stream (i.e., the video ends).
    pub fn on_end_of_stream(self, on_end_of_stream: Message) -> Self {
        VideoPlayer {
            on_end_of_stream: Some(on_end_of_stream),
            ..self
        }
    }

    /// Message to send when the video receives a new frame.
    pub fn on_new_frame(self, on_new_frame: Message) -> Self {
        VideoPlayer {
            on_new_frame: Some(on_new_frame),
            ..self
        }
    }

    /// Message to send when the video receives a new frame.
    pub fn on_subtitle_text<F>(self, on_subtitle_text: F) -> Self
    where
        F: 'a + Fn(Option<String>) -> Message,
    {
        VideoPlayer {
            on_subtitle_text: Some(Box::new(on_subtitle_text)),

            ..self
        }
    }

    /// Message to send when the video playback encounters an error.
    pub fn on_error<F>(self, on_error: F) -> Self
    where
        F: 'a + Fn(&glib::Error) -> Message,
    {
        VideoPlayer {
            on_error: Some(Box::new(on_error)),
            ..self
        }
    }

    /// Sets the message produced when a [`KeyPress`] is received.
    pub fn on_keypress<F>(self, on_keypress: F) -> Self
    where
        F: 'a + Fn(KeyPress) -> Option<Message>,
    {
        VideoPlayer {
            on_keypress: Some(Box::new(on_keypress)),
            ..self
        }
    }

    /// Sets the message produced when a [`MouseClick`] is received.
    pub fn on_click<F>(self, on_click: F) -> Self
    where
        F: 'a + Fn(MouseClick) -> Option<Message>,
    {
        VideoPlayer {
            on_click: Some(Box::new(on_click)),
            ..self
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VideoPlayer<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: PrimitiveRenderer,
{
    fn size(&self) -> iced::Size<iced::Length> {
        iced::Size {
            width: iced::Length::Shrink,
            height: iced::Length::Shrink,
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let (video_width, video_height) = self.video.size();

        // based on `Image::layout`
        let image_size = iced::Size::new(video_width as f32, video_height as f32);
        let raw_size = limits.resolve(self.width, self.height, image_size);
        let full_size = self.content_fit.fit(image_size, raw_size);
        let final_size = iced::Size {
            width: match self.width {
                iced::Length::Shrink => f32::min(raw_size.width, full_size.width),
                _ => raw_size.width,
            },
            height: match self.height {
                iced::Length::Shrink => f32::min(raw_size.height, full_size.height),
                _ => raw_size.height,
            },
        };

        layout::Node::new(final_size)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &advanced::renderer::Style,
        layout: advanced::Layout<'_>,
        _cursor: advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        let mut inner = self.video.write();

        // bounds based on `Image::draw`
        let image_size = iced::Size::new(inner.width as f32, inner.height as f32);
        let bounds = layout.bounds();
        let adjusted_fit = self.content_fit.fit(image_size, bounds.size());
        let scale = iced::Vector::new(
            adjusted_fit.width / image_size.width,
            adjusted_fit.height / image_size.height,
        );
        let final_size = image_size * scale;

        let position = match self.content_fit {
            iced::ContentFit::None => iced::Point::new(
                bounds.x + (image_size.width - adjusted_fit.width) / 2.0,
                bounds.y + (image_size.height - adjusted_fit.height) / 2.0,
            ),
            _ => iced::Point::new(
                bounds.center_x() - final_size.width / 2.0,
                bounds.center_y() - final_size.height / 2.0,
            ),
        };

        let drawing_bounds = iced::Rectangle::new(position, final_size);

        let upload_frame = inner.upload_frame.swap(false, Ordering::SeqCst);

        if upload_frame {
            let last_frame_time = inner
                .last_frame_time
                .lock()
                .map(|time| *time)
                .unwrap_or_else(|_| Instant::now());
            inner.set_av_offset(Instant::now() - last_frame_time);
        }

        let render = |renderer: &mut Renderer| {
            renderer.draw_primitive(
                drawing_bounds,
                VideoPrimitive::new(
                    inner.id,
                    Arc::clone(&inner.alive),
                    Arc::clone(&inner.frame),
                    (inner.width as _, inner.height as _),
                    upload_frame,
                ),
            );
        };

        if adjusted_fit.width > bounds.width || adjusted_fit.height > bounds.height {
            renderer.with_layer(bounds, render);
        } else {
            render(renderer);
        }
    }

    fn update(
        &mut self,
        state: &mut widget::Tree,
        event: &iced::Event,
        layout: advanced::Layout<'_>,
        cursor: advanced::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn advanced::Clipboard,
        shell: &mut advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(new)) => {
                let state = state.state.downcast_mut::<State>();
                state.modifiers = *new;
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(on_keypress) = &self.on_keypress {
                    let keypress = KeyPress {
                        key: key.clone(),
                        modifiers: *modifiers,
                    };
                    if let Some(message) = (on_keypress)(keypress) {
                        shell.publish(message);
                        shell.capture_event();
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(button))
                if cursor.is_over(layout.bounds()) =>
            {
                let state = state.state.downcast_mut::<State>();
                let click = mouse::Click::new(
                    cursor.position_over(layout.bounds()).unwrap(),
                    *button,
                    state.last_click,
                );

                if let Some(on_click) = &self.on_click {
                    let action = MouseAction::Button {
                        button: *button,
                        kind: click.kind(),
                    };
                    let mouse_click = MouseClick {
                        modifiers: state.modifiers,
                        action,
                    };
                    if let Some(message) = (on_click)(mouse_click) {
                        shell.publish(message);
                        shell.capture_event();
                    }
                }

                state.last_click = Some(click);
                state.last_update = Some(Update {
                    time: Instant::now(),
                    parent: Some(click.position()),
                })
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(layout.bounds()) =>
            {
                let state = state.state.downcast_mut::<State>();
                if let Some(on_click) = &self.on_click {
                    let action = MouseAction::Scroll(*delta);

                    let mouse_click = MouseClick {
                        modifiers: state.modifiers,
                        action,
                    };

                    if let Some(message) = (on_click)(mouse_click) {
                        shell.publish(message);
                        shell.capture_event();
                    }
                }

                state.last_update = Some(Update {
                    time: Instant::now(),
                    parent: cursor.position_over(layout.bounds()),
                })
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Mouse(mouse::Event::CursorLeft)
            | Event::Mouse(mouse::Event::CursorEntered) => {
                let state = state.state.downcast_mut::<State>();
                state.last_update = match state.last_update {
                    Some(Update { time, .. }) => Some(Update {
                        time,
                        parent: cursor.position_over(layout.bounds()),
                    }),
                    None => {
                        if cursor.is_over(layout.bounds()) {
                            Some(Update {
                                time: Instant::now(),
                                parent: cursor.position_over(layout.bounds()),
                            })
                        } else {
                            None
                        }
                    }
                };
            }
            Event::Window(window::Event::RedrawRequested(_)) => {
                let mut inner = self.video.write();
                if inner.restart_stream || (!inner.is_eos && !inner.paused()) {
                    let mut restart_stream = false;
                    if inner.restart_stream {
                        restart_stream = true;
                        // Set flag to false to avoid potentially multiple seeks
                        inner.restart_stream = false;
                    }
                    let mut eos_pause = false;

                    while let Some(msg) = inner
                        .bus
                        .pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])
                    {
                        match msg.view() {
                            gst::MessageView::Error(err) => {
                                error!("bus returned an error: {err}");
                                if let Some(ref on_error) = self.on_error {
                                    shell.publish(on_error(&err.error()))
                                };
                            }
                            gst::MessageView::Eos(_eos) => {
                                if let Some(on_end_of_stream) = self.on_end_of_stream.clone() {
                                    shell.publish(on_end_of_stream);
                                }
                                if inner.looping {
                                    restart_stream = true;
                                } else {
                                    eos_pause = true;
                                }
                            }
                            _ => {}
                        }
                    }

                    // Don't run eos_pause if restart_stream is true; fixes "pausing" after restarting a stream
                    if restart_stream {
                        if let Err(err) = inner.restart_stream() {
                            error!("cannot restart stream (can't seek): {err:#?}");
                        }
                    } else if eos_pause {
                        inner.is_eos = true;
                        inner.set_paused(true);
                    }

                    if inner.upload_frame.load(Ordering::SeqCst) {
                        if let Some(on_new_frame) = self.on_new_frame.clone() {
                            shell.publish(on_new_frame);
                        }
                    }

                    if let Some(on_subtitle_text) = &self.on_subtitle_text {
                        if inner.upload_text.swap(false, Ordering::SeqCst) {
                            if let Ok(text) = inner.subtitle_text.try_lock() {
                                shell.publish(on_subtitle_text(text.clone()));
                            }
                        }
                    }

                    shell.request_redraw_at(iced::window::RedrawRequest::NextFrame);
                } else {
                    shell.request_redraw_at(iced::window::RedrawRequest::At(
                        Instant::now() + Duration::from_millis(32),
                    ));
                }

                let state = state.state.downcast_mut::<State>();
                match state.last_update.take() {
                    Some(Update {
                        parent: position,
                        time,
                    }) if position.is_some() => {
                        if cursor.position_over(layout.bounds()) == position
                            && Instant::now().duration_since(time).as_secs() >= 3
                        {
                        } else {
                            state.last_update = Some(Update {
                                time,
                                parent: position,
                            })
                        }
                    }
                    Some(Update { parent: None, .. }) => {}
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        state: &tree::Tree,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &iced::Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::default();
        }

        let state = state.state.downcast_ref::<State>();

        if state.last_update.is_some() {
            mouse::Interaction::default()
        } else {
            mouse::Interaction::Hidden
        }
    }
}

impl<'a, Message, Theme, Renderer> From<VideoPlayer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + PrimitiveRenderer,
{
    fn from(video_player: VideoPlayer<'a, Message, Theme, Renderer>) -> Self {
        Self::new(video_player)
    }
}

pub(crate) struct State {
    last_click: Option<mouse::Click>,
    modifiers: keyboard::Modifiers,
    pub(crate) last_update: Option<Update>,
}

impl State {
    fn new() -> Self {
        Self {
            modifiers: keyboard::Modifiers::default(),
            last_click: None,
            last_update: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Update {
    pub time: Instant,
    pub parent: Option<Point>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// A mouse click.
pub struct MouseClick {
    /// The state of keyboard modifiers.
    pub modifiers: Modifiers,
    /// A mouse action
    pub action: MouseAction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseAction {
    Button {
        /// The mouse button clicked.
        button: Button,
        /// The kind of mouse click.
        kind: Kind,
    },
    Scroll(ScrollDelta),
}

#[derive(Debug, Clone, PartialEq)]
/// A key press.
pub struct KeyPress {
    /// The key pressed.
    pub key: Key,
    // The state of keyboard modifiers.
    pub modifiers: Modifiers,
}
