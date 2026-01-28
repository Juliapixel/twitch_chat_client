use std::{fmt::Display, sync::LazyLock};

use iced::{
    Background, Border, Color, Element, Event, Rectangle, Shadow, Size,
    advanced::{
        Renderer, Widget,
        layout::Node,
        mouse::{Click, Cursor, click},
        renderer::Quad,
        text,
        widget::{Text, Tree, tree::Tag},
    },
    gradient::{ColorStop, Linear},
    mouse,
    theme::{self, Palette, palette::Extended},
    touch,
    widget::{Svg, svg},
};

use crate::res;

static CROSS_SVG: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(res!("cross.svg")));

pub struct Tab<TabId, M, T, R>
where
    T: iced::widget::text::Catalog + svg::Catalog + theme::Base,
    R: text::Renderer + iced::advanced::svg::Renderer,
{
    id: TabId,
    cross: Svg<'static, T>,
    label: Text<'static, T, R>,
    active: bool,
    on_click: Option<M>,
    on_double_click: Option<M>,
    on_close: Option<M>,
}

impl<TabId, M, T, R> Tab<TabId, M, T, R>
where
    T: iced::widget::text::Catalog + svg::Catalog + theme::Base,
    <T as svg::Catalog>::Class<'static>: From<Box<dyn Fn(&T, svg::Status) -> svg::Style + 'static>>,
    R: text::Renderer + iced::advanced::svg::Renderer,
    TabId: Display,
{
    pub fn new(id: TabId) -> Self {
        let close_button = Svg::new(CROSS_SVG.clone())
            .width(10)
            .height(10)
            .style(|_, _| {
                let t = <T as theme::Base>::default(theme::Mode::Dark);
                let col = t
                    .palette()
                    .map(|p| Extended::generate(p).secondary.base.color)
                    .unwrap_or(t.base().text_color);
                svg::Style { color: Some(col) }
            });
        Self {
            label: iced::widget::Text::new(id.to_string()).size(14),
            id,
            cross: close_button,
            active: false,
            on_click: None,
            on_double_click: None,
            on_close: None,
        }
    }

    pub fn on_click(mut self, on_click: M) -> Self {
        self.on_click = Some(on_click);
        self
    }

    pub fn on_double_click(mut self, on_double_click: M) -> Self {
        self.on_double_click = Some(on_double_click);
        self
    }

    pub fn on_close(mut self, on_close: M) -> Self {
        self.on_close = Some(on_close);
        self
    }

    pub fn set_active(mut self) -> Self {
        self.active = true;
        self
    }
}

fn tab_background(palette: Palette) -> (Color, Color) {
    let e = Extended::generate(palette);
    (e.background.strong.color, e.background.strongest.color)
}

struct State {
    last_click: Option<Click>,
    mouse_over: bool,
}

impl<M, T, R, TabId> Widget<M, T, R> for Tab<TabId, M, T, R>
where
    M: Clone,
    T: theme::Base + iced::widget::text::Catalog + svg::Catalog,
    R: Renderer + text::Renderer + iced::advanced::svg::Renderer,
{
    fn size(&self) -> iced::Size<iced::Length> {
        Size::new(iced::Length::Fill, iced::Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &R,
        limits: &iced::advanced::layout::Limits,
    ) -> Node {
        let mut text = <Text<_, _> as Widget<M, _, _>>::layout(
            &mut self.label,
            &mut tree.children[0],
            renderer,
            &limits.loose(),
        );
        let mut close = <Svg<_> as Widget<M, _, _>>::layout(
            &mut self.cross,
            &mut tree.children[1],
            renderer,
            &limits.loose(),
        );

        close.translate_mut([text.bounds().width + 4.0, 0.0]);

        if text.bounds().height > close.bounds().height {
            close.translate_mut([0.0, (text.bounds().height - close.bounds().height) / 2.0]);
        } else {
            text.translate_mut([0.0, (close.bounds().height - text.bounds().height) / 2.0]);
        }

        text.translate_mut([10.0, 6.0]);
        close.translate_mut([10.0, 6.0]);

        Node::with_children(
            text.bounds()
                .union(&close.bounds())
                .size()
                .expand([20.0, 12.0]),
            vec![text, close],
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &T,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let (base, strong) = theme.palette().map(tab_background).unwrap_or_default();
        let bg = if self.active
            || cursor
                .position()
                .is_some_and(|p| layout.bounds().contains(p))
        {
            Linear::new(0.0)
                .add_stops([
                    ColorStop {
                        offset: 0.2,
                        color: strong,
                    },
                    ColorStop {
                        offset: 1.0,
                        color: base,
                    },
                ])
                .into()
        } else {
            Background::Color(base)
        };

        renderer.fill_quad(
            Quad {
                bounds: layout.bounds(),
                border: Border::default().rounded(6.0),
                shadow: Shadow::default(),
                snap: false,
            },
            bg,
        );

        if cursor
            .position()
            .is_some_and(|p| layout.bounds().contains(p))
        {
            <Text<_, _> as Widget<M, _, _>>::draw(
                &self.label,
                &tree.children[0],
                renderer,
                theme,
                style,
                layout.child(0),
                cursor,
                viewport,
            );
            <Svg<_> as Widget<M, _, _>>::draw(
                &self.cross,
                &tree.children[1],
                renderer,
                theme,
                style,
                layout.child(1),
                cursor,
                viewport,
            );
        } else {
            renderer.with_translation(
                [layout.child(1).bounds().width / 2.0 + 2.0, 0.0].into(),
                |r| {
                    <Text<_, _> as Widget<M, _, _>>::draw(
                        &self.label,
                        &tree.children[0],
                        r,
                        theme,
                        style,
                        layout.child(0),
                        cursor,
                        viewport,
                    );
                },
            );
        }
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State {
            last_click: None,
            mouse_over: false,
        })
    }

    fn children(&self) -> Vec<Tree> {
        let l: &dyn Widget<M, T, R> = &self.label;
        let c: &dyn Widget<M, T, R> = &self.cross;
        vec![Tree::new(l), Tree::new(c)]
    }

    fn diff(&self, tree: &mut Tree) {
        let l: &dyn Widget<M, T, R> = &self.label;
        let c: &dyn Widget<M, T, R> = &self.cross;
        tree.diff_children(&[l, c]);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &R,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |op| {
            <Text<_, _> as Widget<M, _, _>>::operate(
                &mut self.label,
                &mut tree.children[0],
                layout.child(0),
                renderer,
                op,
            );
            <Svg<_> as Widget<M, _, _>>::operate(
                &mut self.cross,
                &mut tree.children[1],
                layout.child(1),
                renderer,
                op,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &R,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let Some(pos) = cursor.position() else { return };

                if layout.child(1).bounds().contains(pos)
                    && let Some(on_close) = &self.on_close
                {
                    shell.publish(on_close.clone());
                    shell.capture_event();
                } else if layout.bounds().contains(pos) {
                    let click = Click::new(pos, mouse::Button::Left, state.last_click);

                    if click.kind() == click::Kind::Double
                        && let Some(on_double_click) = &self.on_double_click
                    {
                        log::info!("Double click!");
                        shell.publish(on_double_click.clone());
                        shell.capture_event();
                    } else if let Some(on_click) = &self.on_click {
                        log::info!("click!");
                        shell.publish(on_click.clone());
                        shell.capture_event();
                    }
                    state.last_click = Some(click)
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position: pos }) => {
                if layout.bounds().contains(*pos) && !state.mouse_over {
                    shell.request_redraw();
                    state.mouse_over = true
                } else if state.mouse_over {
                    shell.request_redraw();
                    state.mouse_over = false
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &R,
    ) -> iced::advanced::mouse::Interaction {
        if let Some(pos) = cursor.position()
            && layout.bounds().contains(pos)
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

impl<'a, M, T, R, TabId> From<Tab<TabId, M, T, R>> for Element<'a, M, T, R>
where
    M: Clone + 'a,
    T: theme::Base + iced::widget::text::Catalog + svg::Catalog + 'a,
    R: Renderer + text::Renderer + iced::advanced::svg::Renderer + 'a,
    TabId: 'a,
{
    fn from(value: Tab<TabId, M, T, R>) -> Self {
        Element::new(value)
    }
}
