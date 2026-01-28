use std::sync::OnceLock;

use iced::{
    Element, Event, Length, Rectangle, Size,
    advanced::{
        Layout, Widget,
        graphics::text::cosmic_text::skrifa::raw::tables::layout,
        layout::{Limits, Node},
        widget::Tree,
    },
    window,
};
use image::{AnimationDecoder, GenericImageView};

#[derive(Clone)]
pub struct AnimatedImage {
    first: Frame,
    frames: Vec<Frame>,
    width: Length,
    height: Length,
    duration: std::time::Duration,
    aspect_ratio: f32,
}

#[derive(Debug)]
pub enum AnimatedImageError {
    UnknownFormat,
    UnsupportedFormat,
    NotEnoughFrames,
    Image(image::ImageError),
}

impl std::fmt::Display for AnimatedImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimatedImageError::UnknownFormat => write!(f, "Unknown image format"),
            AnimatedImageError::UnsupportedFormat => write!(f, "Unsupported image format"),
            AnimatedImageError::NotEnoughFrames => {
                write!(f, "Not enough frames (needs at least 1)")
            }
            AnimatedImageError::Image(image_error) => image_error.fmt(f),
        }
    }
}

impl std::error::Error for AnimatedImageError {}

impl From<image::ImageError> for AnimatedImageError {
    fn from(value: image::ImageError) -> Self {
        Self::Image(value)
    }
}

#[derive(Clone)]
struct Frame {
    delay: std::time::Duration,
    handle: iced::advanced::image::Handle,
}

impl AnimatedImage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AnimatedImageError> {
        let format = image::guess_format(bytes).map_err(|_| AnimatedImageError::UnknownFormat)?;
        match format {
            image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::Avif => {
                let img = image::load_from_memory_with_format(bytes, format)?;
                let (width, height) = img.dimensions();
                Ok(Self {
                    first: img.into(),
                    frames: Vec::new(),
                    width: Length::Fixed(width as f32),
                    height: Length::Shrink,
                    duration: std::time::Duration::MAX,
                    aspect_ratio: width as f32 / height as f32,
                })
            }
            image::ImageFormat::Gif => {
                let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes))?;
                Self::from_animation_decoder(decoder)
            }
            image::ImageFormat::WebP => {
                let decoder = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(bytes))?;
                Self::from_animation_decoder(decoder)
            }
            _ => Err(AnimatedImageError::UnsupportedFormat),
        }
    }

    // pub fn from_frames(frames: Vec<Frame>) -> Result<Self, AnimatedImageError> {
    //     let first: Frame = frames.next().ok_or(AnimatedImageError::NotEnoughFrames)??;
    //     let iced::advanced::image::Handle::Rgba { width, height, .. } = first.handle else {
    //         panic!("Expecint RGBA data frame")
    //     };
    //     let frames: Vec<Frame> = frames.filter_map(|f| f.ok()).collect();

    //     let mut duration = first.delay;
    //     duration = frames
    //         .iter()
    //         .map(|f| f.delay)
    //         .fold(duration, |acc, b| acc + b);
    //     Self {
    //         first,
    //         frames,
    //         width: Length::Fixed(width as f32),
    //         height: Length::Shrink,
    //         duration,
    //         aspect_ratio: width as f32 / height as f32,
    //     }
    // }

    fn from_animation_decoder<'a, D: image::AnimationDecoder<'a>>(
        dec: D,
    ) -> Result<Self, AnimatedImageError> {
        let mut frames = dec.into_frames().map(|r| r.map(Into::into));
        let first: Frame = frames.next().ok_or(AnimatedImageError::NotEnoughFrames)??;
        let iced::advanced::image::Handle::Rgba { width, height, .. } = first.handle else {
            unreachable!()
        };
        let frames: Vec<Frame> = frames.filter_map(|f| f.ok()).collect();

        let mut duration = first.delay;
        duration = frames
            .iter()
            .map(|f| f.delay)
            .fold(duration, |acc, b| acc + b);
        Ok(Self {
            first,
            frames,
            width: Length::Fixed(width as f32),
            height: Length::Shrink,
            duration,
            aspect_ratio: width as f32 / height as f32,
        })
    }

    fn durations(&self) -> impl Iterator<Item = std::time::Duration> {
        core::iter::once(self.first.delay).chain(self.frames.iter().map(|f| f.delay))
    }
}

impl From<image::Frame> for Frame {
    fn from(value: image::Frame) -> Self {
        let delay = value.delay().into();
        let handle = iced::widget::image::Handle::from_rgba(
            value.buffer().width(),
            value.buffer().height(),
            value.into_buffer().to_vec(),
        );
        Self { delay, handle }
    }
}

impl From<image::DynamicImage> for Frame {
    fn from(value: image::DynamicImage) -> Self {
        let img = value.to_rgba8();
        let handle =
            iced::widget::image::Handle::from_rgba(img.width(), img.height(), img.to_vec());
        Self {
            delay: std::time::Duration::ZERO,
            handle,
        }
    }
}

struct State {
    frame: usize,
}

impl<M, T, R> Widget<M, T, R> for AnimatedImage
where
    R: iced::advanced::image::Renderer<Handle = iced::advanced::image::Handle>
        + iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &R, limits: &Limits) -> Node {
        match (self.width, self.height) {
            (Length::Shrink, Length::Fixed(h)) => Node::new(Size::new(h * self.aspect_ratio, h)),
            (Length::Fixed(w), Length::Shrink) => Node::new(Size::new(w, w / self.aspect_ratio)),
            (Length::Fixed(w), Length::Fixed(h)) => Node::new(Size::new(w, h)),
            (w, h) => iced::advanced::layout::atomic(limits, w, h),
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut R,
        _theme: &T,
        _style: &iced::advanced::renderer::Style,
        layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let frame = match state.frame {
            0 => Some(&self.first.handle),
            f => self.frames.get(f - 1).map(|f| &f.handle),
        };

        if let Some(frame) = frame {
            renderer.draw_image(
                iced::advanced::image::Image::new(frame).snap(true),
                layout.bounds(),
                layout.bounds(),
            );
        }
    }

    fn size_hint(&self) -> Size<Length> {
        <AnimatedImage as Widget<M, T, R>>::size(self)
    }

    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State { frame: 0 })
    }

    fn children(&self) -> Vec<Tree> {
        Vec::new()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children.clear();
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        _layout: Layout<'_>,
        _renderer: &R,
        _operation: &mut dyn iced::advanced::widget::Operation,
    ) {
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _renderer: &R,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        static FIRST_FRAME: OnceLock<std::time::Instant> = OnceLock::new();

        if !viewport.intersects(&layout.bounds()) || self.frames.is_empty() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        if let Event::Window(window::Event::RedrawRequested(i)) = event {
            let time = i
                .duration_since(*FIRST_FRAME.get_or_init(|| *i))
                .as_secs_f32()
                % self.duration.as_secs_f32();
            let mut delay = std::time::Duration::ZERO;

            let cur_frame = self.durations().enumerate().find(|i| {
                delay += i.1;
                time < delay.as_secs_f32()
            });

            state.frame = cur_frame.map(|f| f.0).unwrap_or(0);
            shell.request_redraw_at(*i + cur_frame.map(|f| f.1).unwrap_or_default());
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &R,
    ) -> iced::advanced::mouse::Interaction {
        iced::advanced::mouse::Interaction::None
    }

    fn overlay<'a>(
        &'a mut self,
        _tree: &'a mut Tree,
        _layout: Layout<'a>,
        _renderer: &R,
        _viewport: &Rectangle,
        _translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'a, M, T, R>> {
        None
    }
}

impl<M, T, R> From<AnimatedImage> for Element<'static, M, T, R>
where
    R: iced::advanced::image::Renderer<Handle = iced::advanced::image::Handle>
        + iced::advanced::Renderer,
{
    fn from(value: AnimatedImage) -> Self {
        Element::new(value)
    }
}
