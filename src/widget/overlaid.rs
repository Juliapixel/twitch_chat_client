use iced::{
    Element, Length, Rectangle, Size,
    advanced::{
        Layout, Renderer, Widget,
        layout::{Limits, Node},
        mouse, overlay,
        widget::Tree,
    },
};

/// Overlays a set of elements in such a way that they are centered inside the
/// largest of them and rendered from first to last
pub struct Overlaid<'a, M, T, R> {
    children: Vec<Element<'a, M, T, R>>,
}

impl<'a, M, T, R> Overlaid<'a, M, T, R> {
    pub fn new(children: Vec<Element<'a, M, T, R>>) -> Self {
        Self { children }
    }
}

impl<'a, M, T, R> Widget<M, T, R> for Overlaid<'a, M, T, R>
where
    R: Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &R, limits: &Limits) -> Node {
        let mut children: Vec<Node> = self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .map(|(c, t)| c.as_widget_mut().layout(t, renderer, limits))
            .collect();
        let bounds = children
            .iter()
            .map(|c| c.bounds())
            .reduce(|a, b| a.union(&b))
            .unwrap_or_default();

        let Size { width, height } = bounds.size();
        children.iter_mut().for_each(|c| {
            c.translate_mut([
                (width - c.bounds().width) / 2.0,
                (height - c.bounds().height) / 2.0,
            ])
        });
        Node::with_children(bounds.size(), children)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &T,
        style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((c, t), l) in self
            .children
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            c.as_widget()
                .draw(t, renderer, theme, style, l, cursor, viewport)
        }
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::stateless()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        self.children
            .iter()
            .map(|c| Tree::new(c.as_widget()))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |op| {
            for ((c, t), l) in self
                .children
                .iter_mut()
                .zip(tree.children.iter_mut())
                .zip(layout.children())
            {
                c.as_widget_mut().operate(t, l, renderer, op);
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        for ((c, t), l) in self
            .children
            .iter_mut()
            .rev()
            .zip(tree.children.iter_mut().rev())
            .zip(layout.children().rev())
        {
            c.as_widget_mut()
                .update(t, event, l, cursor, renderer, clipboard, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &R,
    ) -> mouse::Interaction {
        for ((c, t), l) in self
            .children
            .iter()
            .rev()
            .zip(tree.children.iter().rev())
            .zip(layout.children().rev())
        {
            let inter = c
                .as_widget()
                .mouse_interaction(t, l, cursor, viewport, renderer);
            if inter != mouse::Interaction::default() {
                return inter;
            }
        }
        mouse::Interaction::None
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &R,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, T, R>> {
        let mut g = Vec::new();
        for ((c, t), l) in self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            if let Some(o) = c
                .as_widget_mut()
                .overlay(t, l, renderer, viewport, translation)
            {
                g.push(o);
            }
        }
        if !g.is_empty() {
            Some(overlay::Group::with_children(g).overlay())
        } else {
            None
        }
    }
}

impl<'a, M, T, R> From<Overlaid<'a, M, T, R>> for Element<'a, M, T, R>
where
    M: 'a,
    T: 'a,
    R: Renderer + 'a,
{
    fn from(value: Overlaid<'a, M, T, R>) -> Self {
        Element::new(value)
    }
}
