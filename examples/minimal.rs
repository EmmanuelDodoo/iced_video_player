use iced::{
    font,
    widget::{Button, Column, Container, Row, Slider, Text},
    Element, Font, Task,
};
use iced_video_player::{Icon, Video, VideoPlayer};
use std::time::Duration;

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view).run()
}

#[derive(Clone, Debug)]
enum Message {
    FontLoaded(Result<(), font::Error>),
    TogglePause,
    ToggleLoop,
    Seek(f64),
    SeekRelease,
    EndOfStream,
    NewFrame,
}

struct App {
    video: Video,
    position: f64,
    dragging: bool,
}

impl App {
    fn boot() -> (Self, Task<Message>) {
        let task =
            font::load(include_bytes!("../assets/minimal.ttf").as_slice()).map(Message::FontLoaded);

        (Self::new(), task)
    }

    fn new() -> Self {
        let mut video = Video::new(
            &url::Url::from_file_path(
                std::path::PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join("../assets/test.mp4")
                    .canonicalize()
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        video.set_gamma(1.5);

        App {
            video,
            position: 0.0,
            dragging: false,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::FontLoaded(Err(error)) => {
                eprintln!("Couldn't load font: \n{error:?}");
            }
            Message::FontLoaded(Ok(_)) => {}
            Message::TogglePause => {
                self.video.set_paused(!self.video.paused());
            }
            Message::ToggleLoop => {
                self.video.set_looping(!self.video.looping());
            }
            Message::Seek(secs) => {
                self.dragging = true;
                self.video.set_paused(true);
                self.position = secs;
            }
            Message::SeekRelease => {
                self.dragging = false;
                self.video
                    .seek(Duration::from_secs_f64(self.position), false)
                    .expect("seek");
                self.video.set_paused(false);
            }
            Message::EndOfStream => {
                println!("end of stream");
            }
            Message::NewFrame => {
                if !self.dragging {
                    self.position = self.video.position().as_secs_f64();
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let code_point = if self.video.eos() || self.video.paused() {
            '\u{E800}'
        } else {
            '\u{E801}'
        };

        let icon = Icon {
            font: Font::with_name("minimal"),
            code_point,
            color: None,
            size: Some(40.0.into()),
        };

        Column::new()
            .push(
                Container::new(
                    VideoPlayer::new(&self.video)
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .play_icon(icon, Message::TogglePause)
                        .content_fit(iced::ContentFit::Contain)
                        .on_end_of_stream(Message::EndOfStream)
                        .on_new_frame(Message::NewFrame),
                )
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill),
            )
            .push(
                Container::new(
                    Slider::new(
                        0.0..=self.video.duration().as_secs_f64(),
                        self.position,
                        Message::Seek,
                    )
                    .step(0.1)
                    .on_release(Message::SeekRelease),
                )
                .padding(iced::Padding::new(5.0).left(10.0).right(10.0)),
            )
            .push(
                Row::new()
                    .spacing(5)
                    .align_y(iced::alignment::Vertical::Center)
                    .padding(iced::Padding::new(10.0).top(0.0))
                    .push(
                        Button::new(Text::new(if self.video.paused() {
                            "Play"
                        } else {
                            "Pause"
                        }))
                        .width(80.0)
                        .on_press(Message::TogglePause),
                    )
                    .push(
                        Button::new(Text::new(if self.video.looping() {
                            "Disable Loop"
                        } else {
                            "Enable Loop"
                        }))
                        .width(120.0)
                        .on_press(Message::ToggleLoop),
                    )
                    .push(
                        Text::new(format!(
                            "{}:{:02}s / {}:{:02}s",
                            self.position as u64 / 60,
                            self.position as u64 % 60,
                            self.video.duration().as_secs() / 60,
                            self.video.duration().as_secs() % 60,
                        ))
                        .width(iced::Length::Fill)
                        .align_x(iced::alignment::Horizontal::Right),
                    ),
            )
            .into()
    }
}
