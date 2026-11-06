//! Pipelines kept between the renderers built on one device. The start builds
//! three (the boot screen's, the title artwork's, the world's) and a resource
//! reload or a graphics option builds another; each compiled every shader
//! module and pipeline again, though most come out identical (the boot and
//! title renderers' entirely, the world's entity pipelines). On a driver that
//! compiles slowly (a cold shader cache, llvmpipe, a weak CPU) that was seconds
//! of the main thread before the title showed.
//!
//! An object is kept under its descriptor's `Debug` text: wgpu prints the
//! labels, sources, constants, formats and states, and the handles a descriptor
//! names (shader modules, layouts) by their ids, so a descriptor that names a
//! kept handle matches only the same handle. Kept objects stay alive, so their
//! ids are never reused while they are keys.
use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

/// One map of kept objects with the generation each was last asked for in.
struct Kept<T, K = String> {
    map: Mutex<HashMap<K, (T, u32)>>,
}
impl<T, K> Default for Kept<T, K> {
    fn default() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }
}
impl<T: Clone, K: Eq + std::hash::Hash> Kept<T, K> {
    /// The object kept under `key`, else `make`'s, kept. The lock is not
    /// held while `make` compiles.
    fn get(&self, key: K, generation: u32, make: impl FnOnce() -> T) -> T {
        if let Some((object, used)) = self
            .map
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(&key)
        {
            *used = generation;
            return object.clone();
        }
        let object = make();
        self.map
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, (object.clone(), generation));
        object
    }
    fn keep_only(&self, generation: u32) {
        self.map
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, (_, used)| *used == generation);
    }
    fn len(&self) -> usize {
        self.map
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// The shader modules, layouts and pipelines of the device's renderers.
#[derive(Default)]
pub struct Reuse {
    generation: std::sync::atomic::AtomicU32,
    shaders: Kept<wgpu::ShaderModule, DescriptorKey>,
    bind_group_layouts: Kept<wgpu::BindGroupLayout, DescriptorKey>,
    pipeline_layouts: Kept<wgpu::PipelineLayout, DescriptorKey>,
    render: Kept<wgpu::RenderPipeline, DescriptorKey>,
}
/// Device identity is part of every key, even for descriptors such as a
/// shader source or bind-group layout that contain no GPU handles themselves.
type DescriptorKey = (wgpu::Device, String);

impl Reuse {
    /// A renderer starts building: what it asks for is kept, the rest is let
    /// go by [`Reuse::finish`].
    pub fn begin(&self) -> u32 {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .wrapping_add(1)
    }
    /// The renderer of `generation` is built: objects only older renderers
    /// used are dropped here (the renderers still alive hold their own).
    pub fn finish(&self, generation: u32) {
        self.shaders.keep_only(generation);
        self.bind_group_layouts.keep_only(generation);
        self.pipeline_layouts.keep_only(generation);
        self.render.keep_only(generation);
    }
    /// How many objects are kept.
    pub fn len(&self) -> usize {
        self.shaders.len()
            + self.bind_group_layouts.len()
            + self.pipeline_layouts.len()
            + self.render.len()
    }
    fn current(&self) -> u32 {
        self.generation.load(std::sync::atomic::Ordering::Relaxed)
    }
    /// `device`'s `create_*` calls through this cache.
    pub fn on<'a>(&'a self, device: &'a wgpu::Device) -> On<'a> {
        On {
            reuse: self,
            device,
            generation: self.current(),
        }
    }
}

/// A device whose shader modules, layouts and render pipelines come from a
/// [`Reuse`] when one with the same descriptor is kept.
pub struct On<'a> {
    reuse: &'a Reuse,
    device: &'a wgpu::Device,
    generation: u32,
}
impl On<'_> {
    pub fn create_shader_module(
        &self,
        descriptor: wgpu::ShaderModuleDescriptor<'_>,
    ) -> wgpu::ShaderModule {
        let key = (self.device.clone(), format!("{descriptor:?}"));
        self.reuse.shaders.get(key, self.generation, || {
            self.device.create_shader_module(descriptor)
        })
    }
    pub fn create_bind_group_layout(
        &self,
        descriptor: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> wgpu::BindGroupLayout {
        let key = (self.device.clone(), format!("{descriptor:?}"));
        self.reuse.bind_group_layouts.get(key, self.generation, || {
            self.device.create_bind_group_layout(descriptor)
        })
    }
    pub fn create_pipeline_layout(
        &self,
        descriptor: &wgpu::PipelineLayoutDescriptor<'_>,
    ) -> wgpu::PipelineLayout {
        let key = (self.device.clone(), format!("{descriptor:?}"));
        self.reuse.pipeline_layouts.get(key, self.generation, || {
            self.device.create_pipeline_layout(descriptor)
        })
    }
    pub fn create_render_pipeline(
        &self,
        descriptor: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> wgpu::RenderPipeline {
        let key = (self.device.clone(), format!("{descriptor:?}"));
        self.reuse.render.get(key, self.generation, || {
            self.device.create_render_pipeline(descriptor)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Program {
        shader: wgpu::ShaderModule,
        bindings: wgpu::BindGroupLayout,
        layout: wgpu::PipelineLayout,
        pipeline: wgpu::RenderPipeline,
    }
    const SHADER: &str = "@group(0) @binding(0) var<uniform> tint: vec4<f32>;
        @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            return vec4<f32>(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 0.0, 1.0);
        }
        @fragment fn fs() -> @location(0) vec4<f32> { return tint; }";
    fn program(on: &On<'_>, format: wgpu::TextureFormat) -> Program {
        let shader = on.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("reuse test"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bindings = on.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("reuse test"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = on.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("reuse test"),
            bind_group_layouts: &[Some(&bindings)],
            ..Default::default()
        });
        let pipeline = on.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("reuse test"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Program {
            shader,
            bindings,
            layout,
            pipeline,
        }
    }
    fn pixel(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        program: &Program,
        tint: [f32; 4],
    ) -> [u8; 4] {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("distinct mutable tint"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform, 0, bytemuck::cast_slice(&tint));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &program.bindings,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&program.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: Default::default(),
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        read.slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let color: [u8; 4] = read.slice(..).get_mapped_range().unwrap()[..4]
            .try_into()
            .unwrap();
        read.unmap();
        color
    }

    #[test]
    fn device_and_descriptor_boundaries_keep_pixels_and_mutable_uniforms_distinct() {
        let instance = crate::gfx::test_gpu::instance();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("Skipping pipeline reuse test: no GPU adapter");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let reuse = Reuse::default();
        let generation = reuse.begin();
        let first = program(&reuse.on(&device), wgpu::TextureFormat::Rgba8Unorm);
        reuse.finish(generation);
        let generation = reuse.begin();
        let second = program(&reuse.on(&device), wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(first.shader, second.shader);
        assert_eq!(first.bindings, second.bindings);
        assert_eq!(first.layout, second.layout);
        assert_eq!(first.pipeline, second.pipeline);
        assert_eq!(
            pixel(&device, &queue, &first, [1.0, 0.0, 0.0, 1.0]),
            [255, 0, 0, 255]
        );
        assert_eq!(
            pixel(&device, &queue, &second, [0.0, 1.0, 0.0, 1.0]),
            [0, 255, 0, 255]
        );
        assert_eq!(
            pixel(&device, &queue, &first, [1.0, 0.0, 0.0, 1.0]),
            [255, 0, 0, 255]
        );
        let changed = program(&reuse.on(&device), wgpu::TextureFormat::Bgra8Unorm);
        assert_ne!(changed.pipeline, first.pipeline);
        let changed_source = reuse
            .on(&device)
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("reuse test"),
                source: wgpu::ShaderSource::Wgsl(
                    SHADER.replace("return tint;", "return tint * 0.5;").into(),
                ),
            });
        assert_ne!(changed_source, first.shader);
        let (other, other_queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let other_program = program(&reuse.on(&other), wgpu::TextureFormat::Rgba8Unorm);
        assert_ne!(other_program.shader, first.shader);
        assert_ne!(other_program.bindings, first.bindings);
        assert_ne!(other_program.layout, first.layout);
        assert_ne!(other_program.pipeline, first.pipeline);
        assert_eq!(
            pixel(&other, &other_queue, &other_program, [0.0, 0.0, 1.0, 1.0]),
            [0, 0, 255, 255]
        );
        reuse.finish(generation);
    }

    /// A key asked for again gets the kept object without making one; what
    /// the latest generation did not ask for is let go when it finishes.
    #[test]
    fn kept_objects_are_shared_until_a_build_stops_asking() {
        let kept = Kept::<u32>::default();
        let made = std::cell::Cell::new(0);
        let make = |value| {
            made.set(made.get() + 1);
            value
        };
        assert_eq!(kept.get("a".into(), 1, || make(10)), 10);
        assert_eq!(kept.get("b".into(), 1, || make(20)), 20);
        kept.keep_only(1);
        assert_eq!(kept.len(), 2);
        // The next build asks for "a" only.
        assert_eq!(kept.get("a".into(), 2, || make(11)), 10);
        kept.keep_only(2);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept.get("b".into(), 3, || make(21)), 21);
        assert_eq!(made.get(), 3);
    }
}
