//! In-process optics over client-owned pixels; Wayland never exposes the desktop.
use super::gpu::{Device, FORMAT, texture};
use crate::{Glass, Image, glass::GlassStyle};
use stillkit_glass as liquid;

pub(super) struct Layer {
    renderer: liquid::Renderer,
    backdrop: wgpu::Texture,
    blit: Blit,
    scene: liquid::Scene,
    container: liquid::ContainerId,
    ids: Vec<liquid::GlassId>,
    last: std::time::Instant,
}
impl Layer {
    pub fn new(gpu: &Device, size: (u32, u32), image: Option<&Image>) -> Self {
        let backdrop = texture(gpu, size, "client glass backdrop");
        let generated;
        let image = if let Some(image) = image {
            image
        } else {
            let mut pixels = Vec::with_capacity(size.0 as usize * size.1 as usize * 4);
            for y in 0..size.1 {
                for x in 0..size.0 {
                    let band = ((x / 48 + y / 48) % 2) * 35;
                    pixels.extend_from_slice(&[
                        (70 + 90 * y / size.1 + band) as u8,
                        (30 + 70 * x / size.0 + band) as u8,
                        (45 + 110 * x / size.0 + band) as u8,
                        255,
                    ]);
                }
            }
            generated = Image {
                width: size.0,
                height: size.1,
                pixels,
            };
            &generated
        };
        // Crop or repeat the client's bitmap to the surface size; no desktop capture.
        let mut pixels = Vec::with_capacity(size.0 as usize * size.1 as usize * 4);
        for y in 0..size.1 {
            for x in 0..size.0 {
                let i = ((y % image.height) * image.width + x % image.width) as usize * 4;
                pixels.extend_from_slice(&image.pixels[i..i + 4]);
            }
        }
        gpu.queue.write_texture(
            backdrop.as_image_copy(),
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.0 * 4),
                rows_per_image: None,
            },
            backdrop.size(),
        );
        let blit = Blit::new(gpu, 1.0);
        let mut scene = liquid::Scene::new();
        let container = scene.add_container(16., 0);
        Self {
            scene,
            container,
            ids: Vec::new(),
            last: std::time::Instant::now(),
            renderer: liquid::Renderer::new(&gpu.device, FORMAT),
            backdrop,
            blit,
        }
    }
    pub fn set(&mut self, glasses: &[Glass], style: &GlassStyle, spacing: f32, scale: f32) {
        self.scene
            .set_container_spacing(self.container, spacing / scale);
        while self.ids.len() > glasses.len() {
            self.scene.remove(self.ids.pop().unwrap());
        }
        for (i, g) in glasses.iter().enumerate() {
            let m = &g.material;
            let mut material = liquid::Material::regular();
            material.refraction = m.refraction;
            material.frost = m.blur / scale;
            material.specular = m.highlight;
            material.brightness = style.brightness;
            material.saturation = style.saturation;
            material.inner_shadow = style.inner_shadow;
            if let Some(depth) = style.depth {
                material.depth = depth;
            }
            if m.tint.a > 0. {
                material.tint = Some(liquid::Color {
                    r: m.tint.r,
                    g: m.tint.g,
                    b: m.tint.b,
                    a: m.tint.a,
                });
            }
            let shape = if style.smoothing > 0. {
                liquid::Shape::Smooth {
                    radius: g.radius / scale,
                    smoothing: style.smoothing,
                }
            } else {
                liquid::Shape::Circular(g.radius / scale)
            };
            let frame = liquid::Rect::new(
                g.body.left / scale,
                g.body.top / scale,
                g.body.width() / scale,
                g.body.height() / scale,
            );
            let appearance = liquid::Appearance {
                dark: true,
                transparency: 1. - m.clarity,
                adapt_to_content: false,
                ..Default::default()
            };
            if let Some(&id) = self.ids.get(i) {
                if self.scene.glass(id).is_some_and(|g| g.frame != frame) {
                    self.scene.set_frame(id, frame);
                }
                self.scene.set_shape(id, shape);
                self.scene.set_material(id, material);
                self.scene.set_appearance(id, Some(appearance));
            } else {
                self.ids.push(
                    self.scene.add(
                        liquid::Glass::new(frame)
                            .container(self.container)
                            .shape(shape)
                            .material(material)
                            .appearance(appearance),
                    ),
                );
            }
        }
    }
    pub fn render(
        &mut self,
        gpu: &Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        content: &wgpu::Texture,
        size: (u32, u32),
        scale: f32,
    ) -> bool {
        let now = std::time::Instant::now();
        let animating = self.scene.update((now - self.last).as_secs_f32().min(0.1));
        self.last = now;
        self.renderer.render(
            &gpu.device,
            &gpu.queue,
            encoder,
            &liquid::Frame {
                content: &self.backdrop.create_view(&Default::default()),
                target,
                size,
                scale,
                content_changed: true,
                copy_content: true,
            },
            &self.scene,
        );
        self.blit.draw(gpu, encoder, target, content, true);
        animating
    }
}

pub(super) struct Blit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}
impl Blit {
    pub fn new(gpu: &Device, alpha: f32) -> Self {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("stillkit content over glass"),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Owned(
                    r#"
@group(0) @binding(0) var content: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4f {
 let p=array<vec2f,3>(vec2f(-1.,-1.),vec2f(3.,-1.),vec2f(-1.,3.));
 return vec4f(p[i],0.,1.);
}
@fragment fn fs(@builtin(position) p:vec4f) -> @location(0) vec4f {
 return textureLoad(content,vec2i(p.xy),0) * OPACITY;
}"#
                    .replace("OPACITY", &format!("{alpha:.8}")),
                )),
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("content over glass"),
                layout: Some(&pipeline_layout),
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
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        Self { pipeline, layout }
    }
    pub fn draw(
        &self,
        gpu: &Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        content: &wgpu::Texture,
        over: bool,
    ) {
        let content = content.create_view(&Default::default());
        let bindings = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&content),
            }],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("stillkit foreground"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if over {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}
