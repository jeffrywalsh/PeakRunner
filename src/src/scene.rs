use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::wgpu;
use eframe::egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};
use glam::{Mat4, Vec3};

use crate::drawlist::{normal_columns, DrawFrame, EmitDraw, LitDraw, MeshId};
use crate::grass;
use crate::terrain::{self, MapId};

const SLOT: u64 = 512;
const WORLD_SIZE: u64 = 304;
const SKY_SIZE: u64 = 96;
const EMIT_SIZE: u64 = 96;

fn precipitation_count(map: MapId, available: usize) -> usize {
    match map { MapId::Valley => available, MapId::Raindance | MapId::BroadsideClone | MapId::StonehengeClone | MapId::SnowblindClone | MapId::DesertOfDeathClone => 0 }
}

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
    ground_cover: [f32; 4],
    ground_soil: [f32; 4],
    ground_rock: [f32; 4],
    ground_rules: [f32; 4],
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
    fog: [f32; 4],
}

struct Mesh {
    vbo: wgpu::Buffer,
    ibo: wgpu::Buffer,
    count: u32,
}

pub struct SceneGpu {
    imported: Option<crate::map_scene::MapGpu>,
    world_pipe: wgpu::RenderPipeline,
    emit_pipe: wgpu::RenderPipeline,
    smoke_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    blit_pipe: wgpu::RenderPipeline,
    world_layout: wgpu::BindGroupLayout,
    sky_layout: wgpu::BindGroupLayout,
    emit_layout: wgpu::BindGroupLayout,
    blit_layout: wgpu::BindGroupLayout,
    meshes: [Mesh; 6],
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
    terrain_map: MapId,
    grass_ready: bool,
    grass_albedo: wgpu::Texture,
    grass_normal: wgpu::Texture,
    grass_albedo_view: wgpu::TextureView,
    grass_normal_view: wgpu::TextureView,
    grass_sampler: wgpu::Sampler,
}

impl SceneGpu {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("peakrunner"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let (grass_albedo, grass_albedo_view, grass_normal, grass_normal_view, grass_sampler) =
            grass_textures(device);
        let world_layout = world_bind_layout(device);
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
        let emit_pipe = emit_pipeline(device, &shader, &emit_layout, false);
        let smoke_pipe = emit_pipeline(device, &shader, &emit_layout, true);
        let sky_pipe = sky_pipeline(device, &shader, &sky_layout);
        let blit_entry = if target_format.is_srgb() {
            "fs_blit_srgb"
        } else {
            "fs_blit"
        };
        let blit_pipe = blit_pipeline(device, &shader, &blit_layout, target_format, blit_entry);

        let meshes = [
            upload(device, "terrain", &terrain::sample_mesh_of(MapId::Valley)),
            upload(device, "cube", &cube_mesh()),
            upload(device, "sphere", &sphere_mesh(10, 16)),
            upload(device, "disc", &disc_mesh()),
            upload(device, "beveled housing", &bevel_mesh()),
            upload(device, "chamfered armor", &armor_mesh()),
        ];
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("scene"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform_slots = 512;
        let uniform = make_uniform(device, uniform_slots);
        let world_bg = world_bind_group(
            device,
            &world_layout,
            &uniform,
            &grass_albedo_view,
            &grass_normal_view,
            &grass_sampler,
        );
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
            imported: None,
            world_pipe,
            emit_pipe,
            smoke_pipe,
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
            terrain_map: MapId::Valley,
            grass_ready: false,
            grass_albedo,
            grass_normal,
            grass_albedo_view,
            grass_normal_view,
            grass_sampler,
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
        self.upload_grass(queue);
        if self.terrain_map != frame.map {
            self.imported = crate::map_scene::MapGpu::new(device,frame.map);
            self.meshes[0] = upload(device, "terrain", &terrain::sample_mesh_of(frame.map));
            self.terrain_map = frame.map;
        }
        if let Some(map)=&mut self.imported {map.update(queue,frame);}

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
        let precipitation_count = precipitation_count(frame.map, self.snow.len());
        // Raindance precipitation is off, for both gameplay and the flyby.
        // Do not reinterpret the sparse Valley snow pool as patchy rain.
        for s in self.snow.iter_mut().take(precipitation_count) {
            s.y -= 4.2*dt;
            s.x += 0.4 * dt;
            let w = eye + *s;
            if w.y < terrain::height_on(frame.map, w.x, w.z) + 0.4 || (w - eye).length() > 24.0 {
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

        let slots = 2 + frame.lit.len() + snow_draws.len() + frame.emit.len() + frame.smoke.len() + frame.viewmodel.len();
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
                bytemuck::bytes_of(&world_uniform(draw, frame.proj * frame.view, frame.eye, frame.sun, frame.fog, frame.fog_density, frame.time, frame.map)),
            ));
        }
        let mut emit_offs = Vec::with_capacity(frame.emit.len());
        for draw in &frame.emit {
            emit_offs.push(push(
                &mut staging,
                &mut cursor,
                bytemuck::bytes_of(&emit_uniform(draw, frame.proj * frame.view, [0.0; 4])),
            ));
        }
        let mut smoke_offs = Vec::with_capacity(frame.smoke.len());
        for draw in &frame.smoke {
            let dist = draw.model.w_axis.truncate().distance(frame.eye);
            let fog = (1.0 - (-dist * frame.fog_density).exp()).clamp(0.0, 0.92);
            smoke_offs.push(push(
                &mut staging,
                &mut cursor,
                bytemuck::bytes_of(&emit_uniform(draw, frame.proj * frame.view,
                    [frame.fog.x, frame.fog.y, frame.fog.z, fog])),
            ));
        }
        let vm_sun = Vec3::new(0.2, 0.8, 0.5).normalize();
        let mut vm_offs = Vec::with_capacity(frame.viewmodel.len());
        for draw in &frame.viewmodel {
            vm_offs.push(push(
                &mut staging,
                &mut cursor,
                bytemuck::bytes_of(&world_uniform(draw, frame.vm_proj, Vec3::ZERO, vm_sun, Vec3::ZERO, 0.0, 0.0, frame.map)),
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

        if frame.map != MapId::Valley {
            if let Some(map)=&self.imported {map.draw(&mut pass);}
        }

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
        // Darkening effects first, back to front (drawlist sorts them), then
        // the additive pass so flames and sparks glow through the smoke.
        if !frame.smoke.is_empty() {
            pass.set_pipeline(&self.smoke_pipe);
            for (i, draw) in frame.smoke.iter().enumerate() {
                let mesh = &self.meshes[draw.mesh as usize];
                pass.set_bind_group(2, &self.emit_bg, &[smoke_offs[i]]);
                pass.set_vertex_buffer(0, mesh.vbo.slice(..));
                pass.set_index_buffer(mesh.ibo.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
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

    fn upload_grass(&mut self, queue: &wgpu::Queue) {
        if self.grass_ready {
            return;
        }
        let (albedo, normal) = grass::bake_textures();
        upload_rgba_mips(queue, &self.grass_albedo, &albedo, grass::TEX_SIZE);
        upload_rgba_mips(queue, &self.grass_normal, &normal, grass::TEX_SIZE);
        self.grass_ready = true;
    }

    fn ensure_slots(&mut self, device: &wgpu::Device, need: u32) {
        if need <= self.uniform_slots {
            return;
        }
        self.uniform_slots = need.next_power_of_two().max(self.uniform_slots * 2);
        self.uniform = make_uniform(device, self.uniform_slots);
        self.world_bg = world_bind_group(
            device,
            &self.world_layout,
            &self.uniform,
            &self.grass_albedo_view,
            &self.grass_normal_view,
            &self.grass_sampler,
        );
        self.sky_bg = uniform_group(device, &self.sky_layout, &self.uniform, SKY_SIZE);
        self.emit_bg = uniform_group(device, &self.emit_layout, &self.uniform, EMIT_SIZE);
    }
}

fn world_uniform(draw: &LitDraw, vp: Mat4, cam: Vec3, sun: Vec3, fog: Vec3, density: f32, time: f32, map: MapId) -> WorldUniform {
    let n = normal_columns(draw.model);
    let style = terrain::surface_style(map);
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
        pad0: density,
        color: draw.color.to_array(),
        pad1: time,
        ground_cover: style.cover,
        ground_soil: style.soil,
        ground_rock: style.rock,
        ground_rules: style.rules,
    }
}

fn emit_uniform(draw: &EmitDraw, vp: Mat4, fog: [f32; 4]) -> EmitUniform {
    EmitUniform {
        mvp: (vp * draw.model).to_cols_array_2d(),
        color: draw.color,
        fog,
    }
}

fn world_bind_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("world"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: NonZeroU64::new(WORLD_SIZE),
                },
                count: None,
            },
            texture_entry(1),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            texture_entry(3),
        ],
    })
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn world_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    albedo: &wgpu::TextureView,
    normal: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("world"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: NonZeroU64::new(WORLD_SIZE),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(albedo),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(normal),
            },
        ],
    })
}

fn grass_textures(
    device: &wgpu::Device,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Sampler,
) {
    let size = grass::TEX_SIZE;
    let mips = size.trailing_zeros() + 1;
    let make = |label: &str| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    };
    let albedo = make("grass-albedo");
    let normal = make("grass-normal");
    let albedo_view = albedo.create_view(&wgpu::TextureViewDescriptor::default());
    let normal_view = normal.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("grass"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: 8,
        ..Default::default()
    });
    (albedo, albedo_view, normal, normal_view, sampler)
}

fn upload_rgba_mips(queue: &wgpu::Queue, texture: &wgpu::Texture, base: &[u8], size: u32) {
    let mut mip = base.to_vec();
    let mut dim = size;
    let mut level = 0u32;
    loop {
        let row = dim * 4;
        let padded = row.div_ceil(256) * 256;
        let mut packed = vec![0u8; (padded * dim) as usize];
        for y in 0..dim as usize {
            let src = y * row as usize;
            let dst = y * padded as usize;
            packed[dst..dst + row as usize].copy_from_slice(&mip[src..src + row as usize]);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &packed,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(dim),
            },
            wgpu::Extent3d {
                width: dim,
                height: dim,
                depth_or_array_layers: 1,
            },
        );
        if dim == 1 {
            break;
        }
        mip = downsample_rgba(&mip, dim);
        dim /= 2;
        level += 1;
    }
}

fn downsample_rgba(src: &[u8], size: u32) -> Vec<u8> {
    let n = size / 2;
    let mut dst = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            for c in 0..4u32 {
                let mut sum = 0u32;
                for dy in 0..2u32 {
                    for dx in 0..2u32 {
                        let sx = x * 2 + dx;
                        let sy = y * 2 + dy;
                        sum += src[((sy * size + sx) * 4 + c) as usize] as u32;
                    }
                }
                dst[((y * n + x) * 4 + c) as usize] = (sum / 4) as u8;
            }
        }
    }
    dst
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
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

/// Effect pipelines share the emit uniform. `smoke` is the alpha-blended,
/// fogged variant that can darken the scene; the other stays additive.
fn emit_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    smoke: bool,
) -> wgpu::RenderPipeline {
    let blend = if smoke {
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }
    } else {
        wgpu::BlendState {
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
        }
    };
    let label = if smoke { "smoke" } else { "emit" };
    let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[None, None, Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipe_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_emit"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(vertex_layout())],
        },
        primitive: wgpu::PrimitiveState {
            // No culling: the sphere mesh winds inward, and culling either
            // side leaves a hollow ring where a puff meets a wall.
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(depth_state(false)),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(if smoke { "fs_smoke" } else { "fs_emit" }),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(blend),
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

/// Chamfered rectangular extrusion, unit size, facing down local -Z.
fn bevel_mesh() -> (Vec<f32>, Vec<u16>) {
    let outline = [(-0.35, -0.5), (0.35, -0.5), (0.5, -0.3), (0.5, 0.3),
        (0.35, 0.5), (-0.35, 0.5), (-0.5, 0.3), (-0.5, -0.3)];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for i in 0..8 {
        let (x, y) = outline[i];
        let (xx, yy) = outline[(i + 1) % 8];
        let n = Vec3::new(yy - y, x - xx, 0.0).normalize();
        let b = (vertices.len() / 6) as u16;
        for p in [[x, y, -0.5], [xx, yy, -0.5], [xx, yy, 0.5], [x, y, 0.5]] {
            vertices.extend_from_slice(&[p[0], p[1], p[2], n.x, n.y, n.z]);
        }
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    for sign in [-1.0, 1.0] {
        let b = (vertices.len() / 6) as u16;
        vertices.extend_from_slice(&[0.0, 0.0, sign * 0.5, 0.0, 0.0, sign]);
        for (x, y) in outline {
            vertices.extend_from_slice(&[x, y, sign * 0.5, 0.0, 0.0, sign]);
        }
        for i in 0..8u16 {
            let a = b + 1 + i;
            let c = b + 1 + (i + 1) % 8;
            indices.extend_from_slice(&if sign > 0.0 { [b, a, c] } else { [b, c, a] });
        }
    }
    (vertices, indices)
}

/// Shared original armor primitive: inset end caps and bevels on all axes.
/// Flat face normals keep the low-poly silhouette crisp without block edges.
fn armor_mesh() -> (Vec<f32>, Vec<u16>) {
    let outline=[(-0.32,-0.5),(0.32,-0.5),(0.5,-0.32),(0.5,0.32),
        (0.32,0.5),(-0.32,0.5),(-0.5,0.32),(-0.5,-0.32)];
    let rings=[(-0.5,0.76),(-0.32,1.0),(0.32,1.0),(0.5,0.76)];
    let mut vertices=Vec::new(); let mut indices=Vec::new();
    let mut face=|points:&[Vec3]| {
        let n=(points[1]-points[0]).cross(points[2]-points[0]).normalize();
        let base=(vertices.len()/6) as u16;
        for p in points {vertices.extend_from_slice(&[p.x,p.y,p.z,n.x,n.y,n.z]);}
        for i in 1..points.len()-1 {indices.extend_from_slice(&[base,base+i as u16,base+i as u16+1]);}
    };
    for pair in rings.windows(2) {
        for i in 0..8 {
            let (x,y)=outline[i]; let (u,v)=outline[(i+1)%8];
            let (z,a)=pair[0]; let (zz,b)=pair[1];
            face(&[Vec3::new(x*a,y*a,z),Vec3::new(u*a,v*a,z),
                Vec3::new(u*b,v*b,zz),Vec3::new(x*b,y*b,zz)]);
        }
    }
    for sign in [-1.,1.] {
        let points:Vec<Vec3>=(0..8).map(|i| {
            let (x,y)=outline[if sign<0. {7-i} else {i}];
            Vec3::new(x*0.76,y*0.76,sign*0.5)
        }).collect();
        face(&points);
    }
    (vertices,indices)
}

fn disc_mesh() -> (Vec<f32>, Vec<u16>) {
    let n = 24u16;
    let mut v = vec![0.0, 0.5, 0.0, 0.0, 1.0, 0.0];
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        v.extend_from_slice(&[a.cos(), 0.5, a.sin(), 0.0, 1.0, 0.0]);
    }
    let mut idx = Vec::new();
    for i in 0..n {
        let a = 1 + i;
        let b = 1 + (i + 1) % n;
        idx.extend_from_slice(&[0, b, a]);
    }
    let base = (n + 1) as usize;
    v.extend_from_slice(&[0.0, -0.5, 0.0, 0.0, -1.0, 0.0]);
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        v.extend_from_slice(&[a.cos(), -0.5, a.sin(), 0.0, -1.0, 0.0]);
    }
    let b0 = base as u16;
    for i in 0..n {
        let a = b0 + 1 + i;
        let c = b0 + 1 + (i + 1) % n;
        idx.extend_from_slice(&[b0, a, c]);
    }
    // Give the projectile a visible rim when seen edge-on.
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let b = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
        let base = (v.len() / 6) as u16;
        for (angle, y) in [(a, -0.5), (b, -0.5), (b, 0.5), (a, 0.5)] {
            v.extend_from_slice(&[angle.cos(), y, angle.sin(), angle.cos(), 0.0, angle.sin()]);
        }
        idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    // A raised ridge so the spin is visible. A plain disc is a circle from every angle.
    let ridge = v.len() / 6;
    let bar = [
        [-0.12, 0.02, -0.16],
        [0.95, 0.02, -0.16],
        [0.95, 0.02, 0.16],
        [-0.12, 0.02, 0.16],
        [-0.12, 0.16, -0.1],
        [0.95, 0.16, -0.1],
        [0.95, 0.16, 0.1],
        [-0.12, 0.16, 0.1],
    ];
    for p in bar {
        v.extend_from_slice(&[p[0], p[1] + 0.5, p[2], 0.0, 1.0, 0.0]);
    }
    let r0 = ridge as u16;
    idx.extend_from_slice(&[
        r0, r0 + 1, r0 + 2, r0, r0 + 2, r0 + 3,
        r0 + 4, r0 + 6, r0 + 5, r0 + 4, r0 + 7, r0 + 6,
        r0, r0 + 4, r0 + 5, r0, r0 + 5, r0 + 1,
        r0 + 3, r0 + 2, r0 + 6, r0 + 3, r0 + 6, r0 + 7,
    ]);
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
    fn raindance_rain_is_off_and_valley_snow_is_preserved() {
        for count in [0, 128, 1024] {
            assert_eq!(super::precipitation_count(super::MapId::Raindance, count), 0);
            assert_eq!(super::precipitation_count(super::MapId::Valley, count), count);
        }
    }
    /// Real GPU render of the production scene, without opening a game window.
    /// Run with --ignored --nocapture; captures are for visual review, not golden tests.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn render_gameplay_captures() {
        use super::*;
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default())
                .await.expect("GPU adapter");
            let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default())
                .await.expect("GPU device");
            let mut scene = SceneGpu::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            let mut world = crate::sim::World::new();
            world.set_map(MapId::Raindance);
            world.start_match(true);
            world.players.truncate(1);
            world.players[0].yaw = std::f32::consts::PI;
            world.players[0].pitch = -0.12;
            if let Ok(camera)=std::env::var("PEAKRUNNER_CAPTURE_CAMERA") {
                let v:Vec<f32>=camera.split(',').map(|s|s.parse().expect("camera number")).collect();
                assert_eq!(v.len(),5);
                world.players[0].pos=Vec3::new(v[0],v[1],v[2]);
                world.players[0].yaw=v[3];world.players[0].pitch=v[4];
            }
            let label = std::env::var("PEAKRUNNER_CAPTURE_LABEL").unwrap_or_else(|_| "current".into());
            std::fs::create_dir_all("screenshots").unwrap();
            for (name, width, height, cooldown) in [
                ("characters", 1280u32, 800u32, 0.0),
                ("characters-flight", 1280, 800, 0.0),
                ("desktop", 1280u32, 800u32, 0.0),
                ("portrait", 390, 844, 0.0),
                ("chat", 1280, 800, 0.0),
                ("chat-portrait", 390, 844, 0.0),
                ("menu-start", 1280, 800, 0.0),
                ("menu-quarter", 1280, 800, 0.0),
                ("menu-half", 1280, 800, 0.0),
                ("plasma", 1280, 800, 0.0),
                ("plasma-portrait", 390, 844, 0.0),
                ("disc-blast", 1280, 800, 0.0),
                ("grenade-blast", 1280, 800, 0.0),
                ("grenade-blast-portrait", 390, 844, 0.0),
                ("firing", 1280, 800, 0.98),
                ("reload-open", 1280, 800, 0.72),
                ("reload-feed", 1280, 800, 0.25),
                ("projectile", 1280, 800, 0.0),
                ("projectile-flight", 1280, 800, 0.0),
                ("chaingun", 1280, 800, 0.0),
                ("chaingun-portrait", 390, 844, 0.0),
                ("grenade", 1280, 800, 0.0),
                ("grenade-portrait", 390, 844, 0.0),
                ("chaingun-shot", 1280, 800, 0.0),
                ("grenade-shot", 1280, 800, 0.0),
            ] {
                if std::env::var_os("QA_CHARACTERS").is_some() && !name.starts_with("characters") { continue; }
                world.players.truncate(1);
                world.state=crate::sim::MatchState::Playing;
                if name.starts_with("menu") {
                    world.state=crate::sim::MatchState::Flyby;
                    world.flyby=if name.ends_with("half") {17.45} else if name.ends_with("quarter") {8.73} else {0.};
                }
                world.players[0].cooldown = cooldown;
                world.discs.clear();
                world.explosions.clear();
                if name.contains("blast") {
                    let kind=if name.starts_with("grenade") {2} else {0};
                    let profile=peakrunner_core::combat::player_weapon_blast(kind).unwrap();
                    let (eye,dir,_)=world.camera();
                    world.explosions.push(crate::sim::Explosion {pos:eye+dir*22.-Vec3::Y*5.,
                        age:0.16,kind,max_r:profile.radius});
                }
                if name.starts_with("plasma") {
                    let (eye,dir,_)=world.camera();
                    world.discs.push(crate::sim::Disc {pos:eye+dir*7.,vel:-dir*80.,
                        owner:usize::MAX,team:crate::sim::Team::Ember,kind:3,life:2.9,spin:0.});
                }
                world.players[0].weapon = if name.starts_with("chaingun") { 1 }
                    else if name.starts_with("grenade") { 2 } else { 0 };
                world.input.weapon = world.players[0].weapon;
                if name.starts_with("projectile") || name.ends_with("shot") {
                    // Clear the base flag so its pickup model cannot obscure
                    // the launch sequence when exercising the real game tick.
                    world.players[0].pos.y += 10.0;
                    world.input.fire = true;
                    world.tick(1.0 / 60.0);
                    world.input.fire = false;
                    if name == "projectile-flight" { world.tick(0.05); }
                    if name == "grenade-shot" {
                        for _ in 0..18 { world.tick(1.0 / 60.0); }
                    }
                    assert!(!world.discs.is_empty(), "capture a real fired round");
                }
                if name.starts_with("characters") {
                    let origin=world.players[0].pos;
                    for i in 0..3 {
                        let mut p=world.players[0].clone();
                        p.pos=origin+Vec3::new((i as f32-1.)*1.7,0.,0.);
                        p.yaw=[0.35,-0.6,2.8][i];
                        p.team=if i==1 {crate::sim::Team::Glacier} else {crate::sim::Team::Ember};
                        p.jetting=name.ends_with("flight"); p.skiing=i==1;
                        p.on_ground=!p.jetting;
                        world.players.push(p);
                    }
                }
                // Develop client-side effects (smoke, debris, sparks, barrel
                // spin, heat) up to the captured moment, as a live client would.
                let mut fx = crate::effects::Effects::new();
                let aspect = width as f32 / height as f32;
                if name.contains("blast") {
                    let target = world.explosions[0].age;
                    for k in 0..12 {
                        world.explosions[0].age = target * k as f32 / 12.0;
                        crate::drawlist::build_frame_with(&world, aspect, target / 12.0, &mut fx);
                    }
                    world.explosions[0].age = target;
                }
                if name == "chaingun-shot" {
                    let keep = world.players[0].cooldown;
                    for _ in 0..70 {
                        world.players[0].cooldown = 0.05;
                        crate::drawlist::build_frame_with(&world, aspect, 1.0 / 60.0, &mut fx);
                    }
                    world.players[0].cooldown = keep;
                }
                let mut frame = crate::drawlist::build_frame_with(&world, aspect, 0.0, &mut fx);
                if name.starts_with("characters") {
                    let target=world.players[0].pos+Vec3::Y*0.95;
                    frame.eye=target+Vec3::new(0.,0.5,-5.3);
                    frame.view=Mat4::look_at_rh(frame.eye,target,Vec3::Y);
                    frame.proj=crate::drawlist::clip_correct(Mat4::perspective_rh(49f32.to_radians(),width as f32/height as f32,0.1,2500.));
                    frame.inv_vp=(frame.proj*frame.view).inverse();
                    frame.viewmodel.clear();
                }
                let mut encoder = device.create_command_encoder(&Default::default());
                scene.render(&device, &queue, &mut encoder, width, height, &frame);
                if name.starts_with("chat") {
                    let ctx=egui::Context::default();
                    let mut app=crate::app::PeakRunnerApp::comms_fixture();
                    let mut renderer=eframe::egui_wgpu::Renderer::new(&device,wgpu::TextureFormat::Rgba8Unorm,Default::default());
                    // Areas need a sizing pass before their first visible frame.
                    for _ in 0..2 {
                        let mut output=ctx.run_ui(egui::RawInput { screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width as f32,height as f32))), ..Default::default() }, |ui|app.capture_comms(ui.ctx()));
                        for (id,deltas) in &output.textures_delta.set {for delta in deltas {renderer.update_texture(&device,&queue,*id,delta);}}
                        output.textures_delta.clear();
                        let jobs=ctx.tessellate(output.shapes,1.0);
                        let screen=eframe::egui_wgpu::ScreenDescriptor {size_in_pixels:[width,height],pixels_per_point:1.0};
                        let commands=renderer.update_buffers(&device,&queue,&mut encoder,&jobs,&screen);
                        assert!(commands.is_empty());
                        let view=scene.color.create_view(&Default::default());
                        let pass=encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label:Some("chat capture"),color_attachments:&[Some(wgpu::RenderPassColorAttachment {
                                view:&view,resolve_target:None,depth_slice:None,
                                ops:wgpu::Operations {load:wgpu::LoadOp::Load,store:wgpu::StoreOp::Store}
                            })],depth_stencil_attachment:None,timestamp_writes:None,occlusion_query_set:None,multiview_mask:None,
                        });
                        renderer.render(&mut pass.forget_lifetime(),&jobs,&screen);
                    }
                }
                let stride = (width * 4).div_ceil(256) * 256;
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("capture"), size: (stride * height) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                encoder.copy_texture_to_buffer(
                    scene.color.as_image_copy(),
                    wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout {
                        offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(height),
                    } },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let mapped = buffer.slice(..).get_mapped_range().unwrap();
                let pixels: Vec<u8> = mapped.chunks(stride as usize)
                    .flat_map(|row| row[..width as usize * 4].iter().copied()).collect();
                assert!(pixels.chunks_exact(4).any(|p| p[0] > 80 && p[1] > 80), "blank render");
                let path = format!("screenshots/{label}-{name}.png");
                let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), width, height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
                println!("Rendered {path}");
            }
        });
    }

    #[test]
    fn shaders_validate() {
        for source in [include_str!("shaders.wgsl"), include_str!("map.wgsl")] {
        let module = naga::front::wgsl::parse_str(source)
            .expect("wgsl parse");
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        validator.validate(&module).expect("wgsl validate");
        }
    }

    /// Weapon visuals: every viewmodel state, third-person models, rounds in
    /// flight, chaingun impacts and layered explosions on three maps. Writes
    /// `screenshots/weapons-after-*.png`. Run with --ignored --nocapture.
    #[test]
    #[ignore = "requires a GPU adapter"]
    fn render_weapon_captures() {
        use super::*;
        use crate::sim::{Disc, Explosion, MatchState, Team, World};
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.expect("GPU adapter");
            let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("GPU device");
            let mut scene = SceneGpu::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            std::fs::create_dir_all("screenshots").unwrap();
            let (width, height) = (1280u32, 800u32);
            let aspect = width as f32 / height as f32;
            let mut save = |frame: &crate::drawlist::DrawFrame, name: &str| {
                let mut encoder = device.create_command_encoder(&Default::default());
                scene.render(&device, &queue, &mut encoder, width, height, frame);
                let stride = (width * 4).div_ceil(256) * 256;
                let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("capture"), size: (stride * height) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
                encoder.copy_texture_to_buffer(scene.color.as_image_copy(),
                    wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout {
                        offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(height) } },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 });
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let mapped = buffer.slice(..).get_mapped_range().unwrap();
                let pixels: Vec<u8> = mapped.chunks(stride as usize).flat_map(|row| row[..width as usize * 4].iter().copied()).collect();
                let path = format!("screenshots/weapons-after-{name}.png");
                let mut png = png::Encoder::new(std::fs::File::create(&path).unwrap(), width, height);
                png.set_color(png::ColorType::Rgba);
                png.set_depth(png::BitDepth::Eight);
                png.write_header().unwrap().write_image_data(&pixels).unwrap();
                println!("wrote {path}");
            };
            let fresh = |map: MapId| {
                let mut w = World::new();
                w.set_map(map);
                w.start_match(true);
                w.players.truncate(1);
                w.state = MatchState::Playing;
                w.players[0].yaw = std::f32::consts::PI;
                w.players[0].pitch = -0.12;
                w
            };
            let dt = 1.0 / 60.0;
            let develop = |w: &World, fx: &mut crate::effects::Effects, frames: usize| {
                for _ in 0..frames { crate::drawlist::build_frame_with(w, aspect, dt, fx); }
            };

            // Viewmodels: idle, firing, mid reload or cycle, mid switch.
            for (weapon, label) in [(0u8, "disc"), (1, "chaingun"), (2, "grenade")] {
                let reload = crate::sim::weapon_reload(weapon);
                let mut w = fresh(MapId::Raindance);
                w.players[0].weapon = weapon;
                let mut fx = crate::effects::Effects::new();
                develop(&w, &mut fx, 30);
                save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx), &format!("vm-{label}-idle"));
                // Firing: the chaingun needs a spun-up, heated barrel first.
                if weapon == 1 {
                    for f in 0..90 {
                        w.players[0].cooldown = reload - (f % 4) as f32 * 0.018;
                        w.players[0].shots += (f % 4 == 0) as u32;
                        develop(&w, &mut fx, 1);
                    }
                    w.players[0].cooldown = reload - 0.012;
                } else {
                    w.players[0].shots += 1;
                    w.players[0].cooldown = reload - 0.02;
                    develop(&w, &mut fx, 1);
                }
                save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx), &format!("vm-{label}-firing"));
                if weapon == 1 {
                    // Released: the barrels coast down while the jacket cools.
                    w.players[0].cooldown = 0.0;
                    develop(&w, &mut fx, 15);
                    save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx), "vm-chaingun-spindown");
                } else {
                    w.players[0].cooldown = reload * 0.45;
                    develop(&w, &mut fx, 1);
                    save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx), &format!("vm-{label}-reload"));
                }
                w.players[0].cooldown = 0.0;
                develop(&w, &mut fx, 30);
                w.players[0].weapon = (weapon + 1) % 3;
                develop(&w, &mut fx, 10);
                save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx), &format!("vm-{label}-switch"));
            }

            // Third-person weapons, one of each, posed for the camera.
            {
                let mut w = fresh(MapId::Raindance);
                let origin = w.players[0].pos;
                for i in 0..3 {
                    let mut p = w.players[0].clone();
                    p.pos = origin + Vec3::new((i as f32 - 1.0) * 1.6, 0.0, 0.0);
                    p.yaw = [0.55, 1.2, 2.6][i];
                    p.team = if i == 1 { Team::Glacier } else { Team::Ember };
                    p.weapon = i as u8;
                    p.net_id = 50 + i as u32;
                    w.players.push(p);
                }
                let mut frame = crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut crate::effects::Effects::new());
                let target = origin + Vec3::Y * 1.0;
                frame.eye = target + Vec3::new(0.6, 0.4, -4.2);
                frame.view = Mat4::look_at_rh(frame.eye, target, Vec3::Y);
                frame.proj = crate::drawlist::clip_correct(Mat4::perspective_rh(45f32.to_radians(), aspect, 0.1, 2500.0));
                frame.inv_vp = (frame.proj * frame.view).inverse();
                frame.viewmodel.clear();
                save(&frame, "thirdperson");
            }

            // Rounds in flight: disc crossing the view, grenade with its fuse
            // lit, turret plasma, all staged a few metres ahead.
            {
                let mut w = fresh(MapId::Raindance);
                w.players[0].pos.y += 10.0;
                let (eye, dir, _) = w.camera();
                let side = dir.cross(Vec3::Y).normalize();
                w.discs.push(Disc { pos: eye + dir * 5.0 - Vec3::Y * 0.9 + side * 0.5, vel: side * 95.0, team: Team::Ember,
                    owner: 0, life: 4.88, kind: 0, spin: 1.1 });
                w.discs.push(Disc { pos: eye + dir * 3.2 - Vec3::Y * 0.4 - side * 1.1, vel: dir * 30.0 + Vec3::Y * 3.0,
                    team: Team::Ember, owner: 0, life: 0.2, kind: 2, spin: 0.7 });
                w.discs.push(Disc { pos: eye + dir * 9.0 + Vec3::Y * 0.6 - side * 2.5, vel: -dir * 80.0 + side * 10.0,
                    team: Team::Glacier, owner: usize::MAX, life: 2.93, kind: 3, spin: 0.0 });
                w.players[0].weapon = 2;
                save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut crate::effects::Effects::new()), "rounds-in-flight");
            }

            // Chaingun stream into the ground: real fired rounds, tracers and
            // impact sparks and dust, detected the way a live client sees them.
            {
                let mut w = fresh(MapId::Raindance);
                w.players[0].pos.y += 10.0;
                w.players[0].weapon = 1;
                w.input.weapon = 1;
                w.players[0].pitch = -0.55;
                let mut fx = crate::effects::Effects::new();
                w.input.fire = true;
                for _ in 0..50 {
                    w.tick(dt);
                    w.players[0].pos.y = w.players[0].pos.y.max(0.0);
                    develop(&w, &mut fx, 1);
                }
                save(&crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx), "chaingun-impacts");
            }

            // Layered explosions on grass, snow and sand: early (flash and
            // flames over fresh smoke) and late (smoke, debris, scorch only).
            for (map, label) in [(MapId::Raindance, "holler"), (MapId::SnowblindClone, "frostline"), (MapId::DesertOfDeathClone, "dustreach")] {
                for (kind, what) in [(2u8, "grenade"), (4, "generator")] {
                    let mut w = fresh(map);
                    let size = crate::terrain::info(map).size;
                    let stand = Vec3::new(size * 0.5 + 40.0, 0.0, size * 0.5 - 120.0);
                    let ground = crate::terrain::height_on(map, stand.x, stand.z);
                    w.players[0].pos = Vec3::new(stand.x, ground + 6.0, stand.z);
                    w.players[0].yaw = 0.0;
                    w.players[0].pitch = -0.2;
                    let (eye, dir, _) = w.camera();
                    let flat = Vec3::new(dir.x, 0.0, dir.z).normalize();
                    let spot = eye + flat * if kind == 4 { 34.0 } else { 20.0 };
                    let floor = crate::terrain::height_on(map, spot.x, spot.z);
                    let pos = Vec3::new(spot.x, floor + if kind == 4 { 2.0 } else { 0.3 }, spot.z);
                    let radius = if kind == 4 { crate::sim::GENERATOR_BLAST_RADIUS } else { 9.0 };
                    let mut fx = crate::effects::Effects::new();
                    for (age, when) in [(0.3f32, "early"), (1.0, "late")] {
                        while fx_age(&w) < age {
                            let next = fx_age(&w) + dt;
                            w.explosions.retain(|e| e.age < 0.55);
                            if next < 0.55 {
                                if w.explosions.is_empty() { w.explosions.push(Explosion { pos, age: 0.0, max_r: radius, kind }); }
                                w.explosions[0].age = next;
                            } else { w.explosions.clear(); }
                            w.time += dt;
                            develop(&w, &mut fx, 1);
                            if next >= 0.55 && w.explosions.is_empty() { w.smoke.clear(); }
                            set_fx_age(&mut w, next);
                        }
                        let mut frame = crate::drawlist::build_frame_with(&w, aspect, 0.0, &mut fx);
                        frame.viewmodel.clear();
                        save(&frame, &format!("blast-{what}-{label}-{when}"));
                    }
                }
            }
        });
        // Simulated time since the staged blast, kept outside the explosion
        // record because that record is dropped at 0.55 s.
        fn fx_age(w: &crate::sim::World) -> f32 { w.flyby }
        fn set_fx_age(w: &mut crate::sim::World, age: f32) { w.flyby = age; }
    }

    /// Eight players in one fight: chaingun streams into the ground, discs and
    /// grenades landing, bots doing their own thing. Reports peak draw counts,
    /// CPU frame-build time and GPU render time. Run with --release --ignored.
    #[test]
    #[ignore = "timing probe; requires a GPU adapter"]
    fn heavy_fight_budget() {
        use super::*;
        use crate::sim::{Disc, MatchState, Team, World};
        let mut w = World::new();
        w.set_map(MapId::Raindance);
        w.start_match(true);
        w.state = MatchState::Playing;
        let me = w.player_id;
        let origin = w.players[me].pos;
        while w.players.len() < 8 { let p = w.players[0].clone(); w.players.push(p); }
        for i in 0..8 {
            let p = &mut w.players[i];
            p.alive = true;
            p.net_id = i as u32 + 1;
            p.team = if i % 2 == 0 { Team::Ember } else { Team::Glacier };
            if i != me { p.pos = origin + Vec3::new((i as f32 - 3.5) * 6.0, 0.0, -24.0 - (i % 2) as f32 * 7.0); }
            p.weapon = (i % 3) as u8;
        }
        let dt = 1.0 / 60.0;
        let mut fx = crate::effects::Effects::new();
        let (mut peak_lit, mut peak_emit, mut peak_smoke, mut total) = (0, 0, 0, std::time::Duration::ZERO);
        let mut heaviest: Option<crate::drawlist::DrawFrame> = None;
        let frames = 600;
        for f in 0..frames {
            let (eye, dir, _) = w.camera();
            for i in 0..8 {
                if i == me { continue; }
                let p = w.players[i].clone();
                let from = p.pos + Vec3::Y * 1.2;
                let spot = eye + Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero() * (18.0 + (i * 3) as f32)
                    + Vec3::X * ((f % 40) as f32 - 20.0) * 0.4;
                let target = Vec3::new(spot.x, crate::terrain::height_on(w.map, spot.x, spot.z), spot.z);
                let aim = (target - from).normalize_or_zero();
                let (kind, every, speed, life) = match i % 3 { 1 => (1u8, 5, 420.0, 1.2), 0 => (0, 63, 95.0, 5.0), _ => (2, 51, 48.0, 2.0) };
                if (f + i * 7) % every == 0 {
                    w.discs.push(Disc { pos: from, vel: aim * speed, team: p.team, owner: i, life, kind, spin: 0.0 });
                    w.players[i].shots += 1;
                }
            }
            w.tick(dt);
            let start = std::time::Instant::now();
            let frame = crate::drawlist::build_frame_with(&w, 1.6, dt, &mut fx);
            total += start.elapsed();
            let weight = frame.lit.len() + frame.emit.len() + frame.smoke.len();
            peak_lit = peak_lit.max(frame.lit.len());
            peak_emit = peak_emit.max(frame.emit.len());
            peak_smoke = peak_smoke.max(frame.smoke.len());
            if heaviest.as_ref().is_none_or(|h| h.lit.len() + h.emit.len() + h.smoke.len() < weight) { heaviest = Some(frame); }
        }
        println!("peak draws: lit {peak_lit} emit {peak_emit} smoke {peak_smoke}; build avg {:.3} ms",
            total.as_secs_f64() * 1000.0 / frames as f64);
        let frame = heaviest.unwrap();
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.expect("GPU adapter");
            let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("GPU device");
            let mut scene = SceneGpu::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            let mut times = Vec::new();
            for k in 0..80 {
                let start = std::time::Instant::now();
                let mut encoder = device.create_command_encoder(&Default::default());
                scene.render(&device, &queue, &mut encoder, 1280, 800, &frame);
                queue.submit([encoder.finish()]);
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                if k >= 20 { times.push(start.elapsed().as_secs_f64() * 1000.0); }
            }
            times.sort_by(f64::total_cmp);
            println!("heaviest frame ({} lit, {} emit, {} smoke): render median {:.2} ms, p95 {:.2} ms",
                frame.lit.len(), frame.emit.len(), frame.smoke.len(), times[times.len() / 2], times[times.len() * 95 / 100]);
        });
    }

    #[test]
    fn armor_mesh_has_outward_unit_normals_and_valid_indices() {
        let (vertices,indices)=super::armor_mesh();
        assert!(indices.iter().all(|&i|(i as usize)<vertices.len()/6));
        for v in vertices.chunks_exact(6) {
            let p=glam::Vec3::from_slice(&v[..3]);
            let n=glam::Vec3::from_slice(&v[3..]);
            assert!((n.length()-1.).abs()<0.001 && p.dot(n)>0.);
        }
    }

    #[test]
    #[ignore = "requires a GPU adapter"]
    fn render_grass_captures() {
        use super::*;
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default())
                .await.expect("GPU adapter");
            let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default())
                .await.expect("GPU device");
            let mut scene = SceneGpu::new(&device, wgpu::TextureFormat::Rgba8Unorm);
            std::fs::create_dir_all("screenshots").unwrap();
            let shoot = |scene: &mut SceneGpu, world: &crate::sim::World, name: &str, w: u32, h: u32| {
                let frame = crate::drawlist::build_frame(world, w as f32 / h as f32, 0.016);
                let mut encoder = device.create_command_encoder(&Default::default());
                scene.render(&device, &queue, &mut encoder, w, h, &frame);
                let stride = (w * 4).div_ceil(256) * 256;
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("capture"),
                    size: (stride * h) as u64,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                encoder.copy_texture_to_buffer(
                    scene.color.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(stride),
                            rows_per_image: Some(h),
                        },
                    },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                rx.recv().unwrap().unwrap();
                let mapped = buffer.slice(..).get_mapped_range().unwrap();
                let pixels: Vec<u8> = mapped.chunks(stride as usize)
                    .flat_map(|row| row[..w as usize * 4].iter().copied()).collect();
                let path = if let Ok(key)=std::env::var("QA_COLLECTION") {
                    let map=MapId::parse(&key).expect("known collection map");
                    format!("local-assets/{}/qa-{name}.png",map.key())
                } else if std::env::var_os("QA_STONEHENGE").is_some() {
                    format!("local-assets/stonehenge-clone/qa-{name}.png")
                } else if std::env::var_os("QA_REFERENCE").is_some() {
                    format!("screenshots/broadside-reference-{name}.png")
                } else { format!("screenshots/grass-look-{name}.png") };
                let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), w, h);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
                println!("Rendered {path}");
            };
            let mut world = crate::sim::World::new();
            if let Ok(key)=std::env::var("QA_COLLECTION") {
                let map=MapId::parse(&key).expect("known map");
                world.set_map(map);world.start_match(true);world.players.truncate(1);
                let pack=peakrunner_core::map_pack::on(map).unwrap();
                let validation:serde_json::Value=serde_json::from_slice(&std::fs::read(pack.root.join("validation.json")).unwrap()).unwrap();
                let vec=|v:&serde_json::Value|glam::Vec3::new(v[0].as_f64().unwrap() as f32,v[1].as_f64().unwrap() as f32,v[2].as_f64().unwrap() as f32);
                let camera=vec(&validation["poster"]["camera"]);
                let heading=vec(&validation["poster"]["heading"]);
                let flag=glam::Vec3::from_array(pack.manifest.flags[0]);
                let other=glam::Vec3::from_array(pack.manifest.flags[1]);
                for (name,pos,target) in [
                    ("donut",camera,camera+heading*7.),
                    ("exterior",flag+glam::Vec3::new(90.,70.,90.),flag),
                    ("opposite",other+glam::Vec3::new(-90.,70.,-90.),other),
                    ("spawn",glam::Vec3::from_array(pack.manifest.spawns[0]),other),
                ] {
                    let d=(target-pos).normalize();world.players[0].pos=pos;
                    world.players[0].yaw=(-d.x).atan2(-d.z);world.players[0].pitch=d.y.asin();
                    shoot(&mut scene,&world,name,1280,800);
                }
                // Exercise real CTF rules using each installed map's flag homes.
                world.players[0].pos=world.flags[1].home+glam::Vec3::Y;
                world.tick(1.0/60.0);
                assert!(world.players[0].carrying.is_some(),"enemy flag pickup");
                world.players[0].pos=world.flags[0].home+glam::Vec3::Y;
                world.players[0].vel=glam::Vec3::ZERO;
                world.tick(1.0/60.0);
                assert_eq!(world.score[0],1,"capture uses imported homes");
                world.set_map(MapId::Raindance);
                world.set_map(map);world.start_match(true);
                assert_eq!(world.score,[0,0]);
                assert_eq!(world.flags[0].home,glam::Vec3::from_array(pack.manifest.flags[0]));
                return;
            }
            if std::env::var_os("QA_ROTATION").is_some() {
                for map in [MapId::Raindance, MapId::BroadsideClone,
                    MapId::StonehengeClone, MapId::SnowblindClone, MapId::DesertOfDeathClone,
                    MapId::Raindance] {
                    world.set_map(map); world.start_match(true); world.players.truncate(1);
                    let pack = peakrunner_core::map_pack::on(map).expect("rotation pack installed");
                    let pos = Vec3::from_array(pack.manifest.spawns[0]) + Vec3::Y * 2.;
                    let target = Vec3::from_array(pack.manifest.flags[1]);
                    let d = (target-pos).normalize();
                    world.players[0].pos = pos;
                    world.players[0].yaw = (-d.x).atan2(-d.z);
                    world.players[0].pitch = d.y.asin();
                    shoot(&mut scene, &world, &format!("rotation-{}",map.key()), 1280, 800);
                }
                return;
            }
            if std::env::var_os("QA_STONEHENGE").is_some() {
                world.set_map(if std::env::var_os("QA_INSTALLED_CLONE").is_some() {MapId::StonehengeClone} else {MapId::Raindance});
                world.start_match(true);
                world.players.truncate(1);
                for (name,position,target) in [
                    ("exterior",[890.,340.,735.],[760.,275.,650.]),
                    ("opposite",[860.,315.,1000.],[790.,260.,1120.]),
                    ("donut",[759.182,278.234,636.799],[759.182,278.234,643.799]),
                    ("flag",[762.,288.538,599.392],[762.,288.538,610.]),
                    ("bunker",[755.,270.,641.],[775.,270.,641.]),
                    ("inventory",[770.,287.,633.],[769.73,286.,627.05]),
                ] {
                    let pos=glam::Vec3::from_array(position);
                    let d=(glam::Vec3::from_array(target)-pos).normalize();
                    world.players[0].pos=pos;
                    world.players[0].yaw=(-d.x).atan2(-d.z);
                    world.players[0].pitch=d.y.asin();
                    shoot(&mut scene,&world,name,1280,800);
                }
                return;
            }
            if std::env::var_os("QA_REFERENCE").is_none() {
            world.set_map(MapId::Raindance);
            world.flyby = 0.6;
            shoot(&mut scene, &world, "menu", 1280, 800);
            world.flyby = 2.4;
            shoot(&mut scene, &world, "menu-b", 1280, 800);
            world.start_match(true);
            world.players.truncate(1);
            world.players[0].yaw = std::f32::consts::PI;
            world.players[0].pitch = -0.18;
            shoot(&mut scene, &world, "ahead", 1280, 800);
            world.players[0].pitch = -0.62;
            shoot(&mut scene, &world, "down", 1280, 800);
            }
            if std::env::var_os("QA_REFERENCE").is_some() {
                let capture_map=if std::env::var_os("QA_INSTALLED_CLONE").is_some() {MapId::BroadsideClone} else {MapId::Raindance};
                world.set_map(capture_map);
                world.start_match(true);
                world.players.truncate(1);
                world.players[0].pos=glam::Vec3::new(1184.,220.,1144.);
                world.players[0].yaw=1.06;world.players[0].pitch=-0.36;
                shoot(&mut scene,&world,"exterior",1280,800);
                let pack=peakrunner_core::map_pack::on(capture_map).unwrap();
                assert!(pack.manifest.private_reference,"reference captures need the private pack");
                let base=&pack.manifest.reference_bases[0];
                if std::env::var_os("QA_DONUT").is_some() {
                    let b=pack.manifest.reference_bases.iter().find(|b|b.name=="Base 1").unwrap();
                    let basis=glam::Mat3::from_cols_array_2d(&b.world_to_local).transpose().inverse();
                    world.players[0].pos=glam::Vec3::from_array(b.position)+basis*glam::Vec3::new(0.35,18.01,-1.13);
                    let d=basis*glam::Vec3::Y;
                    world.players[0].yaw=(-d.x).atan2(-d.z);world.players[0].pitch=0.12;
                    shoot(&mut scene,&world,"donut-wall",1280,800);
                }
                let basis=glam::Mat3::from_cols_array_2d(&base.world_to_local).transpose().inverse();
                for (name,position,heading) in [
                    ("entry-approach",[0.,-44.,1.2],[0.,1.,0.]),
                    ("entry",[0.,-29.,1.2],[0.,1.,0.]),
                    ("hall",[0.,-13.,8.2],[0.,1.,0.]),
                    ("flag",[8.,-13.,15.2],[-1.,0.,0.]),
                    ("armory",[0.,-12.,26.2],[0.,1.,0.])
                ] {
                    world.players[0].pos=glam::Vec3::from_array(base.position)+basis*glam::Vec3::from_array(position);
                    let direction=basis*glam::Vec3::from_array(heading);
                    world.players[0].yaw=(-direction.x).atan2(-direction.z);world.players[0].pitch=0.;
                    shoot(&mut scene,&world,name,1280,800);
                }
            }
        });
    }
}
