use core::f32;
use std::collections::HashMap;
use std::hash::Hash;

use iced::{
    Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Renderer, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay,
        widget::{Operation, Tree, operation::Scrollable, tree::Tag},
    },
    keyboard,
    widget::Id,
    window,
};

#[derive(Debug, Clone)]
pub struct ScrollViewport {
    pub translation: f32,
    pub bounds: Rectangle,
    pub content_bounds: Rectangle,
}

impl ScrollViewport {
    const SIGMA: f32 = 0.01;

    pub fn is_at_top(&self) -> bool {
        self.translation < Self::SIGMA
    }

    pub fn is_at_bottom(&self) -> bool {
        let edge = self.translation + self.bounds.height;
        edge < self.content_bounds.height + Self::SIGMA
            && edge > self.content_bounds.height - Self::SIGMA
    }
}

pub struct Scrollie<'a, M, T, R, K> {
    children: Vec<Element<'a, M, T, R>>,
    keys: Vec<K>,
    id: Option<Id>,
    width: Length,
    height: Length,
    natural_scrolling: bool,
    on_scroll: Option<Box<dyn Fn(ScrollViewport) -> M + 'a>>,
}

pub fn scrollie<'a, M, T, R, K>(
    children: impl IntoIterator<Item = (impl Into<Element<'a, M, T, R>>, K)>,
) -> Scrollie<'a, M, T, R, K> {
    children.into_iter().map(|(c, k)| (c.into(), k)).collect()
}

impl<'a, M, T, R, K> Scrollie<'a, M, T, R, K> {
    pub fn new(children: Vec<Element<'a, M, T, R>>, keys: Vec<K>) -> Self {
        Self {
            children,
            keys,
            id: None,
            width: Length::Shrink,
            height: Length::Shrink,
            natural_scrolling: false,
            on_scroll: None,
        }
    }

    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    pub fn natural_scrolling(mut self, natural_scrolling: bool) -> Self {
        self.natural_scrolling = natural_scrolling;
        self
    }

    pub fn on_scroll(mut self, on_scroll: impl Fn(ScrollViewport) -> M + 'a) -> Self {
        self.on_scroll = Some(Box::new(on_scroll));
        self
    }

    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }
}

/// Reorders and diffs children using their keys so middle insertions or removals
/// preserve the state of unaffected siblings.
fn keyed_diff<K, W>(
    tree_children: &mut Vec<Tree>,
    keys: &mut Vec<K>,
    new_children: &[W],
    new_keys: &[K],
    diff: impl Fn(&mut Tree, &W),
    new_state: impl Fn(&W) -> Tree,
) where
    K: Eq + Hash + Clone,
{
    let old_children = std::mem::take(tree_children);
    let old_keys = std::mem::take(keys);

    let mut map: HashMap<K, Tree> = old_keys.into_iter().zip(old_children).collect();

    tree_children.clear();
    tree_children.reserve(new_children.len());

    for (widget, key) in new_children.iter().zip(new_keys.iter()) {
        if let Some(mut child_tree) = map.remove(key) {
            diff(&mut child_tree, widget);
            tree_children.push(child_tree);
        } else {
            tree_children.push(new_state(widget));
        }
    }

    keys.clear();
    keys.extend_from_slice(new_keys);
}

impl<'a, M, T, R, K> FromIterator<(Element<'a, M, T, R>, K)> for Scrollie<'a, M, T, R, K> {
    fn from_iter<I: IntoIterator<Item = (Element<'a, M, T, R>, K)>>(iter: I) -> Self {
        let (elems, keys) = iter.into_iter().unzip();
        Self::new(elems, keys)
    }
}

#[derive(Debug)]
pub struct State<K> {
    pub bounds: Rectangle,
    pub content_bounds: Rectangle,
    pub layouts: Vec<(Rectangle, K)>,
    pub keys: Vec<K>,
    translation: f32,
    animation_state: AnimationState,
    last_frame: std::time::Instant,
    /// Whether this was scrolled, either by scrolling or an operation and on_scroll should be called
    scrolled: bool,
}

#[derive(Debug)]
enum AnimationState {
    None,
    Animating { start: f32, target: f32, lerp: f32 },
}

impl<K: PartialEq> State<K> {
    const SIGMA: f32 = 0.01;

    fn new(keys: Vec<K>) -> Self {
        Self {
            bounds: Rectangle::with_size(Size::ZERO),
            content_bounds: Rectangle::with_size(Size::ZERO),
            layouts: Vec::new(),
            keys,
            translation: 0.0,
            animation_state: AnimationState::None,
            last_frame: std::time::Instant::now(),
            scrolled: false,
        }
    }

    fn clamp(&mut self) {
        self.translation = self
            .translation
            .max(0.0)
            .min(self.content_bounds().height - self.bounds.height)
    }

    fn is_at_bottom(&self, bounds: Rectangle, content_bounds: Rectangle) -> bool {
        let edge = self.translation + bounds.height;
        edge < content_bounds.height + Self::SIGMA && edge > content_bounds.height - Self::SIGMA
    }

    pub fn scroll_to_idx(&mut self, idx: usize) {
        let prev_transl = self.translation;
        if let Some(child) = self.layouts.get(idx) {
            self.translation = child.0.y;
            self.clamp();
            if self.translation != prev_transl {
                self.scrolled = true;
            }
        }
    }

    pub fn scroll_to(&mut self, translation: f32) {
        let prev_transl = self.translation;
        self.translation = translation;
        self.clamp();
        if self.translation != prev_transl {
            self.scrolled = true;
        }
    }

    fn is_at_top(&self) -> bool {
        self.translation < Self::SIGMA && self.translation > -Self::SIGMA
    }

    fn content_bounds(&self) -> Rectangle {
        self.content_bounds
    }

    fn current_idx(&self) -> Option<usize> {
        self.layouts
            .iter()
            .enumerate()
            .find(|(_, (l, _))| l.y >= self.translation && l.y + l.height > self.translation)
            .map(|l| l.0)
    }
}

impl<'a, M, T, R, K> Widget<M, T, R> for Scrollie<'a, M, T, R, K>
where
    K: Eq + Hash + Clone + 'static,
    R: Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &R, limits: &Limits) -> Node {
        let span = iced::debug::time("scrollie layout");

        let state = tree.state.downcast_mut::<State<K>>();
        let l = limits.loose();
        let l = &Limits::with_compression(
            l.min(),
            Size::new(l.max().width, f32::INFINITY),
            Size::new(false, true),
        );

        let (children, bounds) = self.children.iter_mut().enumerate().fold(
            (Vec::new(), Rectangle::with_size(Size::ZERO)),
            |(mut c, b), (i, e)| {
                let mut child = e.as_widget_mut().layout(&mut tree.children[i], renderer, l);
                child.translate_mut([0.0, b.height]);
                let b = b.union(&child.bounds());
                c.push(child);
                (c, b)
            },
        );
        let layouts: Vec<(Rectangle, K)> = children
            .iter()
            .map(|c| c.bounds())
            .zip(self.keys.iter().cloned())
            .collect();
        let node = Node::with_children(
            limits
                .resolve(self.width, self.height, bounds.size())
                .min(limits.max()),
            children,
        );

        let was_at_bottom = state.is_at_bottom(state.bounds, state.content_bounds());
        if was_at_bottom {
            state.translation = layouts
                .iter()
                .map(|l| &l.0)
                .fold(Rectangle::with_size(Size::ZERO), |a, b| a.union(b))
                .height
                - node.bounds().height;
        } else if let Some(idx) = state.current_idx() {
            let cur = &state.layouts[idx];
            let (new_idx, _) = layouts
                .iter()
                .enumerate()
                .find(|(_, (_, k))| k == &cur.1)
                .unwrap_or((idx, cur));
            let old_transl = state.translation;
            state.translation = layouts[new_idx].0.y + (state.translation - cur.0.y);
            if let AnimationState::Animating { start, target, .. } = &mut state.animation_state {
                *start += state.translation - old_transl;
                *target += state.translation - old_transl;
            }
        }
        state.layouts = layouts;
        state.content_bounds = bounds;
        state.bounds = node.bounds();
        state.clamp();
        if let AnimationState::Animating { start, target, .. } = &mut state.animation_state {
            let max = (state.content_bounds.height - state.bounds.height).max(0.0);
            *start = start.min(max).max(0.0);
            *target = target.min(max).max(0.0);
        }
        span.finish();
        node
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        theme: &T,
        style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<K>>();
        let bounds = layout.bounds();
        renderer.with_layer(bounds, |r| {
            r.with_translation([0.0, -state.translation].into(), |r| {
                let cursor = match cursor {
                    mouse::Cursor::Available(p) => {
                        mouse::Cursor::Available(p + [0.0, state.translation].into())
                    }
                    mouse::Cursor::Levitating(p) => {
                        mouse::Cursor::Levitating(p + [0.0, state.translation].into())
                    }
                    c => c,
                };
                for ((c, t), l) in self
                    .children
                    .iter()
                    .zip(tree.children.iter())
                    .zip(layout.children())
                {
                    c.as_widget().draw(
                        t,
                        r,
                        theme,
                        style,
                        l,
                        cursor,
                        &Rectangle {
                            x: bounds.x,
                            y: bounds.y + state.translation,
                            ..bounds
                        },
                    );
                }
            });
        });
    }

    fn size_hint(&self) -> Size<Length> {
        self.size()
    }

    fn tag(&self) -> Tag {
        Tag::of::<State<K>>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::<K>::new(self.keys.clone()))
    }

    fn children(&self) -> Vec<Tree> {
        self.children
            .iter()
            .map(|c| Tree::new(c.as_widget()))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State<K>>();

        keyed_diff(
            &mut tree.children,
            &mut state.keys,
            &self.children,
            &self.keys,
            |child_tree, widget| widget.as_widget().diff(child_tree),
            |widget| Tree::new(widget.as_widget()),
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &R,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State<K>>();

        operation.scrollable(
            self.id.as_ref(),
            layout.bounds(),
            layout
                .children()
                .map(|c| c.bounds())
                .fold(Default::default(), |a, b| a.union(&b)),
            [0.0, state.translation].into(),
            state,
        );

        operation.custom(self.id.as_ref(), layout.bounds(), state);

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
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &R,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State<K>>();
        let bounds = layout.bounds();
        for ((c, l), t) in self
            .children
            .iter_mut()
            .zip(layout.children())
            .zip(tree.children.iter_mut())
        {
            let cursor = match cursor {
                mouse::Cursor::Available(p) => {
                    mouse::Cursor::Available(p + [0.0, state.translation].into())
                }
                mouse::Cursor::Levitating(p) => {
                    mouse::Cursor::Levitating(p + [0.0, state.translation].into())
                }
                c => c,
            };
            c.as_widget_mut().update(
                t,
                event,
                l,
                cursor,
                renderer,
                clipboard,
                shell,
                &Rectangle {
                    x: bounds.x,
                    y: bounds.y + state.translation,
                    ..bounds
                },
            );
        }

        if !shell.is_event_captured() {
            let delta = match (cursor.position_in(layout.bounds()).is_some(), event) {
                (
                    true,
                    Event::Mouse(mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Lines { x: _, y },
                    }),
                ) => Some(-y * 80.0),
                (
                    true,
                    Event::Mouse(mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Pixels { x: _, y },
                    }),
                ) => Some(-y),
                (
                    _,
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::PageDown),
                        ..
                    }),
                ) => Some(layout.bounds().height),
                (
                    _,
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::PageUp),
                        ..
                    }),
                ) => Some(-layout.bounds().height),
                _ => None,
            };

            if let Some(mut delta) = delta {
                if self.natural_scrolling {
                    delta = -delta;
                }

                let (lerp, start, target) =
                    if let AnimationState::Animating { target, .. } = state.animation_state {
                        (0.0, state.translation, target + delta)
                    } else {
                        (0.0, state.translation, state.translation + delta)
                    };

                state.animation_state = AnimationState::Animating {
                    lerp,
                    start,
                    target,
                };
                state.last_frame = std::time::Instant::now();
                if layout.bounds().intersects(viewport) {
                    shell.request_redraw();
                }
            }
        }

        if let Event::Window(window::Event::RedrawRequested(i)) = event {
            if let AnimationState::Animating {
                lerp,
                start,
                target,
            } = &mut state.animation_state
            {
                *lerp += i.duration_since(state.last_frame).as_secs_f32() * 30.0;
                *lerp = lerp.clamp(0.0, 1.0);
                let prev_transl = state.translation;
                state.translation = *start + (*lerp * (*target - *start));

                if layout.bounds().intersects(viewport) {
                    shell.request_redraw();
                }

                state.clamp();

                if state.translation != prev_transl {
                    state.scrolled = true;
                }
            }
            if let AnimationState::Animating { lerp, .. } = state.animation_state
                && lerp >= 1.0
            {
                state.animation_state = AnimationState::None
            }
            state.last_frame = *i;
        }

        if state.scrolled {
            state.scrolled = false;
            if let Some(on_scroll) = &self.on_scroll {
                shell.publish(on_scroll(ScrollViewport {
                    translation: state.translation,
                    bounds,
                    content_bounds: state.content_bounds,
                }))
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        renderer: &R,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State<K>>();

        let bounds = layout.bounds();

        let cursor = match cursor {
            mouse::Cursor::Available(p) => {
                mouse::Cursor::Available(p + [0.0, state.translation].into())
            }
            mouse::Cursor::Levitating(p) => {
                mouse::Cursor::Levitating(p + [0.0, state.translation].into())
            }
            c => c,
        };
        for ((c, l), t) in self
            .children
            .iter()
            .zip(layout.children())
            .zip(tree.children.iter())
        {
            let inter = c.as_widget().mouse_interaction(
                t,
                l,
                cursor,
                &Rectangle {
                    x: bounds.x,
                    y: bounds.y + state.translation,
                    ..bounds
                },
                renderer,
            );
            if !matches!(inter, mouse::Interaction::None) {
                return inter;
            }
        }
        mouse::Interaction::default()
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &R,
        _viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'b, M, T, R>> {
        let state = tree.state.downcast_ref::<State<K>>();
        let bounds = layout.bounds();
        let viewport = &Rectangle {
            x: bounds.x,
            y: bounds.y + state.translation,
            ..bounds
        };

        let translation = translation + [0.0, -state.translation].into();

        let overlays: Vec<_> = self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
            .filter_map(|((c, t), l)| {
                c.as_widget_mut()
                    .overlay(t, l, renderer, viewport, translation)
            })
            .collect();

        if overlays.is_empty() {
            None
        } else {
            Some(overlay::Group::with_children(overlays).into())
        }
    }
}

impl<K: PartialEq> Scrollable for State<K> {
    fn snap_to(&mut self, offset: iced::widget::operation::RelativeOffset<Option<f32>>) {
        if let Some(y) = offset.y {
            let bounds = self.content_bounds;
            self.animation_state = AnimationState::None;
            self.scroll_to((bounds.height * y).min(bounds.height).max(0.0));
        }
    }

    fn scroll_to(&mut self, offset: iced::widget::operation::AbsoluteOffset<Option<f32>>) {
        if let Some(y) = offset.y {
            let height = self.layouts.iter().map(|(i, _)| i.height).sum();
            self.animation_state = AnimationState::None;
            self.scroll_to(y.min(height).max(0.0));
        }
    }

    fn scroll_by(
        &mut self,
        offset: iced::widget::operation::AbsoluteOffset,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.animation_state = AnimationState::None;
        self.scroll_to(
            (offset.y + self.translation)
                .min(content_bounds.height - bounds.height)
                .max(0.0),
        );
    }
}

impl<'a, M, T, R, K> From<Scrollie<'a, M, T, R, K>> for Element<'a, M, T, R>
where
    M: 'a,
    T: 'a,
    R: Renderer + 'a,
    K: Clone + Eq + Hash + 'static,
{
    fn from(value: Scrollie<'a, M, T, R, K>) -> Self {
        Element::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::widget::tree::{State as WidgetState, Tag};

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Key {
        A,
        B,
        C,
        X,
    }

    struct MarkerA;
    struct MarkerB;
    struct MarkerC;
    struct MarkerX;

    impl Key {
        fn tag(&self) -> Tag {
            match self {
                Key::A => Tag::of::<MarkerA>(),
                Key::B => Tag::of::<MarkerB>(),
                Key::C => Tag::of::<MarkerC>(),
                Key::X => Tag::of::<MarkerX>(),
            }
        }
    }

    fn make_tree(key: &Key, label: &'static str) -> Tree {
        Tree {
            tag: key.tag(),
            state: WidgetState::new(label.to_string()),
            children: Vec::new(),
        }
    }

    #[test]
    fn keyed_diff_preserves_state_after_middle_insertion() {
        let mut children = vec![
            make_tree(&Key::A, "old-A"),
            make_tree(&Key::B, "old-B"),
            make_tree(&Key::C, "old-C"),
        ];
        let mut keys = vec![Key::A, Key::B, Key::C];

        let new_keys = vec![Key::A, Key::X, Key::B, Key::C];
        let new_children = new_keys.clone();

        keyed_diff(
            &mut children,
            &mut keys,
            &new_children,
            &new_keys,
            |tree, key| {
                if tree.tag != key.tag() {
                    tree.tag = key.tag();
                    tree.state = WidgetState::new(format!("rebuilt-{key:?}"));
                }
            },
            |key| make_tree(key, "new"),
        );

        assert_eq!(children.len(), 4);
        assert_eq!(keys, new_keys);

        assert_eq!(children[0].state.downcast_ref::<String>(), "old-A");
        assert_eq!(children[1].state.downcast_ref::<String>(), "new");
        assert_eq!(children[2].state.downcast_ref::<String>(), "old-B");
        assert_eq!(children[3].state.downcast_ref::<String>(), "old-C");
    }

    #[test]
    fn keyed_diff_preserves_state_after_middle_removal() {
        let mut children = vec![
            make_tree(&Key::A, "old-A"),
            make_tree(&Key::B, "old-B"),
            make_tree(&Key::C, "old-C"),
        ];
        let mut keys = vec![Key::A, Key::B, Key::C];

        let new_keys = vec![Key::A, Key::C];
        let new_children = new_keys.clone();

        keyed_diff(
            &mut children,
            &mut keys,
            &new_children,
            &new_keys,
            |_, _| {},
            |key| make_tree(key, "new"),
        );

        assert_eq!(children.len(), 2);
        assert_eq!(keys, new_keys);

        assert_eq!(children[0].state.downcast_ref::<String>(), "old-A");
        assert_eq!(children[1].state.downcast_ref::<String>(), "old-C");
    }

    #[test]
    fn keyed_diff_preserves_state_on_reorder() {
        let mut children = vec![
            make_tree(&Key::A, "old-A"),
            make_tree(&Key::B, "old-B"),
            make_tree(&Key::C, "old-C"),
        ];
        let mut keys = vec![Key::A, Key::B, Key::C];

        let new_keys = vec![Key::A, Key::C, Key::B];
        let new_children = new_keys.clone();

        keyed_diff(
            &mut children,
            &mut keys,
            &new_children,
            &new_keys,
            |_, _| {},
            |key| make_tree(key, "new"),
        );

        assert_eq!(children.len(), 3);
        assert_eq!(keys, new_keys);

        assert_eq!(children[0].state.downcast_ref::<String>(), "old-A");
        assert_eq!(children[1].state.downcast_ref::<String>(), "old-C");
        assert_eq!(children[2].state.downcast_ref::<String>(), "old-B");
    }
}
