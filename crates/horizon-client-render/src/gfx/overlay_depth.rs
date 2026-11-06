//! Native-sized, attachment-only hand depth for the optional mobile experiment.
//! It is never sampled or loaded by another pass; HDR keeps its ordinary depth.

pub(super) struct OverlayDepth {
    enabled: bool,
    transient: bool,
    target: Option<([u32; 2], wgpu::TextureView)>,
    pub allocations: u64,
    attempts: u64,
}
impl OverlayDepth {
    pub fn new(adapter: &wgpu::Adapter) -> Self {
        let enabled = cfg!(target_os = "ios")
            && std::env::var("HORIZON_IOS_TRANSIENT_DEPTH").as_deref() == Ok("1");
        let transient = adapter.get_info().transient_saves_memory == Some(true)
            && adapter
                .get_texture_format_features(wgpu::TextureFormat::Depth32Float)
                .allowed_usages
                .contains(wgpu::TextureUsages::TRANSIENT_ATTACHMENT);
        Self {
            enabled,
            transient,
            target: None,
            allocations: 0,
            attempts: 0,
        }
    }
    pub fn view(&mut self, device: &wgpu::Device, size: [u32; 2]) -> Option<&wgpu::TextureView> {
        if !self.enabled || !self.transient {
            return None;
        }
        if self.target.as_ref().is_none_or(|(old, _)| *old != size) {
            // Release before allocating the replacement, including screenshot sizes.
            self.target = None;
            self.attempts += 1;
            let Some(view) = create(device, size, true) else {
                // Use the renderer's existing regular depth, without a
                // duplicate fallback allocation or retries every frame.
                self.enabled = false;
                return None;
            };
            self.allocations += 1;
            self.target = Some((size, view));
        }
        self.target.as_ref().map(|(_, view)| view)
    }
}

fn create(device: &wgpu::Device, size: [u32; 2], transient: bool) -> Option<wgpu::TextureView> {
    if size.contains(&0)
        || size
            .iter()
            .any(|&s| s > device.limits().max_texture_dimension_2d)
    {
        return None;
    }
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Native overlay depth"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | if transient {
                wgpu::TextureUsages::TRANSIENT_ATTACHMENT
            } else {
                wgpu::TextureUsages::empty()
            },
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let error = pollster::block_on(memory.pop()).or(pollster::block_on(validation.pop()));
    error.is_none().then_some(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_reuses_size_replaces_on_resize_and_validates_discard_pass() {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(desc);
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("Skipping overlay depth test: no GPU adapter");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        for transient in [false, true] {
            let mut depth = OverlayDepth {
                enabled: true,
                transient,
                target: None,
                allocations: 0,
                attempts: 0,
            };
            let fallback = create(&device, [16, 24], false).unwrap();
            if !transient {
                assert!(depth.view(&device, [16, 24]).is_none());
                assert!(depth.view(&device, [32, 24]).is_none());
                assert_eq!(depth.attempts, 0);
                assert_eq!(depth.allocations, 0);
            }
            let first = depth.view(&device, [16, 24]).unwrap_or(&fallback).clone();
            if depth.allocations == 0 {
                let attempts = depth.attempts;
                assert!(depth.view(&device, [32, 24]).is_none());
                assert_eq!(depth.attempts, attempts);
            } else {
                assert_eq!(depth.view(&device, [16, 24]), Some(&first));
                assert_eq!(depth.allocations, 1);
                assert_ne!(*depth.view(&device, [32, 24]).unwrap(), first);
                assert_eq!(depth.allocations, 2);
            }
            let second = depth.view(&device, [32, 24]).unwrap_or(&fallback);
            let mut encoder = device.create_command_encoder(&Default::default());
            let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: second,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
            }
            queue.submit([encoder.finish()]);
            assert!(pollster::block_on(validation.pop()).is_none());
            assert!(depth.view(&device, [0, 24]).is_none());
            let attempts = depth.attempts;
            assert!(depth.view(&device, [16, 24]).is_none());
            assert_eq!(depth.attempts, attempts);
        }
    }
}
