use std::time::Instant;

use crate::{video_player::State, Icon, Update, VideoPlayer};
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

const SPEED_SIZE_MULT: f32 = 0.75;

pub struct VideoOverlay<'a, Message, Renderer = iced::Renderer>
where
    Renderer: text::Renderer,
{
    state: &'a mut State,
    timeout: u64,
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
            timeout: player.overlay_timeout,
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
        let ppn_spacing = 48.0;
        let horizontal_padding = 10.0;
        let vertical_padding = 10.0;
        let bounds_size = self.bounds.size();
        let bounds_position = Point::ORIGIN;
        let mut paragraph: Plain<Renderer::Paragraph> = Plain::default();

        let mut min_bounds = |icon: &Icon<Renderer::Font>| {
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
            Some((icon, _)) => {
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
        let no_overlay = self.play_pause.is_none()
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

        let draw = |renderer: &mut Renderer, icon: &Icon<Renderer::Font>, bounds: Rectangle| {
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

        match &self.play_pause {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
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
            Some((icon, _)) => {
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
            Some((icon, _)) => {
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
            Some((icon, _)) => {
                let layout = children.next().expect("Missing fullscreen layout");
                let bounds = layout.bounds();
                draw(renderer, icon, bounds);
            }
        };

        match &self.captions {
            None => {
                let _ = children.next();
            }
            Some((icon, _)) => {
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
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let mut children = layout.children();
                let _speed = children.next();

                let play = children.next().expect("Update: Missing play layout");
                if cursor.is_over(play.bounds()) {
                    self.state.last_update = Some(Update {
                        time: Instant::now(),
                        parent: None,
                        overlay: cursor.position_over(play.bounds()),
                    });
                    if let Some((_, message)) = &self.play_pause {
                        shell.publish(message.clone());
                        shell.capture_event();
                        return;
                    }
                }

                let previous = children.next().expect("Update: Missing previous layout");
                if cursor.is_over(previous.bounds()) {
                    self.state.last_update = Some(Update {
                        time: Instant::now(),
                        parent: None,
                        overlay: cursor.position_over(previous.bounds()),
                    });
                    if let Some((_, message)) = &self.previous {
                        shell.publish(message.clone());
                        shell.capture_event();
                        return;
                    }
                }

                let next = children.next().expect("Update: Missing next layout");
                if cursor.is_over(next.bounds()) {
                    self.state.last_update = Some(Update {
                        time: Instant::now(),
                        parent: None,
                        overlay: cursor.position_over(next.bounds()),
                    });
                    if let Some((_, message)) = &self.next {
                        shell.publish(message.clone());
                        shell.capture_event();
                        return;
                    }
                }

                let fullscreen = children.next().expect("Update: Missing fullscreen layout");
                if cursor.is_over(fullscreen.bounds()) {
                    self.state.last_update = Some(Update {
                        time: Instant::now(),
                        parent: None,
                        overlay: cursor.position_over(fullscreen.bounds()),
                    });
                    if let Some((_, message)) = &self.fullscreen {
                        shell.publish(message.clone());
                        shell.capture_event();
                        return;
                    }
                }

                let captions = children.next().expect("Update: Missing captions layout");
                if cursor.is_over(captions.bounds()) {
                    self.state.last_update = Some(Update {
                        time: Instant::now(),
                        parent: None,
                        overlay: cursor.position_over(captions.bounds()),
                    });
                    if let Some((_, message)) = &self.captions {
                        shell.publish(message.clone());
                        shell.capture_event();
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorEntered)
            | Event::Mouse(mouse::Event::CursorLeft)
            | Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                self.state.last_update = match self.state.last_update {
                    Some(Update { time, parent, .. }) => Some(Update {
                        time,
                        parent,
                        overlay: cursor.position_over(layout.bounds()),
                    }),
                    None => {
                        if cursor.is_over(layout.bounds()) {
                            Some(Update {
                                time: Instant::now(),
                                parent: None,
                                overlay: cursor.position_over(layout.bounds()),
                            })
                        } else {
                            None
                        }
                    }
                };
            }
            Event::Window(iced::window::Event::RedrawRequested(_)) => {
                match self.state.last_update.take() {
                    Some(Update {
                        parent: position,
                        time,
                        overlay,
                    }) if overlay.is_some() => {
                        if cursor.position_over(layout.bounds()) == overlay
                            && Instant::now().duration_since(time).as_secs() >= self.timeout
                        {
                        } else {
                            self.state.last_update = Some(Update {
                                time,
                                parent: position,
                                overlay,
                            })
                        }
                    }
                    Some(Update {
                        parent: None,
                        overlay: None,
                        ..
                    }) => {}
                    Some(Update {
                        overlay,
                        parent,
                        time,
                    }) if parent.is_some() => {
                        self.state.last_update = Some(Update {
                            time,
                            parent,
                            overlay,
                        });
                    }
                    _ => {}
                }
            }
            _ => {}
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
