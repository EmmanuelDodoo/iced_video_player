use crate::{video_player::State, Icon, VideoPlayer};
use iced::{
    advanced::{
        self,
        layout::{self, Node},
        overlay,
        renderer::Quad,
        text::{self, paragraph::Plain, Text},
    },
    alignment, color, mouse, Color, Event, Rectangle, Size,
};
use iced_wgpu::primitive::Renderer as PrimitiveRenderer;

const SPEED_SIZE_MULT: f32 = 0.75;

pub struct VideoOverlay<'a, Message, Renderer = iced::Renderer>
where
    Renderer: text::Renderer,
{
    state: &'a mut State,
    bounds: Rectangle,
    speed: f64,
    play_pause: Option<(Icon<Renderer::Font>, Message)>,
    fullscreen: Option<(Icon<Renderer::Font>, Message)>,
    captions: Option<(Icon<Renderer::Font>, Message)>,
    previous: Option<(Icon<Renderer::Font>, Message)>,
    next: Option<(Icon<Renderer::Font>, Message)>,
}

impl<'a, Message, Renderer> VideoOverlay<'a, Message, Renderer>
where
    Message: Clone,
    Renderer: PrimitiveRenderer + text::Renderer,
{
    pub fn new<Theme>(
        state: &'a mut State,
        player: &VideoPlayer<'_, Message, Theme, Renderer>,
        bounds: Rectangle,
        speed: f64,
    ) -> Self {
        Self {
            state,
            bounds,
            speed,
            play_pause: player.play_pause.clone(),
            fullscreen: player.fullscreen.clone(),
            captions: player.captions.clone(),
            previous: player.previous.clone(),
            next: player.next.clone(),
        }
    }
}

impl<'a, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for VideoOverlay<'a, Message, Renderer>
where
    Message: Clone,
    Renderer: advanced::Renderer + text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, _bounds: iced::Size) -> layout::Node {
        let ppn_spacing = 24.0;
        let horizontal_padding = 10.0;
        let vertical_padding = 10.0;
        let bounds_size = self.bounds.size();
        let bounds_position = self.bounds.position();
        let mut paragraph: Plain<Renderer::Paragraph> = Plain::default();

        let mut min_bounds = |icon: &Icon<Renderer::Font>| {
            let size = icon.size.unwrap_or_else(|| renderer.default_size());
            let line_height = text::LineHeight::default();
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

        let play = match &self.play_pause {
            None => Node::default(),
            Some((icon, _)) => {
                let min_bounds = min_bounds(icon);

                let x = bounds_position.x + (bounds_size.width * 0.5) - (min_bounds.width * 0.5);
                let y = bounds_position.y + (bounds_size.height * 0.5) - (min_bounds.height * 0.5);

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let previous = match &self.previous {
            None => Node::default(),
            Some((icon, _)) => {
                let play = play.size();
                let min_bounds = min_bounds(icon);
                let x = bounds_position.x + (bounds_size.width * 0.5)
                    - (play.width * 0.5)
                    - ppn_spacing
                    - (min_bounds.width);
                let y = bounds_position.y + (bounds_size.height * 0.5) - (min_bounds.height * 0.5);

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let next = match &self.next {
            None => Node::default(),
            Some((icon, _)) => {
                let min_bounds = min_bounds(icon);
                let x = bounds_position.x + (bounds_size.width * 0.5) + ppn_spacing;

                let y = bounds_position.y + (bounds_size.height * 0.5) - (min_bounds.height * 0.5);

                Node::new(min_bounds).move_to((x, y))
            }
        };

        let fullscreen = match &self.fullscreen {
            None => Node::default(),
            Some((icon, _)) => {
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
            Some((icon, _)) => {
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

            let text = Text {
                content: content.as_str(),
                font: renderer.default_font(),
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
        .move_to(bounds_position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &advanced::renderer::Style,
        layout: layout::Layout<'_>,
        _cursor: advanced::mouse::Cursor,
    ) {
        let no_overlay = self.play_pause.is_none()
            && self.previous.is_none()
            && self.next.is_none()
            && self.fullscreen.is_none()
            && self.captions.is_none();

        let alpha = 0.85;
        let clip_bounds = layout.bounds();
        let mut children = layout.children();

        let speed_layout = children.next().expect("Missing speed layout");
        let speed = speed_layout.bounds();
        let color = style.text_color;
        let color = Color { a: alpha, ..color };

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
            color!(15, 26, 32, 0.3),
        );
        renderer.fill_text(text, speed.position(), color, clip_bounds);

        let mut draw = |icon: &Icon<Renderer::Font>, bounds: Rectangle| {
            let color = icon.color.unwrap_or(style.text_color);
            let color = Color { a: alpha, ..color };

            let size = icon.size.unwrap_or_else(|| renderer.default_size());
            let line_height = text::LineHeight::default();
            let height = line_height.to_absolute(size);

            let mut content = [0; 4];
            let content = icon.code_point.encode_utf8(&mut content) as &str;
            let content = content.to_string();

            let icon_text = Text {
                content,
                font: icon.font,
                size,
                bounds: Size::new(f32::INFINITY, height.0),
                line_height,
                wrapping: text::Wrapping::default(),
                shaping: text::Shaping::Advanced,
                align_x: text::Alignment::Left,
                align_y: alignment::Vertical::Top,
            };

            renderer.fill_text(icon_text, bounds.position(), color, clip_bounds);
        };

        match &self.play_pause {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
                let layout = children.next().expect("Missing play layout");
                let bounds = layout.bounds();
                draw(icon, bounds);
            }
        };

        match &self.previous {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
                let layout = children.next().expect("Missing previous layout");
                let bounds = layout.bounds();
                draw(icon, bounds);
            }
        };

        match &self.next {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
                let layout = children.next().expect("Missing next layout");
                let bounds = layout.bounds();
                draw(icon, bounds);
            }
        };

        match &self.fullscreen {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
                let layout = children.next().expect("Missing fullscreen layout");
                let bounds = layout.bounds();
                draw(icon, bounds);
            }
        };

        match &self.captions {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
                let layout = children.next().expect("Missing captions layout");
                let bounds = layout.bounds();
                draw(icon, bounds);
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
        if !cursor.is_over(layout.bounds()) {
            self.state.overlay = false;
            return;
        }

        self.state.overlay = true;

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            let mut children = layout.children();
            let _speed = children.next();

            let play = children.next().expect("Update: Missing play layout");
            if let Some((_, message)) = &self.play_pause {
                if cursor.is_over(play.bounds()) {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let previous = children.next().expect("Update: Missing previous layout");
            if let Some((_, message)) = &self.previous {
                if cursor.is_over(previous.bounds()) {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let next = children.next().expect("Update: Missing next layout");
            if let Some((_, message)) = &self.next {
                if cursor.is_over(next.bounds()) {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let fullscreen = children.next().expect("Update: Missing fullscreen layout");
            if let Some((_, message)) = &self.fullscreen {
                if cursor.is_over(fullscreen.bounds()) {
                    shell.publish(message.clone());
                    shell.capture_event();
                    return;
                }
            }

            let captions = children.next().expect("Update: Missing captions layout");
            if let Some((_, message)) = &self.captions {
                if cursor.is_over(captions.bounds()) {
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
