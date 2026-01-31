use iced::{
    Element, Task,
    widget::{self, button, column, container, row, sensor, text_input},
};

pub struct JoinPopup {
    pub value: String,
    input_id: widget::Id,
}

#[derive(Debug, Clone)]
pub enum Message {
    Shown,
    ChannelChange(String),
    Submit,
    Close,
}

impl JoinPopup {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            input_id: widget::Id::unique(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        sensor(
            container(column![
                text_input("Twitch Login", &self.value)
                    .id(self.input_id.clone())
                    .on_input(Message::ChannelChange)
                    .on_submit(Message::Submit)
                    .width(300.0),
                row![
                    button("Confirm").on_press(Message::Submit),
                    button("Cancel").on_press(Message::Close)
                ]
            ])
            .style(iced::widget::container::rounded_box)
            .padding(20),
        )
        .on_show(|_| Message::Shown)
        .into()
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Shown => return iced::widget::operation::focus(self.input_id.clone()),
            Message::ChannelChange(c) => self.value = c,
            Message::Submit => (),
            Message::Close => (),
        };
        Task::none()
    }
}
