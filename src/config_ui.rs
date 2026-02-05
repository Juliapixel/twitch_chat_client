use iced::{
    Element, Length, Padding,
    widget::{Button, Container, Text, button, checkbox, column, row},
};

use crate::config::{CONFIG, Config};

pub struct ConfigUi {
    active_tab: Tab,
}

#[derive(Debug, Clone, Default)]
pub enum Tab {
    #[default]
    General,
    Highlights,
    Sounds,
    About,
}

#[derive(derive_more::Debug)]
pub enum Message {
    SwitchTo(Tab),
    #[debug("Box<dyn ConfigChanger>")]
    Execute(Box<dyn ConfigChanger>),
}

pub trait ConfigChanger: Fn(&mut Config) + Send {
    fn clone_boxed(&self) -> Box<dyn ConfigChanger>;
}

impl<T: Fn(&mut Config) + Clone + Send + 'static> ConfigChanger for T {
    fn clone_boxed(&self) -> Box<dyn ConfigChanger> {
        Box::new(self.clone())
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::SwitchTo(arg0) => Self::SwitchTo(arg0.clone()),
            Self::Execute(arg0) => Self::Execute(arg0.clone_boxed()),
        }
    }
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
        Self {
            active_tab: Default::default(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let cfg = CONFIG.read();

        let sections = row![
            tab("General", Tab::General),
            tab("Highlights", Tab::Highlights),
            tab("Sounds", Tab::Sounds),
            tab("About", Tab::About),
        ]
        .spacing(4)
        .width(Length::FillPortion(1))
        .wrap();
        let view: Element<'_, Message> = match self.active_tab {
            Tab::General => column![
                checkbox(cfg.ui.natural_scrolling)
                    .label("Natural scrolling")
                    .on_toggle(|l| Message::Execute(Box::new(move |c| {
                        c.ui.natural_scrolling = l
                    })))
            ]
            .into(),
            Tab::Highlights => column![].into(),
            Tab::Sounds => column![].into(),
            Tab::About => Element::new(Text::new("FART").size(200)),
        };
        let view = Container::new(view).width(Length::FillPortion(3));
        row![sections, view]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::SwitchTo(tab) => self.active_tab = tab,
            Message::Execute(f) => {
                let mut cfg = CONFIG.write();
                f(&mut cfg);
                if let Err(e) = cfg.save() {
                    log::error!("Error when saving settings: {e}");
                }
            }
        };
    }
}
