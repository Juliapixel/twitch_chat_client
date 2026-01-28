use std::fmt::Display;

use iced::{
    Element, Event, Length, Rectangle, Size,
    advanced::{
        Renderer, Widget,
        layout::Node,
        svg::Renderer as SvgRenderer,
        text::Renderer as TextRenderer,
        widget::{
            Tree,
            tree::{State, Tag},
        },
    },
    mouse::{self, Interaction},
    theme,
    widget::{
        Row,
        button::Catalog as ButtonCatalog,
        row::Wrapping,
        svg::{self, Catalog as SvgCatalog},
        text::Catalog as TextCatalog,
    },
};

use crate::widget::tab::Tab;

pub struct Tabs<'a, M, T, R, TabId> {
    row: Wrapping<'a, M, T, R>,
    tabs: Vec<(TabId, Element<'a, M, T, R>)>,
    fallback: Option<Element<'a, M, T, R>>,
    on_add: Option<M>,
    on_close: Option<Box<dyn Fn(TabId) -> M>>,
    on_reorder: Option<Box<dyn Fn(usize, usize) -> M>>,
}

#[derive(Debug)]
pub struct TabsState<TabId: Clone + Eq> {
    selected: Option<TabId>,
}

impl<'a, M, T, R, TabId> Tabs<'a, M, T, R, TabId>
where
    M: Clone + 'a,
    R: Renderer + TextRenderer + SvgRenderer + 'a,
    T: TextCatalog + ButtonCatalog + SvgCatalog + theme::Base + 'a,
    <T as SvgCatalog>::Class<'static>: From<Box<dyn Fn(&T, svg::Status) -> svg::Style + 'static>>,
    TabId: Clone + Eq + Display + 'a,
{
    pub fn new(tabs: impl IntoIterator<Item = (TabId, impl Into<Element<'a, M, T, R>>)>) -> Self {
        let mut row = Row::new().spacing(2).width(Length::Fill);
        let mut tabs_vec = Vec::<(TabId, Element<'a, M, T, R>)>::new();
        for c in tabs {
            row = row.push(Tab::new(c.0.clone()));
            tabs_vec.push((c.0, c.1.into()));
        }
        Self {
            row: row.wrap(),
            tabs: tabs_vec,
            fallback: None,
            on_add: None,
            on_close: None,
            on_reorder: None,
        }
    }

    pub fn fallback(mut self, fallback: impl Into<Element<'a, M, T, R>>) -> Self {
        self.fallback = Some(fallback.into());
        self
    }

    pub fn on_add(mut self, msg: M) -> Self {
        self.on_add = Some(msg);
        self
    }

    pub fn on_close(mut self, msg: impl Fn(TabId) -> M + 'static) -> Self {
        self.on_close = Some(Box::new(msg));
        self
    }

    pub fn on_reorder(mut self, msg: impl Fn(usize, usize) -> M + 'static) -> Self {
        self.on_reorder = Some(Box::new(msg));
        self
    }

    #[allow(clippy::type_complexity)]
    fn get_active(
        &self,
        state: &TabsState<TabId>,
    ) -> Option<(usize, &(TabId, Element<'a, M, T, R>))> {
        if let Some(selected) = &state.selected {
            self.tabs.iter().enumerate().find(|t| selected == &t.1.0)
        } else {
            None
        }
    }

    #[allow(clippy::type_complexity)]
    fn get_active_mut(
        &mut self,
        state: &TabsState<TabId>,
    ) -> Option<(usize, &mut (TabId, Element<'a, M, T, R>))> {
        if let Some(selected) = &state.selected {
            self.tabs
                .iter_mut()
                .enumerate()
                .find(|t| selected == &t.1.0)
        } else {
            None
        }
    }
}

impl<'a, M, T, R, TabId> Widget<M, T, R> for Tabs<'a, M, T, R, TabId>
where
    M: Clone + 'a,
    R: Renderer + TextRenderer + SvgRenderer + 'a,
    T: TextCatalog + ButtonCatalog + SvgCatalog + theme::Base + 'a,
    <T as SvgCatalog>::Class<'static>: From<Box<dyn Fn(&T, svg::Status) -> svg::Style + 'static>>,
    TabId: Clone + Eq + Display + 'static,
{
    fn size(&self) -> iced::Size<iced::Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &R,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let span = iced::debug::time("tabs layout");
        let limits = limits.width(Length::Fill);
        let row_node = self.row.layout(
            &mut tree.children[0],
            renderer,
            &limits.height(Length::Shrink).loose(),
        );

        let mut children = Vec::with_capacity(2);
        let bounds = row_node.bounds();
        children.push(row_node);
        let offset = if self.fallback.is_some() { 2 } else { 1 };
        if let Some((i, (_, active))) = self.get_active_mut(tree.state.downcast_ref()) {
            let tree = &mut tree.children[i + offset];
            let mut shown_node =
                active
                    .as_widget_mut()
                    .layout(tree, renderer, &limits.shrink([0.0, bounds.height]));
            shown_node = shown_node.move_to([bounds.x, bounds.y + bounds.height]);
            children.push(shown_node);
        } else if let Some(fallback) = &mut self.fallback
            && self.tabs.is_empty()
        {
            children.push(fallback.as_widget_mut().layout(
                &mut tree.children[1],
                renderer,
                &limits,
            ));
        };
        span.finish();
        Node::with_children(limits.max(), children)
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut R,
        theme: &T,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.row.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.child(0),
            cursor,
            viewport,
        );
        if let Some(layout) = layout.children().nth(1) {
            if let Some((i, (_, active))) = self.get_active(tree.state.downcast_ref()) {
                let offset = if self.fallback.is_some() { 2 } else { 1 };
                let tree = &tree.children[i + offset];

                active
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, viewport);
            } else if let Some(fallback) = &self.fallback {
                fallback.as_widget().draw(
                    &tree.children[1],
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    viewport,
                )
            };
        }
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_mut::<TabsState<TabId>>();
        if (state.selected.is_none() && !self.tabs.is_empty())
            || !self
                .tabs
                .iter()
                .any(|t| Some(&t.0) == state.selected.as_ref())
        {
            state.selected = self.tabs.first().map(|t| t.0.clone());
            if state.selected.is_some() {
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }

        let click = if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && let Some(pos) = cursor.position()
        {
            layout
                .child(0)
                .children()
                .enumerate()
                .find(|l| l.1.bounds().contains(pos))
                .map(|b| (b.0, b.1.child(1).bounds().contains(pos)))
        } else {
            None
        };

        if let Some((idx, close)) = click {
            if let Some(on_add) = &self.on_add
                && idx == self.tabs.len()
            {
                shell.publish(on_add.clone());
                shell.capture_event();
            } else if close && let Some(on_close) = &self.on_close {
                shell.publish(on_close(self.tabs[idx].0.clone()));
                shell.capture_event();
            } else {
                let new_selected = self.tabs[idx].0.clone();

                if state.selected.as_ref().is_some_and(|s| s != &new_selected) {
                    shell.invalidate_layout();
                    shell.request_redraw();
                }

                state.selected = Some(new_selected);
                shell.capture_event();
            }
        }

        self.row.update(
            &mut tree.children[0],
            event,
            layout.child(0),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if let Some(layout) = layout.children().nth(1) {
            let offset = if self.fallback.is_some() { 2 } else { 1 };
            if let Some((i, (_, active))) = self.get_active_mut(tree.state.downcast_ref()) {
                active.as_widget_mut().update(
                    &mut tree.children[i + offset],
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            } else if let Some(fallback) = &mut self.fallback {
                fallback.as_widget_mut().update(
                    &mut tree.children[1],
                    event,
                    layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
        }
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        let mut children = Vec::<&dyn Widget<_, _, _>>::with_capacity(2 + self.tabs.len());
        children.push(&self.row);
        if let Some(fallback) = &self.fallback {
            children.push(fallback.as_widget());
        }
        children.extend(self.tabs.iter().map(|t| t.1.as_widget()));
        tree.diff_children(children.as_slice());
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        State::new(TabsState::<TabId> {
            selected: self.tabs.first().map(|f| f.0.clone()),
        })
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        Tag::of::<TabsState<TabId>>()
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
            self.row
                .operate(&mut tree.children[0], layout.child(0), renderer, op);
            if let Some(layout) = layout.children().nth(1) {
                let offset = if self.fallback.is_some() { 2 } else { 1 };
                if let Some((i, (_, active))) = self.get_active_mut(tree.state.downcast_ref()) {
                    let tree = &mut tree.children[i + offset];
                    active.as_widget_mut().operate(tree, layout, renderer, op);
                } else if let Some(fallback) = &mut self.fallback {
                    let tree = &mut tree.children[1];
                    fallback.as_widget_mut().operate(tree, layout, renderer, op);
                }
            }
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &Rectangle,
        renderer: &R,
    ) -> iced::advanced::mouse::Interaction {
        let row_inter = self.row.mouse_interaction(
            &tree.children[0],
            layout.child(0),
            cursor,
            viewport,
            renderer,
        );
        if !matches!(row_inter, Interaction::None) {
            return row_inter;
        }
        if let Some((i, (_, active))) = self.get_active(tree.state.downcast_ref())
            && let Some(layout) = layout.children().nth(1)
        {
            let offset = if self.fallback.is_some() { 2 } else { 1 };
            let tree = &tree.children[i + offset];

            return active
                .as_widget()
                .mouse_interaction(tree, layout, cursor, viewport, renderer);
        };
        Interaction::None
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        let mut children = Vec::with_capacity(2 + self.tabs.len());
        let dyn_row: &dyn Widget<M, T, R> = &self.row;
        children.push(Tree::new(dyn_row));
        if let Some(fallback) = &self.fallback {
            children.push(Tree::new(fallback.as_widget()));
        }
        children.extend(self.tabs.iter().map(|t| Tree::new(t.1.as_widget())));
        children
    }
}

impl<'a, M, T, R, TabId> From<Tabs<'a, M, T, R, TabId>> for Element<'a, M, T, R>
where
    M: Clone + 'a,
    T: TextCatalog + ButtonCatalog + SvgCatalog + theme::Base + 'a,
    <T as SvgCatalog>::Class<'static>: From<Box<dyn Fn(&T, svg::Status) -> svg::Style + 'static>>,
    R: Renderer + TextRenderer + SvgRenderer + 'a,
    TabId: Clone + Eq + Display + 'static,
{
    fn from(value: Tabs<'a, M, T, R, TabId>) -> Self {
        Element::new(value)
    }
}
