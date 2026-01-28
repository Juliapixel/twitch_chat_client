use iced::{
    Element, Event, Padding, Point, Rectangle, Vector,
    advanced::{Layout, Renderer, Widget, layout::Node, widget::Tree},
    mouse, touch,
};

pub struct Draggable<'a, M, T, R> {
    content: Element<'a, M, T, R>,
}

impl<'a, M, T, R> Draggable<'a, M, T, R> {
    pub fn new(content: impl Into<Element<'a, M, T, R>>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

#[derive(Debug)]
struct State {
    offset: Vector,
    last_position: Option<Point>,
    is_dragging: bool,
}

impl State {
    fn new() -> Self {
        Self {
            offset: Vector::ZERO,
            last_position: None,
            is_dragging: false,
        }
    }
}

fn traverse<'a>(offset: Vector, bounds: Rectangle, n: impl Iterator<Item = Layout<'a>>) -> Node {
    let a = n.map(|i| {
        let offset = {
            let p = i.bounds().position() - offset;
            Vector::new(p.x, p.y)
        };
        traverse(offset, i.bounds(), i.children())
    });
    Node::with_children(bounds.size(), a.collect()).translate(offset)
}

impl<'a, M, T, R> Widget<M, T, R> for Draggable<'a, M, T, R>
where
    R: Renderer,
{
    fn size(&self) -> iced::Size<iced::Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &R,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        // let state = tree.state.downcast_ref::<State>();

        let node =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &limits.loose());
        Node::container(node, Padding::ZERO)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &T,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let layout = layout.child(0);
        let node = traverse(state.offset, layout.bounds(), layout.children());
        let layout = Layout::new(&node);
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        self.content.as_widget().size_hint()
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::new())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &R,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let state = tree.state.downcast_ref::<State>();
        operation.traverse(&mut |op| {
            let layout = layout.child(0);
            let node = traverse(state.offset, layout.bounds(), layout.children());
            let layout = Layout::new(&node);
            self.content
                .as_widget_mut()
                .operate(&mut tree.children[0], layout, renderer, op);
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();

        if state.is_dragging {
            shell.capture_event();
        }

        {
            let layout = layout.child(0);
            let node = traverse(state.offset, layout.bounds(), layout.children());
            let layout = Layout::new(&node);

            self.content.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }

        if !state.is_dragging && shell.is_event_captured() {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Some(pos) = cursor.position()
                    && (layout.bounds() + state.offset).contains(pos)
                {
                    state.is_dragging = true;
                    state.last_position = Some(pos);
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                state.is_dragging = false;
                state.last_position = None;
            }
            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                if !state.is_dragging {
                    return;
                }
                let Some(last_pos) = state.last_position else {
                    state.last_position = Some(*position);
                    return;
                };

                let diff = *position - last_pos;
                let last_offset = state.offset;
                state.offset += diff;

                if last_offset != state.offset {
                    shell.request_redraw();
                }
                state.last_position = Some(*position);
                dbg!(&state);
            }
            _ => (),
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &R,
    ) -> iced::advanced::mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let inter = {
            let layout = layout.child(0);
            let node = traverse(state.offset, layout.bounds(), layout.children());
            let layout = Layout::new(&node);
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        };

        if state.is_dragging {
            mouse::Interaction::Grabbing
        } else if inter == mouse::Interaction::default()
            && let Some(pos) = cursor.position()
            && (layout.bounds() + state.offset).contains(pos)
        {
            if state.is_dragging {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Grab
            }
        } else {
            inter
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &R,
        viewport: &iced::Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, T, R>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.child(0),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, M, T, R> From<Draggable<'a, M, T, R>> for Element<'a, M, T, R>
where
    M: 'a,
    T: 'a,
    R: Renderer + 'a,
{
    fn from(value: Draggable<'a, M, T, R>) -> Self {
        Element::new(value)
    }
}
