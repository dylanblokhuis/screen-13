mod data;

/// A collection of smart-pointer types used internally to operate the GFX-HAL API.
mod driver;

mod model;

/// A collection of operation implementations used internally to fulfill the Render API.
mod op;

/// A collection of resource pool types used internally to cache GFX-HAL types.
mod pool;

mod render;
mod swapchain;
mod texture;

pub use self::{
    model::{Model, Pose},
    op::{Bitmap, Command, Font, Material, Write, WriteMode},
    render::Render,
    swapchain::Swapchain,
    texture::Texture,
};

pub(crate) use self::{
    driver::{Driver, PhysicalDevice},
    op::Op,
};

use {
    self::{
        data::{Data, Mapping},
        driver::{Device, Image2d, Surface},
        op::BitmapOp,
        pool::{Lease, Pool},
    },
    crate::{math::Extent, pak::Pak, Error},
    gfx_hal::{
        adapter::Adapter, buffer::Usage, device::Device as _, format::Format, queue::QueueFamily,
        window::Surface as _, Instance as _,
    },
    gfx_impl::{Backend as _Backend, Instance},
    std::{
        cell::RefCell,
        fmt::Debug,
        io::{Read, Seek},
        rc::Rc,
    },
    winit::window::Window,
};

#[cfg(debug_assertions)]
use {
    num_format::{Locale, ToFormattedString},
    std::time::Instant,
};

// TODO: Make configurable!
pub const MULTISAMPLE_COUNT: u8 = 4;
// const DIRECTIONAL_SHADOW_BUFFERS: usize = 1;
// const SPOT_SHADOW_BUFFERS: usize = 8;

/// A two dimensional rendering result.
pub type Texture2d = TextureRef<Image2d>;

pub type BitmapRef = Rc<Bitmap>;
pub type ModelRef = Rc<Model>;

pub(crate) type PoolRef = Rc<RefCell<Pool>>;
pub(crate) type TextureRef<I> = Rc<RefCell<Texture<I>>>;

type OpCache = RefCell<Option<Vec<Box<dyn Op>>>>;

fn create_instance() -> (Adapter<_Backend>, Instance) {
    let instance = Instance::create("attackgoat/screen-13", 1).unwrap();
    let mut adapters = instance.enumerate_adapters();
    if adapters.is_empty() {
        // TODO: Error::adapter
    }
    let adapter = adapters.remove(0);
    (adapter, instance)
}

// TODO: Different path for webgl and need this -> #[cfg(any(feature = "vulkan", feature = "metal"))]
fn create_surface(window: &Window) -> (Adapter<_Backend>, Surface) {
    let (adapter, instance) = create_instance();
    let surface = Surface::new(instance, window).unwrap();
    (adapter, surface)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlendMode {
    Add,
    Alpha,
    ColorBurn,
    ColorDodge,
    Color,
    Darken,
    DarkenColor,
    Difference,
    Divide,
    Exclusion,
    HardLight,
    HardMix,
    LinearBurn,
    Multiply,
    Normal,
    Overlay,
    Screen,
    Subtract,
    VividLight,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

/// Allows you to load resources and begin rendering operations.
pub struct Gpu {
    driver: Driver,
    ops: OpCache,
    pool: PoolRef,
}

impl Gpu {
    pub(super) fn new(window: &Window) -> (Self, Surface) {
        let (adapter, surface) = create_surface(window);
        let queue = adapter
            .queue_families
            .iter()
            .find(|family| {
                let ty = family.queue_type();
                surface.supports_queue_family(family)
                    && ty.supports_graphics()
                    && ty.supports_compute()
            })
            .ok_or_else(Error::graphics_queue_family)
            .unwrap();
        let driver = Driver::new(RefCell::new(
            Device::new(adapter.physical_device, queue).unwrap(),
        ));
        let pool = PoolRef::new(RefCell::new(Pool::new(&driver, Format::Rgba8Unorm)));

        (
            Self {
                driver,
                ops: Default::default(),
                pool,
            },
            surface,
        )
    }

    // TODO: This is a useful function, but things you end up rendering with it cannot be used with the window's presentation
    // surface. Maybe change the way this whole thing works. Or document better?
    pub fn offscreen() -> Self {
        let (adapter, _) = create_instance();
        let queue = adapter
            .queue_families
            .iter()
            .find(|family| {
                let ty = family.queue_type();
                ty.supports_graphics() && ty.supports_compute()
            })
            .ok_or_else(Error::graphics_queue_family)
            .unwrap();
        let driver = Driver::new(RefCell::new(
            Device::new(adapter.physical_device, queue).unwrap(),
        ));
        let pool = PoolRef::new(RefCell::new(Pool::new(&driver, Format::Rgba8Unorm)));

        Self {
            driver,
            ops: Default::default(),
            pool,
        }
    }

    // TODO: This should not be exposed, bring users into this code?
    pub(crate) fn driver(&self) -> &Driver {
        &self.driver
    }

    pub fn load_animation<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] _name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> ModelRef {
        #[cfg(debug_assertions)]
        debug!("Loading animation `{}`", key.as_ref());

        let _pool = PoolRef::clone(&self.pool);
        let _anim = pak.read_animation(key.as_ref());
        // let indices = model.indices();
        // let index_buf_len = indices.len() as _;
        // let mut index_buf = pool.borrow_mut().data_usage(
        //     #[cfg(debug_assertions)]
        //     name,
        //     index_buf_len,
        //     Usage::INDEX,
        // );

        // {
        //     let mut mapped_range = index_buf.map_range_mut(0..index_buf_len).unwrap();
        //     mapped_range.copy_from_slice(&indices);
        //     Mapping::flush(&mut mapped_range).unwrap();
        // }

        // let vertices = model.vertices();
        // let vertex_buf_len = vertices.len() as _;
        // let mut vertex_buf = pool.borrow_mut().data_usage(
        //     #[cfg(debug_assertions)]
        //     name,
        //     vertex_buf_len,
        //     Usage::VERTEX,
        // );

        // {
        //     let mut mapped_range = vertex_buf.map_range_mut(0..vertex_buf_len).unwrap();
        //     mapped_range.copy_from_slice(&vertices);
        //     Mapping::flush(&mut mapped_range).unwrap();
        // }

        // let model = Model::new(
        //     model.meshes().map(Clone::clone).collect(),
        //     index_buf,
        //     vertex_buf,
        // );

        // ModelRef::new(model)
        todo!()
    }

    pub fn load_bitmap<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> BitmapRef {
        #[cfg(debug_assertions)]
        debug!("Loading bitmap `{}`", key.as_ref());

        let bitmap = pak.read_bitmap(key.as_ref());
        let pool = PoolRef::clone(&self.pool);
        let bitmap = unsafe {
            BitmapOp::new(
                #[cfg(debug_assertions)]
                name,
                &pool,
                &bitmap,
                Format::Rgba8Unorm,
            )
            .record()
        };

        BitmapRef::new(bitmap)
    }

    /// Only bitmapped fonts are supported.
    pub fn load_font<F: AsRef<str>, R: Read + Seek>(&self, pak: &mut Pak<R>, face: F) -> Font {
        #[cfg(debug_assertions)]
        debug!("Loading font `{}`", face.as_ref());

        let pool = PoolRef::clone(&self.pool);
        Font::load(&pool, pak, face.as_ref(), Format::Rgba8Unorm)
    }

    pub fn load_model<K: AsRef<str>, R: Read + Seek>(
        &self,
        #[cfg(debug_assertions)] name: &str,
        pak: &mut Pak<R>,
        key: K,
    ) -> ModelRef {
        #[cfg(debug_assertions)]
        debug!("Loading model `{}`", key.as_ref());

        let pool = PoolRef::clone(&self.pool);
        let model = pak.read_model(key.as_ref());
        let indices = model.indices();
        let index_buf_len = indices.len() as _;
        let mut index_buf = pool.borrow_mut().data_usage(
            #[cfg(debug_assertions)]
            name,
            index_buf_len,
            Usage::INDEX,
        );

        {
            let mut mapped_range = index_buf.map_range_mut(0..index_buf_len).unwrap();
            mapped_range.copy_from_slice(&indices);
            Mapping::flush(&mut mapped_range).unwrap();
        }

        let vertices = model.vertices();
        let vertex_buf_len = vertices.len() as _;
        let mut vertex_buf = pool.borrow_mut().data_usage(
            #[cfg(debug_assertions)]
            name,
            vertex_buf_len,
            Usage::VERTEX,
        );

        {
            let mut mapped_range = vertex_buf.map_range_mut(0..vertex_buf_len).unwrap();
            mapped_range.copy_from_slice(&vertices);
            Mapping::flush(&mut mapped_range).unwrap();
        }

        let model = Model::new(
            pool,
            model.take_meshes(),
            index_buf,
            vertex_buf,
        );

        ModelRef::new(model)
    }

    // TODO: This should not be exposed, bring users into this code?
    pub(crate) fn pool(&self) -> &PoolRef {
        &self.pool
    }

    pub fn render(&self, #[cfg(debug_assertions)] name: &str, dims: Extent) -> Render {
        // There may be pending operations from a previously resolved render; if so
        // we just stick them into the next render that goes out the door.
        let ops = if let Some(ops) = self.ops.borrow_mut().take() {
            ops
        } else {
            Default::default()
        };

        Render::new(
            #[cfg(debug_assertions)]
            name,
            &self.pool,
            dims,
            Format::Rgba8Unorm,
            ops,
        )
    }

    /// Resolves a render into a texture which can be written to other renders.
    pub fn resolve(&self, render: Render) -> Lease<Texture2d> {
        let (target, ops) = render.resolve();
        let mut cache = self.ops.borrow_mut();
        if let Some(cache) = cache.as_mut() {
            cache.extend(ops);
        } else {
            cache.replace(ops);
        }

        target
    }

    pub(crate) fn wait_idle(&self) {
        #[cfg(debug_assertions)]
        let started = Instant::now();

        // We are required to wait for the GPU to finish what we submitted before dropping the driver
        self.driver.borrow().wait_idle().unwrap();

        #[cfg(debug_assertions)]
        {
            let elapsed = Instant::now() - started;
            debug!(
                "Wait for GPU idle took {}ms",
                elapsed.as_millis().to_formatted_string(&Locale::en)
            );
        }
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        self.wait_idle();
    }
}
