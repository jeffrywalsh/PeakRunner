use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::wgpu;
use eframe::egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};
use glam::{Mat4, Vec3};

use crate::drawlist::{normal_columns, DrawFrame, EmitDraw, LitDraw, MeshId};
use crate::terrain;

const SLOT: u64 = 256;
const WORLD_SIZE: u64 = 240;
const SKY_SIZE: u64 = 96;
const EMIT_SIZE: u64 = 80;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WorldUniform {
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    n0: [f32; 4],
    n1: [f32; 4],
    n2: [f32; 4],
    cam: [f32; 3],
    emit: f32,
    sun: [f32; 3],
    mode: f32,
    fog: [f32; 3],
    pad0: f32,
    color: [f32; 3],
    pad1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkyUniform {
    inv_vp: [[f32; 4]; 4],
    cam: [f32; 3],
    pad0: f32,
    sun: [f32; 3],
    pad1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EmitUniform {
    mvp: [[f32; 4]; 4],
    color: [f32; 4],
}

struct Mesh {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    count: u32,
}

pub struct SceneGpu {
    world_pipe: wgpu::RenderPipeline,
    emit_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    blit_pipe: wgpu::RenderPipeline,
    world_layout: wgpu::BindGroupLayout,
    sky_layout: wgpu::BindGroupLayout,
    emit_layout: wgpu::BindGroupLayout,
    blit_layout: wgpu::BindGroupLayout,
    meshes: [Mesh; 4],
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    uniform_slots: u32,
    world_bg: wgpu::BindGroup,
    sky_bg: wgpu::BindGroup,
    emit_bg: wgpu::BindGroup,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    blit_bg: wgpu::BindGroup,
    size: (u32, u32),
    snow: Vec<Vec3>,
    target_is_srgb: bool,
}

impl SceneGpu {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("peakrunner"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let world_layout = uniform_layout(device, "world", WORLD_SIZE);
        let sky_layout = uniform_layout(device, "sky", SKY_SIZE);
        let emit_layout = uniform_layout(device, "emit", EMIT_SIZE);
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let world_pipe = lit_pipeline(device, &shader, &world_layout, false, target_format);
        let emit_pipe = emit_pipeline(device, &shader, &emit_layout);
        let sky_pipe = sky_pipeline(device, &shader, &sky_layout);
        let blit_entry = if target_format.is_srgb() {
            "fs_blit_srgb"
        } else {
            "fs_blit"
        };
        let blit_pipe = blit_pipeline(device, &shader, &blit_layout, target_format, blit_entry);

        let meshes = [
            upload(device, "terrain", &terrain::sample_mesh()),
            upload(device, "cube", &cube_mesh()),
            upload(device, "sphere", &sphere_mesh(10, 16)),
            upload(device, "disc", &disc_mesh()),
        ];
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform_slots = 512;
        let uniform = make_uniform(device, uniform_slots);
        let world_bg = uniform_group(device, &world_layout, &uniform, WORLD_SIZE);
        let sky_bg = uniform_group(device, &sky_layout, &uniform, SKY_SIZE);
        let emit_bg = uniform_group(device, &emit_layout, &uniform, EMIT_SIZE);
        let (color, color_view, depth, depth_view, blit_bg) =
            make_target(device, &blit_layout, &sampler, 4, 4);

        let mut snow = Vec::new();
        let mut rng = 0x91A2u32;
        for _ in 0..90 {
            rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let x = (rng >> 8) as f32 / 16_777_216.0;
            rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let y = (rng >> 8) as f32 / 16_777_216.0;
            rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let z = (rng >> 8) as f32 / 16_777_216.0;
            snow.push(Vec3::new(x * 40.0 - 20.0, y * 22.0, z * 40.0 - 20.0));
        }

        Self {
            world_pipe,
            emit_pipe,
            sky_pipe,
            blit_pipe,
            world_layout,
            sky_layout,
            emit_layout,
            blit_layout,
            meshes,
            sampler,
            uniform,
            uniform_slots,
            world_bg,
            sky_bg,
            emit_bg,
            color,
            color_view,
            depth,
            depth_view,
            blit_bg,
            size: (4, 4),
            snow,
            target_is_srgb: target_format.is_srgb(),
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        width: u32,
        height: u32,
        frame: &DrawFrame,
    ) {
        let width = width.max(1);
        let height = height.max(1);
        if self.size != (width, height) {
            let (color, color_view, depth, depth_view, blit_bg) =
                make_target(device, &self.blit_layout, &self.sampler, width, height);
            self.color = color;
            self.color_view = color_view;
            self.depth = depth;
            self.depth_view = depth_view;
            self.blit_bg = blit_bg;
            self.size = (width, height);
        }

        let mut snow_draws = Vec::with_capacity(self.snow.len());
        let eye = frame.eye;
        let dt = frame.dt;
        for s in &mut self.snow {
            s.y -= 4.2 * dt;
            s.x += 0.4 * dt;
            let w = eye + *s;
            if w.y < terrain::height(w.x, w.z) + 0.4 || (w - eye).length() > 24.0 {
                *s = Vec3::new(
                    ((s.x + 17.0) % 40.0) - 20.0,
                    10.0 + ((s.z.abs() * 3.0) % 8.0),
                    ((s.z + 11.0) % 40.0) - 20.0,
                );
            }
            snow_draws.push(LitDraw {
                mesh: MeshId::Cube,
                model: Mat4::from_translation(eye + *s) * Mat4::from_scale(Vec3::splat(0.045)),
                color: Vec3::splat(0.92),
                emit: 0.15,
                mode: 0.0,
            });
        }

        let slots = 2 + frame.lit.len() + snow_draws.len() + frame.emit.len() + frame.viewmodel.len();
        self.ensure_slots(device, slots as u32 + 4);

        let mut staging = vec![0u8; self.uniform_slots as usize * SLOT as usize];
        let mut cursor = 0u32;
        let push = |staging: &mut [u8], cursor: &mut u32, bytes: &[u8]| -> u32 {
            let offset = *cursor;
            let start = offset as usize;
            staging[start..start + bytes.len()].copy_from_slice(bytes);
            *cursor += SLOT as u32;
            offset
        };

        let sky_off = push(
            &mut staging,
            &mut cursor,
            bytemuck::bytes_of(&SkyUniform {
                inv_vp: frame.inv_vp.to_cols_array_2d(),
                cam: frame.eye.to_array(),
                pad0: 0.0,
                sun: frame.sun.to_array(),
                pad1: 0.0,
            }),
        );

        let mut lit_offs = Vec::with_capacity(frame.lit.len() + snow_draws.len());
        for draw in frame.lit.iter().chain(snow_draws.iter()) {
            lit_offs.push(push(
                &mut staging,
                &mut cursor,
                bytemuck::bytes_of(&world_uniform(draw, frame.proj * frame.view, frame.eye, frame.sun, frame.fog)),
            ));
        }
        let mut emit_offs = Vec::with_capacity(frame.emit.len());
        for draw in &frame.emit {
            emit_offs.push(push(
                &mut staging,
                &mut cursor,
                bytemuck::bytes_of(&emit_uniform(draw, frame.proj * frame.view)),
            ));
        }
        let vm_sun = Vec3::new(0.2, 0.8, 0.5).normalize();
        let mut vm_offs = Vec::with_capacity(frame.viewmodel.len());
        for draw in &frame.viewmodel {
            vm_offs.push(push(
                &mut staging,
                &mut cursor,
                bytemuck::bytes_of(&world_uniform(draw, frame.vm_proj, Vec3::ZERO, vm_sun, Vec3::ZERO)),
            ));
        }

        let used = cursor as u64;
        queue.write_buffer(&self.uniform, 0, &staging[..used as usize]);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.07,
                        g: 0.10,
                        b: 0.16,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.sky_pipe);
        pass.set_bind_group(1, &self.sky_bg, &[sky_off]);
        pass.draw(0..3, 0..1);

        pass.set_pipeline(&self.world_pipe);
        let lit_count = frame.lit.len();
        for (i, draw) in frame.lit.iter().chain(snow_draws.iter()).enumerate() {
            let mesh = &self.meshes[draw.mesh as usize];
            pass.set_bind_group(0, &self.world_bg, &[lit_offs[i]]);
            pass.set_vertex_buffer(0, mesh.vbo.slice(..));
            pass.set_index_buffer(mesh.ibo.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..mesh.count, 0, 0..1);
            let _ = lit_count;
        }
        if !frame.emit.is_empty() {
            pass.set_pipeline(&self.emit_pipe);
            for (i, draw) in frame.emit.iter().enumerate() {
                let mesh = &self.meshes[draw.mesh as usize];
                pass.set_bind_group(2, &self.emit_bg, &[emit_offs[i]]);
                pass.set_vertex_buffer(0, mesh.vbo.slice(..));
                pass.set_index_buffer(mesh.ibo.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
        }
        drop(pass);

        if !frame.viewmodel.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewmodel"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.world_pipe);
            for (i, draw) in frame.viewmodel.iter().enumerate() {
                let mesh = &self.meshes[draw.mesh as usize];
                pass.set_bind_group(0, &self.world_bg, &[vm_offs[i]]);
                pass.set_vertex_buffer(0, mesh.vbo.slice(..));
                pass.set_index_buffer(mesh.ibo.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
        }
    }

    fn ensure_slots(&mut self, device: &wgpu::Device, need: u32) {
        if need <= self.uniform_slots {
            return;
        }
        self.uniform_slots = need.next_power_of_two().max(self.uniform_slots * 2);
        self.uniform = make_uniform(device, self.uniform_slots);
        self.world_bg = uniform_group(device, &self.world_layout, &self.uniform, WORLD_SIZE);
        self.sky_bg = uniform_group(device, &self.sky_layout, &self.uniform, SKY_SIZE);
        self.emit_bg = uniform_group(device, &self.emit_layout, &self.uniform, EMIT_SIZE);
    }
}

fn world_uniform(draw: &LitDraw, vp: Mat4, cam: Vec3, sun: Vec3, fog: Vec3) -> WorldUniform {
    let n = normal_columns(draw.model);
    WorldUniform {
        mvp: (vp * draw.model).to_cols_array_2d(),
        model: draw.model.to_cols_array_2d(),
        n0: n[0],
        n1: n[1],
        n2: n[2],
        cam: cam.to_array(),
        emit: draw.emit,
        sun: sun.to_array(),
        mode: draw.mode,
        fog: fog.to_array(),
        pad0: 0.0,
        color: draw.color.to_array(),
        pad1: 0.0,
    }
}

fn emit_uniform(draw: &EmitDraw, vp: Mat4) -> EmitUniform {
    EmitUniform {
        mvp: (vp * draw.model).to_cols_array_2d(),
        color: draw.color,
    }
}

fn uniform_layout(device: &wgpu::Device, label: &str, size: u64) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: NonZeroU64::new(size),
            },
            count: None,
        }],
    })
}

fn uniform_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    size: u64,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("uniform"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: NonZeroU64::new(size),
            }),
        }],
    })
}

fn make_uniform(device: &wgpu::Device, slots: u32) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uniforms"),
        size: slots as u64 * SLOT,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_target(
    device: &wgpu::Device,
    blit_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::BindGroup,
) {
    let color = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene-color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene-depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    let blit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit"),
        layout: blit_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&color_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (color, color_view, depth, depth_view, blit_bg)
}

fn lit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    _emit: bool,
    _target: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("world"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("world"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_world"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(vertex_layout())],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(depth_state(true)),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_world"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn emit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("emit"),
        bind_group_layouts: &[None, None, Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("emit"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_emit"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(vertex_layout())],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(depth_state(false)),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_emit"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn sky_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sky"),
        bind_group_layouts: &[None, Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sky"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_fullscreen"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_sky"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn blit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    target: wgpu::TextureFormat,
    entry: &str,
) -> wgpu::RenderPipeline {
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("blit"),
        bind_group_layouts: &[None, None, None, Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit"),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_fullscreen"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24Plus,
        depth_write_enabled: Some(write),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: 24,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
        ],
    }
}

fn upload(device: &wgpu::Device, label: &str, mesh: &(Vec<f32>, Vec<u16>)) -> Mesh {
    use wgpu::util::DeviceExt;
    let vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&mesh.0),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&mesh.1),
        usage: wgpu::BufferUsages::INDEX,
    });
    Mesh {
        vbo,
        ibo,
        count: mesh.1.len() as u32,
    }
}

fn cube_mesh() -> (Vec<f32>, Vec<u16>) {
    let faces: [([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]); 6] = [
        ([-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5], [0.0, 0.0, 1.0]),
        ([0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.0, 0.0, -1.0]),
        ([-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5], [-1.0, 0.0, 0.0]),
        ([0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [1.0, 0.0, 0.0]),
        ([-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5], [0.0, 1.0, 0.0]),
        ([-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5], [0.0, -1.0, 0.0]),
    ];
    let mut v = Vec::new();
    let mut i = Vec::new();
    for (fi, (a, b, c, d, n)) in faces.iter().enumerate() {
        let base = (fi * 4) as u16;
        for p in [a, b, c, d] {
            v.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2]]);
        }
        i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (v, i)
}

fn sphere_mesh(lat: u32, lon: u32) -> (Vec<f32>, Vec<u16>) {
    let mut v = Vec::new();
    let mut idx = Vec::new();
    for y in 0..=lat {
        let p = y as f32 / lat as f32;
        let th = p * std::f32::consts::PI;
        let sy = th.cos();
        let r = th.sin();
        for x in 0..=lon {
            let q = x as f32 / lon as f32;
            let ph = q * std::f32::consts::TAU;
            let px = r * ph.cos();
            let pz = r * ph.sin();
            v.extend_from_slice(&[px, sy, pz, px, sy, pz]);
        }
    }
    let stride = lon + 1;
    for y in 0..lat {
        for x in 0..lon {
            let i = (y * stride + x) as u16;
            let s = stride as u16;
            idx.extend_from_slice(&[i, i + s, i + 1, i + 1, i + s, i + s + 1]);
        }
    }
    (v, idx)
}

fn disc_mesh() -> (Vec<f32>, Vec<u16>) {
    let n = 24u16;
    let mut v = vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        v.extend_from_slice(&[a.cos(), 0.0, a.sin(), 0.0, 1.0, 0.0]);
    }
    let mut idx = Vec::new();
    for i in 0..n {
        let a = 1 + i;
        let b = 1 + (i + 1) % n;
        idx.extend_from_slice(&[0, a, b]);
    }
    let base = (n + 1) as usize;
    v.extend_from_slice(&[0.0, -0.08, 0.0, 0.0, -1.0, 0.0]);
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        v.extend_from_slice(&[a.cos(), -0.08, a.sin(), 0.0, -1.0, 0.0]);
    }
    let b0 = base as u16;
    for i in 0..n {
        let a = b0 + 1 + i;
        let c = b0 + 1 + (i + 1) % n;
        idx.extend_from_slice(&[b0, c, a]);
    }
    (v, idx)
}

pub struct SceneCallback {
    pub frame: DrawFrame,
    pub pixels: [u32; 2],
}

impl CallbackTrait for SceneCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(scene) = resources.get_mut::<SceneGpu>() {
            scene.render(device, queue, encoder, self.pixels[0], self.pixels[1], &self.frame);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(scene) = resources.get::<SceneGpu>() else {
            return;
        };
        pass.set_pipeline(&scene.blit_pipe);
        pass.set_bind_group(3, &scene.blit_bg, &[]);
        pass.draw(0..3, 0..1);
        let _ = scene.target_is_srgb;
    }
}

pub fn install(cc: &eframe::CreationContext<'_>) -> Result<(), String> {
    let state = cc
        .wgpu_render_state
        .as_ref()
        .ok_or_else(|| "PeakRunner needs the wgpu renderer".to_string())?;
    let scene = SceneGpu::new(&state.device, state.target_format);
    state.renderer.write().callback_resources.insert(scene);
    Ok(())
}

const _: () = assert!(std::mem::size_of::<WorldUniform>() == WORLD_SIZE as usize);
const _: () = assert!(std::mem::size_of::<SkyUniform>() == SKY_SIZE as usize);
const _: () = assert!(std::mem::size_of::<EmitUniform>() == EMIT_SIZE as usize);

#[cfg(test)]
mod shader_check {
    #[test]
    fn shaders_validate() {
        let module = naga::front::wgsl::parse_str(include_str!("shaders.wgsl"))
            .expect("wgsl parse");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        validator.validate(&module).expect("wgsl validate");
    }
}
