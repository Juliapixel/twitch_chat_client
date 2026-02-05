use std::sync::LazyLock;

use iced::{
    Alignment, Color, Element, Length, Padding, Point, Task,
    widget::{Text, container, mouse_area, row, svg},
    window,
};

use crate::{res, widget::icon_button::IconButton};

static ICON: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(res!("cross.svg")));

pub struct TitleBar {
    title: String,
    window_id: window::Id,
    last_pos: Option<Point>,
    is_dragging: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Minimize,
    Maximize,
    Close,
    StartDrag,
    EndDrag,
    Move(Point),
    OpenSettings,
}

fn round_button(a: svg::Handle) -> IconButton<'static, Message> {
    IconButton::new(svg::Svg::new(a))
        .size(24)
        .padding(Padding::new(7.0))
        .color(Color::WHITE)
}

impl TitleBar {
    pub fn new(title: impl Into<String>, window_id: window::Id) -> Self {
        Self {
            title: title.into(),
            window_id,
            last_pos: None,
            is_dragging: false,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        mouse_area(
            container(
                row![
                    Text::new(&self.title),
                    container(
                        row![
                            round_button(ICON.clone()).on_click(Message::Minimize),
                            round_button(ICON.clone()).on_click(Message::Maximize),
                            round_button(ICON.clone()).on_click(Message::Close)
                        ]
                        .spacing(3.0)
                    )
                    .align_right(Length::Fill)
                    .padding(Padding::new(3.0))
                ]
                .width(Length::Fill)
                .align_y(Alignment::Center),
            )
            .padding(Padding::ZERO.left(12.0)),
        )
        .on_press(Message::StartDrag)
        .on_release(Message::EndDrag)
        .on_move(Message::Move)
        .into()
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Minimize => iced::window::minimize(self.window_id, true),
            Message::Maximize => iced::window::toggle_maximize(self.window_id),
            Message::Close => iced::exit(),
            Message::OpenSettings => Task::none(),
            Message::StartDrag => iced::window::drag(self.window_id),
            Message::EndDrag => Task::none(),
            Message::Move(point) => {
                if self.is_dragging {
                    if let Some(pos) = self.last_pos {}
                    self.last_pos = Some(point)
                }
                Task::none()
            }
        }
    }
}
