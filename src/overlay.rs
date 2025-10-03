use iced::{
    advanced::{
        self,
        layout::{self, Node},
        overlay,
        renderer::Quad,
        text::{self, paragraph::Plain, Text},
    },
    alignment, color, mouse, Border, Color, Event, Pixels, Point, Rectangle, Size,
};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;

use crate::Video;

const SPEED_SIZE_MULT: f32 = 0.75;

/// A default overlay. It does not draw anything.
pub struct DefaultOverlay;

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer> for DefaultOverlay
where
    Renderer: advanced::Renderer,
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        layout::Node::new(Size::ZERO)
    }

    fn draw(
        &self,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &advanced::renderer::Style,
        _layout: layout::Layout<'_>,
        _cursor: advanced::mouse::Cursor,
    ) {
    }
}

#[derive(Debug, Clone, Copy)]
/// An icon for the overlay on the [`VideoOverlay`].
pub struct Icon<Message, Font> {
    /// The font that will be used to display the `code_point`.
    pub font: Font,
    /// The unicode code point that will be used as the icon.
    pub code_point: char,
    /// The font size of the content.
    pub size: Option<Pixels>,
    /// The font color of the content.
    pub color: Option<Color>,
    /// The message produced by the icon, if any.
    pub message: Option<Message>,
}

/// An overlay for [`crate::VideoPlayer`].
pub struct VideoOverlay<Message, Renderer = iced::Renderer>
where
    Renderer: text::Renderer,
{
    bounds: Rectangle,
    speed: f64,
    /// The 'play/paused' icon used.
    pub play: Option<Icon<Message, Renderer::Font>>,
    /// The 'fullscreen' icon used.
    pub fullscreen: Option<Icon<Message, Renderer::Font>>,
    /// The 'captions' icon used.
    pub captions: Option<Icon<Message, Renderer::Font>>,
    /// The 'previous' icon used.
    pub previous: Option<Icon<Message, Renderer::Font>>,
    /// The 'next' icon used.
    pub next: Option<Icon<Message, Renderer::Font>>,
}

impl<Message, Renderer> VideoOverlay<Message, Renderer>
where
    Message: Clone,
    Renderer: PrimitiveRenderer + text::Renderer,
{
    /// Creates a new [`VideoOverlay`] with the given video and bounds
    pub fn new(video: &Video, bounds: Rectangle) -> Self {
        Self {
            bounds,
            speed: video.speed(),
            play: None,
            fullscreen: None,
            captions: None,
            previous: None,
            next: None,
        }
    }

    /// Sets the [`Icon`] used for the play/pause/restart
    /// overlay.
    pub fn play_icon(self, icon: Icon<Message, Renderer::Font>) -> Self {
        Self {
            play: Some(icon),
            ..self
        }
    }

    /// Sets the [`Icon`] used for the next overlay.
    pub fn next_icon(self, icon: Icon<Message, Renderer::Font>) -> Self {
        Self {
            next: Some(icon),
            ..self
        }
    }

    /// Sets the [`Icon`] used for the previous overlay.
    pub fn previous_icon(self, icon: Icon<Message, Renderer::Font>) -> Self {
        Self {
            previous: Some(icon),
            ..self
        }
    }

    /// Sets the [`Icon`] used for the fullscreen overlay.
    pub fn fullscreen_icon(self, icon: Icon<Message, Renderer::Font>) -> Self {
        Self {
            fullscreen: Some(icon),
            ..self
        }
    }

    /// Sets the [`Icon`] used for the captions overlay.
    pub fn subtitles_icon(self, icon: Icon<Message, Renderer::Font>) -> Self {
        Self {
            captions: Some(icon),
            ..self
        }
    }
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for VideoOverlay<Message, Renderer>
where
    Message: Clone,
    Renderer: advanced::Renderer + text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, _bounds: iced::Size) -> layout::Node {
        let ppn_spacing = 48.0;
        let horizontal_padding = 10.0;
        let vertical_padding = 10.0;
        let bounds_size = self.bounds.size();
        let bounds_position = Point::ORIGIN;
        let mut paragraph: Plain<Renderer::Paragraph> = Plain::default();

        let mut min_bounds = |icon: &Icon<Message, Renderer::Font>| {
            let size = icon.size.unwrap_or_else(|| renderer.default_size());
            let line_height = text::LineHeight::Relative(1.0);
            let height = line_height.to_absolute(size);

            let mut content = [0; 4];

            let icon_text = Text {
                content: icon.code_point.encode_utf8(&mut content) as &_,
                font: icon.font,
                size,
                bounds: Size::new(f32::INFINITY, height.0),
                line_height,
                wrapping: text::Wrapping::default(),
                shaping: text::Shaping::Advanced,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
            };

            paragraph.update(icon_text);
            paragraph.min_bounds()
        };

        let play = match &self.play {
            None => Node::default(),
            Some(icon) => {
                let min_bounds = min_bounds(icon);

                let x = bounds_position.x + (bounds_size.width * 0.5) - (min_bounds.width * 0.5);
                let y = bounds_position.y + (bounds_size.height * 0.5) - (min_bounds.height * 0.5);

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let previous = match &self.previous {
            None => Node::default(),
            Some(icon) => {
                let play = play.size();
                let min_bounds = min_bounds(icon);
                let (_, hor) = padding(min_bounds, icon.size.unwrap_or(renderer.default_size()));
                let x = bounds_position.x + (bounds_size.width * 0.5)
                    - (play.width * 0.5)
                    - ppn_spacing
                    - (hor / 2.0)
                    - (min_bounds.width);
                let y = bounds_position.y + (bounds_size.height * 0.5) - (min_bounds.height * 0.5);

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let next = match &self.next {
            None => Node::default(),
            Some(icon) => {
                let min_bounds = min_bounds(icon);
                let (_, hor) = padding(min_bounds, icon.size.unwrap_or(renderer.default_size()));
                let play = play.size().width * 0.5;
                let x = bounds_position.x
                    + (bounds_size.width * 0.5)
                    + play
                    + ppn_spacing
                    + (hor / 2.0);

                let y = bounds_position.y + (bounds_size.height * 0.5) - (min_bounds.height * 0.5);

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let fullscreen = match &self.fullscreen {
            None => Node::default(),
            Some(icon) => {
                let min_bounds = min_bounds(icon);
                let x =
                    bounds_position.x + bounds_size.width - horizontal_padding - min_bounds.width;
                let y =
                    bounds_position.y + bounds_size.height - vertical_padding - min_bounds.height;

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let captions = match &self.captions {
            None => Node::default(),
            Some(icon) => {
                let min_bounds = min_bounds(icon);
                let x =
                    bounds_position.x + bounds_size.width - horizontal_padding - min_bounds.width;
                let y = bounds_position.y + vertical_padding;

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let speed = {
            let size = renderer.default_size() * SPEED_SIZE_MULT;
            let line_height = text::LineHeight::default();
            let height = line_height.to_absolute(size);
            let content = format!("{:.02}", self.speed);
            let font = <Renderer as text::Renderer>::MONOSPACE_FONT;

            let text = Text {
                content: content.as_str(),
                font,
                size,
                bounds: Size::new(f32::INFINITY, height.0),
                line_height,
                wrapping: text::Wrapping::default(),
                shaping: text::Shaping::Basic,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
            };

            paragraph.update(text);
            let min_bounds = paragraph.min_bounds();
            let x = bounds_position.x + horizontal_padding;
            let y = bounds_position.y + vertical_padding;

            Node::new(min_bounds).move_to((x, y))
        };

        layout::Node::with_children(
            bounds_size,
            vec![speed, play, previous, next, fullscreen, captions],
        )
        .move_to(self.bounds.position())
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &advanced::renderer::Style,
        layout: layout::Layout<'_>,
        _cursor: advanced::mouse::Cursor,
    ) {
        let no_overlay = self.play.is_none()
            && self.previous.is_none()
            && self.next.is_none()
            && self.fullscreen.is_none()
            && self.captions.is_none();

        let alpha = 0.85;
        let overlay_color = color!(15, 26, 32);
        let clip_bounds = layout.bounds();
        let mut children = layout.children();

        let speed_layout = children.next().expect("Missing speed layout");
        let speed = speed_layout.bounds();
        let text_color = style.text_color;
        let text_color = Color {
            a: alpha,
            ..text_color
        };

        let size = renderer.default_size() * SPEED_SIZE_MULT;
        let line_height = text::LineHeight::default();
        let height = line_height.to_absolute(size);

        let content = format!("{:.02}", self.speed);

        let text = Text {
            content,
            font: renderer.default_font(),
            size,
            bounds: Size::new(f32::INFINITY, height.0),
            line_height,
            wrapping: text::Wrapping::default(),
            shaping: text::Shaping::Basic,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
        };

        renderer.fill_quad(
            Quad {
                bounds: if no_overlay {
                    speed_layout.bounds().expand([3, 6])
                } else {
                    layout.bounds()
                },
                ..Default::default()
            },
            overlay_color.scale_alpha(0.3),
        );
        renderer.fill_text(text, speed.position(), text_color, clip_bounds);

        let draw =
            |renderer: &mut Renderer, icon: &Icon<Message, Renderer::Font>, bounds: Rectangle| {
                let color = icon.color.unwrap_or(style.text_color);
                let color = Color { a: alpha, ..color };

                let size = icon.size.unwrap_or_else(|| renderer.default_size());
                let line_height = text::LineHeight::Relative(1.0);

                let mut content = [0; 4];
                let content = icon.code_point.encode_utf8(&mut content) as &str;
                let content = content.to_string();

                let icon_text = Text {
                    content,
                    font: icon.font,
                    size,
                    bounds: bounds.size(),
                    line_height,
                    wrapping: text::Wrapping::default(),
                    shaping: text::Shaping::Advanced,
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                };

                renderer.fill_text(icon_text, bounds.center(), color, clip_bounds);
            };

        let border = Border::default().rounded(50.0);
        let background_color = overlay_color.scale_alpha(0.5);

        match &self.play {
            None => {
                let _ = children.next();
            }
            Some(icon) => {
                let layout = children.next().expect("Missing play layout");
                let bounds = layout.bounds();
                let (ver, hor) =
                    padding(bounds.size(), icon.size.unwrap_or(renderer.default_size()));

                let bounds = bounds.expand([ver, hor]);

                renderer.fill_quad(
                    Quad {
                        bounds,
                        border,
                        ..Default::default()
                    },
                    background_color,
                );

                draw(renderer, icon, bounds);
            }
        };

        match &self.previous {
            None => {
                let _ = children.next();
            }
            Some(icon) => {
                let layout = children.next().expect("Missing previous layout");
                let bounds = layout.bounds();
                let (ver, hor) =
                    padding(bounds.size(), icon.size.unwrap_or(renderer.default_size()));

                let bounds = bounds.expand([ver, hor]);

                renderer.fill_quad(
                    Quad {
                        bounds,
                        border,
                        ..Default::default()
                    },
                    background_color,
                );

                draw(renderer, icon, bounds);
            }
        };

        match &self.next {
            None => {
                let _ = children.next();
            }
            Some(icon) => {
                let layout = children.next().expect("Missing next layout");
                let bounds = layout.bounds();
                let (ver, hor) =
                    padding(bounds.size(), icon.size.unwrap_or(renderer.default_size()));

                let bounds = bounds.expand([ver, hor]);

                renderer.fill_quad(
                    Quad {
                        bounds,
                        border,
                        ..Default::default()
                    },
                    background_color,
                );

                draw(renderer, icon, bounds);
            }
        };

        match &self.fullscreen {
            None => {
                let _ = children.next();
            }
            Some(icon) => {
                let layout = children.next().expect("Missing fullscreen layout");
                let bounds = layout.bounds();
                draw(renderer, icon, bounds);
            }
        };

        match &self.captions {
            None => {
                let _ = children.next();
            }
            Some(icon) => {
                let layout = children.next().expect("Missing captions layout");
                let bounds = layout.bounds();
                draw(renderer, icon, bounds);
            }
        };
    }

    fn update(
        &mut self,
        event: &iced::Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn advanced::Clipboard,
        shell: &mut advanced::Shell<'_, Message>,
    ) {
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            let mut children = layout.children();
            let _speed = children.next();

            let play = children.next().expect("Update: Missing play layout");
            if cursor.is_over(play.bounds()) {
                if let Some(message) = self.play.as_ref().and_then(|icon| icon.message.as_ref()) {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let previous = children.next().expect("Update: Missing previous layout");
            if cursor.is_over(previous.bounds()) {
                if let Some(message) = self
                    .previous
                    .as_ref()
                    .and_then(|icon| icon.message.as_ref())
                {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let next = children.next().expect("Update: Missing next layout");
            if cursor.is_over(next.bounds()) {
                if let Some(message) = self.next.as_ref().and_then(|icon| icon.message.as_ref()) {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let fullscreen = children.next().expect("Update: Missing fullscreen layout");
            if cursor.is_over(fullscreen.bounds()) {
                if let Some(message) = self
                    .fullscreen
                    .as_ref()
                    .and_then(|icon| icon.message.as_ref())
                {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let captions = children.next().expect("Update: Missing captions layout");
            if cursor.is_over(captions.bounds()) {
                if let Some(message) = self
                    .captions
                    .as_ref()
                    .and_then(|icon| icon.message.as_ref())
                {
                    shell.publish(message.clone());
                    shell.capture_event();
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        layout: layout::Layout<'_>,
        cursor: advanced::mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }

        let mut children = layout.children();
        let _speed = children.next();

        if children.any(|child| cursor.is_over(child.bounds())) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

fn padding(bounds: Size, size: Pixels) -> (f32, f32) {
    let padding = size.0 / 3.0;
    let max = bounds.height.max(bounds.width);

    let hor = padding + (max - bounds.width) / 2.0;
    let ver = padding + (max - bounds.height) / 2.0;

    (ver, hor)
}
