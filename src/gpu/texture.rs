use {
    super::driver::{Driver, Image2d, ImageView, PhysicalDevice},
    crate::math::Extent,
    gfx_hal::{
        command::CommandBuffer,
        format::{Aspects, Format, Swizzle},
        image::{Access, Layout, SubresourceRange, Tiling, Usage, ViewKind},
        memory::{Barrier, Dependencies},
        pso::PipelineStage,
        Backend,
    },
    gfx_impl::Backend as _Backend,
    std::{
        cell::{Ref, RefCell},
        collections::HashMap,
        fmt::{Debug, Error, Formatter},
        ops::Deref,
    },
};

pub(crate) struct Image(<_Backend as Backend>::Image);

#[cfg(debug_assertions)]
use gfx_hal::device::Device as _;

impl AsRef<<_Backend as Backend>::Image> for Image {
    fn as_ref(&self) -> &<_Backend as Backend>::Image {
        &self.0
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct ImageViewKey {
    view_kind: ViewKind,
    format: Format,
    swizzle: Swizzle,
    range: SubresourceRange,
}

struct State {
    access_mask: Access,
    layout: Layout,
    pipeline_stage: PipelineStage,
}

/// A generic structure which can hold an N dimensional GPU texture.
pub struct Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    dims: Extent,
    driver: Driver,
    format: Format,
    image: I,
    state: RefCell<State>,
    views: RefCell<HashMap<ImageViewKey, ImageView>>,
}

impl Texture<Image> {
    pub(crate) fn from_swapchain(
        image: <_Backend as Backend>::Image,
        driver: &Driver,
        dims: Extent,
        format: Format,
    ) -> Self {
        Self {
            dims,
            driver: Driver::clone(driver),
            format,
            image: Image(image),
            state: RefCell::new(State {
                access_mask: Access::empty(),
                layout: Layout::Undefined,
                pipeline_stage: PipelineStage::TOP_OF_PIPE, // TODO: Was BOTTOM_ in vlb. What to do?
            }),
            views: Default::default(),
        }
    }

    /// # Safety
    /// None
    pub(crate) unsafe fn acquire_swapchain(&mut self) {
        let mut state = self.state.borrow_mut();
        state.access_mask = Access::empty();
        state.layout = Layout::Undefined;
        state.pipeline_stage = PipelineStage::TOP_OF_PIPE;

        #[cfg(debug_assertions)]
        self.driver
            .as_ref()
            .borrow()
            .set_image_name(&mut self.image.0, "Swapchain image");
    }
}

impl<I> Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    pub(crate) fn as_default_2d_view(&self) -> ImageViewRef {
        self.as_default_2d_view_format(self.format())
    }

    pub(crate) fn as_default_2d_view_format(&self, format: Format) -> ImageViewRef {
        self.as_view(
            ViewKind::D2,
            format,
            Default::default(),
            SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            },
        )
    }

    pub(crate) fn as_view(
        &self,
        view_kind: ViewKind,
        format: Format,
        swizzle: Swizzle,
        range: SubresourceRange,
    ) -> ImageViewRef {
        let key = ImageViewKey {
            view_kind,
            format,
            swizzle,
            range: range.clone(),
        };

        {
            let mut views = self.views.borrow_mut();
            if !views.contains_key(&key) {
                let view = ImageView::new(
                    Driver::clone(&self.driver),
                    self.image.as_ref(),
                    view_kind,
                    format,
                    swizzle,
                    range,
                );
                views.insert(key.clone(), view);
            }
        }

        ImageViewRef {
            key,
            views: self.views.borrow(),
        }
    }

    pub fn dims(&self) -> Extent {
        self.dims
    }

    pub(crate) fn format(&self) -> Format {
        self.format
    }

    /// # Safety
    /// None
    /// TODO: Swap order of last two params, better name, layout_barrier?
    pub(crate) unsafe fn set_layout(
        &mut self,
        cmd_buf: &mut <_Backend as Backend>::CommandBuffer,
        layout: Layout,
        pipeline_stage: PipelineStage,
        access_mask: Access,
    ) {
        let mut state = self.state.borrow_mut();
        cmd_buf.pipeline_barrier(
            state.pipeline_stage..pipeline_stage,
            Dependencies::empty(),
            &[Barrier::Image {
                states: (state.access_mask, state.layout)..(access_mask, layout),
                target: self.image.as_ref(),
                families: None,
                range: SubresourceRange {
                    aspects: if self.format.is_depth() {
                        Aspects::DEPTH
                    } else {
                        Aspects::COLOR
                    },
                    levels: 0..1,
                    layers: 0..1,
                },
            }],
        );

        state.access_mask = access_mask;
        state.layout = layout;
        state.pipeline_stage = pipeline_stage;
    }
}

impl Texture<Image2d> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        #[cfg(debug_assertions)] name: &str,
        driver: Driver,
        dims: Extent,
        mut desired_tiling: Tiling,
        desired_format: Format,
        layout: Layout,
        usage: Usage,
        layers: u16,
        samples: u8,
        mips: u8,
    ) -> Self {
        let format = {
            let device = driver.as_ref().borrow();
            device
                .best_fmt(desired_format, desired_tiling, usage)
                .unwrap_or_else(|| {
                    desired_tiling = Tiling::Linear;
                    device
                        .best_fmt(desired_format, desired_tiling, usage)
                        .unwrap()
                })
        };
        let access_mask = if layout == Layout::Preinitialized {
            Access::HOST_WRITE
        } else {
            Access::empty()
        };
        let image = Image2d::new(
            #[cfg(debug_assertions)]
            name,
            Driver::clone(&driver),
            dims,
            layers,
            samples,
            mips,
            format,
            desired_tiling,
            usage,
        );

        Self {
            dims,
            driver,
            format,
            image,
            state: RefCell::new(State {
                access_mask,
                layout,
                pipeline_stage: PipelineStage::TOP_OF_PIPE, // TODO: Was BOTTOM_ in vlb. What to do?
            }),
            views: Default::default(),
        }
    }
}

impl<I> AsMut<<_Backend as Backend>::Image> for Texture<I>
where
    I: AsMut<<_Backend as Backend>::Image> + AsRef<<_Backend as Backend>::Image>,
{
    fn as_mut(&mut self) -> &mut <_Backend as Backend>::Image {
        self.image.as_mut()
    }
}

impl<I> AsRef<<_Backend as Backend>::Image> for Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn as_ref(&self) -> &<_Backend as Backend>::Image {
        self.image.as_ref()
    }
}

impl<I> Debug for Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.write_str("Texture")
    }
}

impl<I> Drop for Texture<I>
where
    I: AsRef<<_Backend as Backend>::Image>,
{
    fn drop(&mut self) {
        // TODO: Do we *need* to drop the views before the image?
        self.views.borrow_mut().clear();
    }
}

pub struct ImageViewRef<'a> {
    key: ImageViewKey,
    views: Ref<'a, HashMap<ImageViewKey, ImageView>>,
}

impl<'a> Deref for ImageViewRef<'a> {
    type Target = ImageView;

    fn deref(&self) -> &Self::Target {
        &self.views[&self.key]
    }
}