use std::marker::PhantomData;

use iced::{
    Task,
    advanced::widget::{Operation, operate},
    widget::Id,
};

use crate::widget::{scrollie, tabs};

pub fn scroll_to_idx<K: PartialEq + Send + 'static>(id: Id, idx: usize) -> Task<()> {
    struct ScrollToIdx<I> {
        id: Id,
        idx: usize,
        _phantom: PhantomData<I>,
    }

    impl<I: PartialEq + Send + 'static> Operation<()> for ScrollToIdx<I> {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
            operate(self)
        }

        fn custom(
            &mut self,
            id: Option<&iced::widget::Id>,
            _bounds: iced::Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            if Some(&self.id) != id {
                return;
            }

            let Some(state) = state.downcast_mut::<scrollie::State<I>>() else {
                return;
            };

            state.scroll_to_idx(self.idx);
        }
    }

    operate(ScrollToIdx::<K> {
        id,
        idx,
        _phantom: Default::default(),
    })
}

pub fn switch_to_tab<TabId: Send + Clone + Eq + 'static>(id: Id, tab_id: TabId) -> Task<bool> {
    struct SwitchToTab<TabId> {
        id: Id,
        tab_id: TabId,
    }

    impl<TabId: Send + Clone + Eq + 'static> Operation<bool> for SwitchToTab<TabId> {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<bool>)) {
            operate(self)
        }

        fn custom(
            &mut self,
            id: Option<&Id>,
            _bounds: iced::Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            if Some(&self.id) != id {
                return;
            }

            let Some(state) = state.downcast_mut::<tabs::State<TabId>>() else {
                return;
            };

            state.switch_to_tab(self.tab_id.clone());
        }
    }

    operate(SwitchToTab::<TabId> { id, tab_id })
}
