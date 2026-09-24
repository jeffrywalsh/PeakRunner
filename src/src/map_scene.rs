//! Renderer for locally converted source-map geometry and painted materials.
use eframe::egui_wgpu::wgpu;
use wgpu::util::DeviceExt;
use crate::drawlist::DrawFrame;
use peakrunner_core::{map_pack,terrain::{self,MapId}};

pub struct MapGpu {
    map: MapId,
    pipeline:wgpu::RenderPipeline, sky:wgpu::RenderPipeline, water:wgpu::RenderPipeline,
    group:wgpu::BindGroup, uniform:wgpu::Buffer, vertices:wgpu::Buffer,
    count:u32, water_start:u32, images:wgpu::Texture, weights:wgpu::Texture,
    uploaded:bool,
}

impl MapGpu {
    pub fn new(device:&wgpu::Device,map:MapId,samples:u32)->Option<Self> {
        let pack=map_pack::on(map)?;
        let layout=device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label:Some("imported map"),entries:&[
                wgpu::BindGroupLayoutEntry {binding:0,visibility:wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty:wgpu::BindingType::Buffer {ty:wgpu::BufferBindingType::Uniform,has_dynamic_offset:false,min_binding_size:None},count:None},
                texture_entry(1,wgpu::TextureViewDimension::D2Array),
                wgpu::BindGroupLayoutEntry {binding:2,visibility:wgpu::ShaderStages::FRAGMENT,
                    ty:wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),count:None},
                texture_entry(3,wgpu::TextureViewDimension::D2),
            ],
        });
        let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor {label:Some("source map"),source:wgpu::ShaderSource::Wgsl(include_str!("map.wgsl").into())});
        let pl=device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {label:Some("map"),bind_group_layouts:&[Some(&layout)],immediate_size:0});
        let attrs=wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32x2,4=>Float32x2];
        let vb=wgpu::VertexBufferLayout {array_stride:48,step_mode:wgpu::VertexStepMode::Vertex,attributes:&attrs};
        let buffers=[Some(vb)];
        let make=|sky:bool,water:bool|device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("map pass"),layout:Some(&pl),
            vertex:wgpu::VertexState {module:&shader,entry_point:Some(if sky {"vs_map_sky"} else {"vs_map"}),
                compilation_options:Default::default(),buffers:if sky {&[]} else {&buffers}},
            primitive:wgpu::PrimitiveState {cull_mode:None,..Default::default()},
            depth_stencil:Some(wgpu::DepthStencilState {format:wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled:Some(!sky&&!water),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),
            multisample:wgpu::MultisampleState {count:samples,..Default::default()},fragment:Some(wgpu::FragmentState {module:&shader,
                entry_point:Some(if sky {"fs_map_sky"} else {"fs_map"}),compilation_options:Default::default(),
                targets:&[Some(wgpu::ColorTargetState {format:wgpu::TextureFormat::Rgba8Unorm,
                    blend:water.then_some(wgpu::BlendState::ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),multiview_mask:None,cache:None,
        });
        let pipeline=make(false,false);let sky=make(true,false);let water=make(false,true);
        let images=texture(device,pack.manifest.texture_count,9);
        let weights=texture(device,1,1);
        let sampler=device.create_sampler(&wgpu::SamplerDescriptor {label:Some("map repeat"),
            address_mode_u:wgpu::AddressMode::Repeat,address_mode_v:wgpu::AddressMode::Repeat,
            mag_filter:wgpu::FilterMode::Linear,min_filter:wgpu::FilterMode::Linear,mipmap_filter:wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp:8,..Default::default()});
        let uniform=device.create_buffer(&wgpu::BufferDescriptor {label:Some("map uniforms"),size:UNIFORM_BYTES,usage:wgpu::BufferUsages::UNIFORM|wgpu::BufferUsages::COPY_DST,mapped_at_creation:false});
        let group=device.create_bind_group(&wgpu::BindGroupDescriptor {label:Some("map"),layout:&layout,entries:&[
            wgpu::BindGroupEntry {binding:0,resource:uniform.as_entire_binding()},
            wgpu::BindGroupEntry {binding:1,resource:wgpu::BindingResource::TextureView(&images.create_view(&wgpu::TextureViewDescriptor {dimension:Some(wgpu::TextureViewDimension::D2Array),..Default::default()}))},
            wgpu::BindGroupEntry {binding:2,resource:wgpu::BindingResource::Sampler(&sampler)},
            wgpu::BindGroupEntry {binding:3,resource:wgpu::BindingResource::TextureView(&weights.create_view(&Default::default()))},
        ]});
        let mut bytes=pack.asset("vertices.bin").expect("validated map vertices");
        let (terrain,indices)=terrain::sample_mesh_of(map);
        let step=pack.manifest.terrain_step;
        for index in indices {
            let v=&terrain[index as usize*6..][..6];
            let vertex=[v[0],v[1],v[2],v[3],v[4],v[5],v[0]/8.0,v[2]/8.0,
                (v[0]/step+0.5)/256.0,(v[2]/step+0.5)/256.0,0.0,-2.0];
            bytes.extend_from_slice(bytemuck::cast_slice(&vertex));
        }
        let water_start=(bytes.len()/48) as u32;
        let parse=|key:&str|->Vec<f32> {pack.manifest.water[key].split_whitespace().map(|s|s.parse().unwrap()).collect()};
        let p=parse("position");let s=parse("scale");
        for (x,z) in [(0.,0.),(1.,0.),(0.,1.),(1.,0.),(1.,1.),(0.,1.)].into_iter().filter(|_|pack.manifest.water_enabled) {
            let vertex=[p[0]+1024.0+x*s[0],p[2],p[1]+1024.0+z*s[1],0.,1.,0.,x*s[0]/32.0,z*s[1]/32.0,0.,0.,pack.manifest.water_layer as f32,-3.];
            bytes.extend_from_slice(bytemuck::cast_slice(&vertex));
        }
        let count=(bytes.len()/48) as u32;
        let vertices=device.create_buffer_init(&wgpu::util::BufferInitDescriptor {label:Some("source map triangles"),contents:&bytes,usage:wgpu::BufferUsages::VERTEX});
        Some(Self {map,pipeline,sky,water,group,uniform,vertices,count,water_start,images,weights,uploaded:false})
    }

    pub fn update(&mut self,queue:&wgpu::Queue,frame:&DrawFrame) {
        let pack=map_pack::on(self.map).unwrap();
        if !self.uploaded {
            for (name,texture,layers,mips) in [("textures.rgba",&self.images,pack.manifest.texture_count,9),("weights.rgba",&self.weights,1,1)] {
                let bytes=pack.asset(name).expect("validated texture");
                let mut offset=0;
                for level in 0..mips {
                    let side=256>>level;let length=(side*side*4*layers) as usize;
                    let mut copy=texture.as_image_copy();copy.mip_level=level;
                    queue.write_texture(copy,&bytes[offset..offset+length],
                        wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(side*4),rows_per_image:Some(side)},
                        wgpu::Extent3d {width:side,height:side,depth_or_array_layers:layers});
                    offset+=length;
                }
            }
            self.uploaded=true;
        }
        let data=uniform_data(pack,frame);
        queue.write_buffer(&self.uniform,0,bytemuck::cast_slice(&data));
    }

    pub fn draw(&self,pass:&mut wgpu::RenderPass<'_>) {
        pass.set_bind_group(0,&self.group,&[]);
        pass.set_pipeline(&self.sky);pass.draw(0..3,0..1);
        pass.set_pipeline(&self.pipeline);pass.set_vertex_buffer(0,self.vertices.slice(..));pass.draw(0..self.water_start,0..1);
        pass.set_pipeline(&self.water);pass.draw(self.water_start..self.count,0..1);
    }
}
fn texture_entry(binding:u32,dimension:wgpu::TextureViewDimension)->wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {binding,visibility:wgpu::ShaderStages::FRAGMENT,
        ty:wgpu::BindingType::Texture {sample_type:wgpu::TextureSampleType::Float {filterable:true},view_dimension:dimension,multisampled:false},count:None}
}
fn texture(device:&wgpu::Device,layers:u32,mips:u32)->wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {label:Some("original map textures"),
        size:wgpu::Extent3d {width:256,height:256,depth_or_array_layers:layers},mip_level_count:mips,sample_count:1,
        dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rgba8Unorm,
        usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[]})
}

/// QA-only: `QA_LOOK` holds a manifest `look` object (JSON) applied to every
/// map, so a sky or fog can be previewed before a pack is rebuilt. Invalid
/// JSON or values are ignored. Absent the variable, packs render as authored.
fn qa_look()->Option<&'static peakrunner_core::look::Look> {
    static LOOK:std::sync::OnceLock<Option<peakrunner_core::look::Look>>=std::sync::OnceLock::new();
    LOOK.get_or_init(|| {
        #[cfg(target_arch="wasm32")] {None}
        #[cfg(not(target_arch="wasm32"))] {
            let raw=std::env::var("QA_LOOK").ok()?;
            let look:peakrunner_core::look::Look=serde_json::from_str(&raw).ok()?;
            look.validate().ok().map(|_|look)
        }
    }).as_ref()
}

/// `map.wgsl`'s `U`: the original 224 bytes plus seven look vectors.
const UNIFORM_FLOATS:usize=84;
const UNIFORM_BYTES:u64=(UNIFORM_FLOATS*4) as u64;

fn uniform_data(pack:&map_pack::MapPack,frame:&DrawFrame)->Vec<f32> {
    let mut data=Vec::with_capacity(UNIFORM_FLOATS);
    data.extend((frame.proj*frame.view).to_cols_array());data.extend(frame.inv_vp.to_cols_array());
    let sky=&pack.manifest.sky;
    let far=sky.get("visibleDistance").and_then(|s|s.parse().ok()).unwrap_or(450.0);
    let near=sky.get("fogDistance").and_then(|s|s.parse().ok()).unwrap_or(200.0);
    let look=qa_look().unwrap_or(&pack.manifest.look).resolved();
    let [sx,sy,sz]=look.sun_direction;
    data.extend([frame.eye.x,frame.eye.y,frame.eye.z,far]);data.extend([sx,sy,sz,frame.time]);
    let [fr,fg,fb]=pack.fog_color();data.extend([fr,fg,fb,near]);data.extend(pack.manifest.terrain_layers.map(|n|n as f32));
    for i in 0..8 {data.push(*pack.manifest.sky_layers.get(i).unwrap_or(&0) as f32);}
    let procedural=look.sky;
    let sky_mode=if procedural.is_some() {1.0} else {0.0};
    let p=procedural.unwrap_or(peakrunner_core::look::ResolvedSky {zenith:[0.;3],horizon:[0.;3],cloud_cover:0.,
        cloud_color:[0.;3],cloud_scale:1.,sun_size:0.022});
    let [r,g,b]=look.sun_color;data.extend([r,g,b,look.exposure]);
    let [r,g,b]=look.ambient_sky;data.extend([r,g,b,sky_mode]);
    let [r,g,b]=look.ambient_ground;data.extend([r,g,b,look.sun_disc]);
    let [r,g,b]=p.zenith;data.extend([r,g,b,p.cloud_cover]);
    let [r,g,b]=p.horizon;data.extend([r,g,b,p.sun_size]);
    let [r,g,b]=p.cloud_color;data.extend([r,g,b,p.cloud_scale]);
    let [d,base,fall]=look.height_fog;data.extend([d,base,fall,0.0]);
    debug_assert_eq!(data.len(),UNIFORM_FLOATS);
    data
}

#[cfg(test)]
mod tests {
    #[test]
    fn uniform_layout_matches_the_shader_and_defaults_keep_the_old_values() {
        use peakrunner_core::terrain::MapId;
        // Cairnhold (key stonehenge-clone) sets no look fields.
        let pack=super::map_pack::on(MapId::StonehengeClone).unwrap();
        let frame=crate::drawlist::build_frame(&crate::sim::World::new(),1.6,0.016);
        let data=super::uniform_data(pack,&frame);
        assert_eq!(data.len(),super::UNIFORM_FLOATS);
        // Sun and the historical grey fog sit where the shader always read them.
        assert_eq!(&data[36..39],&peakrunner_core::look::DEFAULT_SUN);
        assert_eq!(&data[40..43],&pack.fog_color());
        // No look fields: cubemap sky, unit exposure, no height fog.
        // Exposure 1, cubemap sky mode, sun disc on, height fog off.
        assert_eq!(data[59],1.0);assert_eq!(data[63],0.0);assert_eq!(data[67],1.0);assert_eq!(data[80],0.0);
        let wgsl=include_str!("map.wgsl");
        assert!(wgsl.contains("hfog: vec4<f32>,"),"map.wgsl uniform must end with the look block");
    }
}
