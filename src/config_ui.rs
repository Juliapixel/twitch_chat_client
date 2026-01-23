use iced::{
    Element, Length, Padding,
    widget::{Button, button, checkbox, column, row},
};

use crate::config::CONFIG;

pub struct ConfigUi {
    active_tab: Tab,
    natural_scrolling: bool,
}

#[derive(Debug, Clone, Default)]
pub enum Tab {
    #[default]
    General,
    Highlights,
    Sounds,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchTo(Tab),
}

fn tab(text: &'static str, tab: Tab) -> Element<'static, Message> {
    Button::new(text)
        .on_press(Message::SwitchTo(tab))
        .padding(Padding::ZERO.horizontal(12.0).vertical(6.0))
        .style(button::subtle)
        .width(Length::Fill)
        .into()
}

impl ConfigUi {
    pub fn new() -> Self {
        let config = CONFIG.read();
        Self {
            active_tab: Default::default(),
            natural_scrolling: config.ui.natural_scrolling,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sections = row![
            tab("General", Tab::General),
            tab("Highlights", Tab::Highlights),
            tab("Sounds", Tab::Sounds),
        ]
        .spacing(4)
        .width(Length::FillPortion(1))
        .wrap();
        let view = match self.active_tab {
            Tab::General => column![checkbox(self.natural_scrolling).label("Natural scrolling: ")],
            Tab::Highlights => column![],
            Tab::Sounds => column![],
        }
        .width(Length::FillPortion(3));
        row![sections, view].width(Length::Fill).into()
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::SwitchTo(tab) => self.active_tab = tab,
        };
        let mut config = CONFIG.write();
        config.ui.natural_scrolling = self.natural_scrolling;
        if let Err(e) = config.save() {
            log::error!("Error saving config file: {e}");
        }
    }
}
