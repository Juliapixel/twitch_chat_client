use iced::{
    Task,
    advanced::widget::{Operation, operate},
    widget::Id,
};

use crate::widget::scrollie;

pub fn scroll_to_idx(id: Id, idx: usize) -> Task<()> {
    struct ScrollToIdx {
        id: Id,
        idx: usize,
    }

    impl Operation<()> for ScrollToIdx {
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

            let Some(state) = state.downcast_mut::<scrollie::State>() else {
                return;
            };

            state.scroll_to_idx(self.idx);
        }
    }

    operate(ScrollToIdx { id, idx })
}
