use iced::{
    Border, Color, Element, Event, Length, Padding, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Renderer, Shell, Widget,
        layout::{Limits, Node},
        mouse::{self, Cursor},
        renderer::Quad,
        widget::{Operation, Tree},
    },
    border::Radius,
    theme::{self, palette::Extended},
    widget::svg,
};

pub struct IconButton<'a, M, T: svg::Catalog = iced::Theme> {
    handle: svg::Svg<'a, T>,
    size: Length,
    padding: Padding,
    on_click: Option<M>,
}

impl<'a, M, T> IconButton<'a, M, T>
where
    M: Clone,
    T: svg::Catalog + 'a,
    T::Class<'a>: From<svg::StyleFn<'a, T>>,
{
    pub fn new(handle: svg::Svg<'a, T>) -> Self {
        Self {
            handle,
            size: Length::Shrink,
            padding: Padding::ZERO,
            on_click: None,
        }
    }

    pub fn size(mut self, size: impl Into<Length>) -> Self {
        self.size = size.into();
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn on_click(mut self, message: M) -> Self {
        self.on_click = Some(message);
        self
    }

    pub fn color(mut self, color: impl Into<Color> + Copy + 'a) -> Self {
        self.handle = self.handle.style(move |_, _| svg::Style {
            color: Some(color.into()),
        });
        self
    }
}

impl<'a, M, T, R> Widget<M, T, R> for IconButton<'a, M, T>
where
    M: Clone,
    T: svg::Catalog + theme::Base,
    R: Renderer + iced::advanced::svg::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.size, self.size)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &R, limits: &Limits) -> Node {
        iced::advanced::layout::padded(limits, self.size, self.size, self.padding, |l| {
            <svg::Svg<_> as Widget<M, T, R>>::layout(
                &mut self.handle,
                &mut tree.children[0],
                renderer,
                l,
            )
        })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &T,
        style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let col = theme
            .palette()
            .map(|p| {
                let e = Extended::generate(p);
                if cursor.position_over(layout.bounds()).is_some() {
                    e.background.weak.color
                } else {
                    e.background.weaker.color
                }
            })
            .unwrap_or_else(|| theme.base().background_color);

        renderer.fill_quad(
            Quad {
                bounds: layout.bounds(),
                border: Border {
                    radius: Radius::new(
                        (layout.bounds().height / 2.0).min(layout.bounds().width / 2.0),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            },
            col,
        );

        <svg::Svg<_> as Widget<M, T, R>>::draw(
            &self.handle,
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.child(0),
            cursor,
            viewport,
        );
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::stateless()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        let h: &dyn Widget<M, T, R> = &self.handle;
        vec![Tree::new(h)]
    }

    fn diff(&self, tree: &mut Tree) {
        let h: &dyn Widget<M, T, R> = &self.handle;
        tree.diff_children(&[h]);
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        _layout: Layout<'_>,
        _renderer: &R,
        _operation: &mut dyn Operation,
    ) {
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &R,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        <svg::Svg<_> as Widget<M, T, R>>::update(
            &mut self.handle,
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if cursor.position_over(layout.bounds()).is_some()
            && let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && let Some(on_click) = self.on_click.clone()
        {
            shell.publish(on_click);
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &R,
    ) -> mouse::Interaction {
        if self.on_click.is_some() && cursor.position_over(layout.bounds()).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn overlay<'b>(
        &'b mut self,
        _tree: &'b mut Tree,
        _layout: Layout<'b>,
        _renderer: &R,
        _viewport: &Rectangle,
        _translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'a, M, T, R>> {
        None
    }
}

impl<'a, M, T, R> From<IconButton<'a, M, T>> for Element<'a, M, T, R>
where
    M: Clone + 'a,
    T: svg::Catalog + theme::Base + 'a,
    R: iced::advanced::svg::Renderer + 'a,
{
    fn from(value: IconButton<'a, M, T>) -> Self {
        Element::new(value)
    }
}
