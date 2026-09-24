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
    shade:wgpu::Texture, shade_size:u32,
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
                texture_entry(4,wgpu::TextureViewDimension::D2),
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
        // Baked terrain shade (sun visibility, ambient occlusion); packs without
        // one get a 1x1 white texture, which leaves lighting unchanged.
        let shade_size=if pack.manifest.files.contains_key("shade.rg") {map_pack::SHADE_SIZE} else {1};
        let shade=device.create_texture(&wgpu::TextureDescriptor {label:Some("terrain shade"),
            size:wgpu::Extent3d {width:shade_size,height:shade_size,depth_or_array_layers:1},mip_level_count:1,sample_count:1,
            dimension:wgpu::TextureDimension::D2,format:wgpu::TextureFormat::Rg8Unorm,
            usage:wgpu::TextureUsages::TEXTURE_BINDING|wgpu::TextureUsages::COPY_DST,view_formats:&[]});
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
            wgpu::BindGroupEntry {binding:4,resource:wgpu::BindingResource::TextureView(&shade.create_view(&Default::default()))},
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
        // Declared water volumes (and QA-staged ones): a flat surface fanned
        // from the outline's centroid, so star-shaped shorelines traced round a
        // pond's centre (scripts/assets/water_bodies.py) fill correctly as well
        // as convex ones. Same translucent water pass.
        let staged=peakrunner_core::water::qa_staged();
        for volume in pack.manifest.water_volumes.iter().chain(&staged) {
            let outline=volume.outline();
            let n=outline.len().max(1) as f32;
            let centre=[outline.iter().map(|q|q[0]).sum::<f32>()/n,outline.iter().map(|q|q[1]).sum::<f32>()/n];
            // Surface tint from the volume's colour, packed for map.wgsl.
            let tint=volume.color.map_or([0.,0.],|c| {
                let b=|v:f32|(v.clamp(0.,1.)*255.).round();
                [b(c[0])*256.+b(c[1]),b(c[2])+1.]
            });
            for k in 0..outline.len() {
                for q in [centre,outline[k],outline[(k+1)%outline.len()]] {
                    let vertex=[q[0],volume.surface,q[1],0.,1.,0.,q[0]/32.0,q[1]/32.0,tint[0],tint[1],pack.manifest.water_layer as f32,-3.];
                    bytes.extend_from_slice(bytemuck::cast_slice(&vertex));
                }
            }
        }
        let count=(bytes.len()/48) as u32;
        let vertices=device.create_buffer_init(&wgpu::util::BufferInitDescriptor {label:Some("source map triangles"),contents:&bytes,usage:wgpu::BufferUsages::VERTEX});
        Some(Self {map,pipeline,sky,water,group,uniform,vertices,count,water_start,images,weights,shade,shade_size,uploaded:false})
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
            let shade=if self.shade_size==1 {vec![255,255]} else {pack.asset("shade.rg").expect("validated shade")};
            queue.write_texture(self.shade.as_image_copy(),&shade,
                wgpu::TexelCopyBufferLayout {offset:0,bytes_per_row:Some(self.shade_size*2),rows_per_image:Some(self.shade_size)},
                wgpu::Extent3d {width:self.shade_size,height:self.shade_size,depth_or_array_layers:1});
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

/// `map.wgsl`'s `U`: the original 224 bytes, seven look vectors and `glow`.
const UNIFORM_FLOATS:usize=88;
/// Texture layer of the kit's `light` material (index 10 of `MATERIALS` in
/// `scripts/build-original-map.py`). Every shipped pack is built through that
/// kit, so light strips, lamps, beacon and capture-tower glow share it.
const KIT_LIGHT_LAYER:f32=10.0;
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
    // hfog.w: world metres to shade-map texture coordinates over the tile.
    let [d,base,fall]=look.height_fog;data.extend([d,base,fall,1.0/(256.0*pack.manifest.terrain_step)]);
    // Bloom mask inputs: emissive layer, its strength, and sun-disc glow.
    let light=if pack.manifest.texture_count as f32>KIT_LIGHT_LAYER {KIT_LIGHT_LAYER} else {-1.0};
    data.extend([light,1.0,1.0,0.0]);
    debug_assert_eq!(data.len(),UNIFORM_FLOATS);
    data
}

#[cfg(test)]
mod tests {
    #[test]
    fn uniform_layout_matches_the_shader_and_defaults_keep_the_old_values() {
        use peakrunner_core::terrain::MapId;
        let frame=crate::drawlist::build_frame(&crate::sim::World::new(),1.6,0.016);
        let maps=[MapId::Raindance,MapId::BroadsideClone,MapId::StonehengeClone,MapId::SnowblindClone,MapId::DesertOfDeathClone];
        for id in maps {
            let pack=super::map_pack::on(id).unwrap();
            let data=super::uniform_data(pack,&frame);
            assert_eq!(data.len(),super::UNIFORM_FLOATS);
            // Sun and fog sit where the shader always read them.
            assert_eq!(&data[36..39],&pack.manifest.look.resolved().sun_direction,"{id:?}");
            assert_eq!(&data[40..43],&pack.fog_color(),"{id:?}");
        }
        // A pack with no look fields keeps the old values: the historical sun,
        // exposure 1, cubemap sky mode, sun disc on, height fog off. Shipped
        // maps may all set a look, so check any that does not, if one exists.
        if let Some(pack)=maps.iter().map(|&id|super::map_pack::on(id).unwrap())
            .find(|p|p.manifest.look==peakrunner_core::look::Look::default()) {
            let data=super::uniform_data(pack,&frame);
            assert_eq!(&data[36..39],&peakrunner_core::look::DEFAULT_SUN);
            assert_eq!(data[59],1.0);assert_eq!(data[63],0.0);assert_eq!(data[67],1.0);assert_eq!(data[80],0.0);
        }
        let empty=peakrunner_core::look::Look::default().resolved();
        assert_eq!(empty.sun_direction,peakrunner_core::look::DEFAULT_SUN);
        assert_eq!(empty.exposure,1.0);assert!(empty.sky.is_none());
        let wgsl=include_str!("map.wgsl");
        assert!(wgsl.contains("hfog: vec4<f32>,"),"map.wgsl uniform must keep the look block");
        assert!(wgsl.contains("glow: vec4<f32>,"),"map.wgsl uniform must end with the glow block");
    }

    #[test]
    fn every_pack_marks_the_kit_light_layer_as_emissive() {
        use peakrunner_core::terrain::MapId;
        let frame=crate::drawlist::build_frame(&crate::sim::World::new(),1.6,0.016);
        for id in [MapId::Raindance,MapId::BroadsideClone,MapId::StonehengeClone,MapId::SnowblindClone,MapId::DesertOfDeathClone] {
            let pack=super::map_pack::on(id).unwrap();
            let data=super::uniform_data(pack,&frame);
            assert_eq!(&data[84..87],&[super::KIT_LIGHT_LAYER,1.0,1.0],"{id:?}");
            // Layer 10 must be a material, not a sky face or lightmap page.
            assert!(!pack.manifest.sky_layers.contains(&10),"{id:?}");
            assert!(!pack.manifest.terrain_layers.contains(&10),"{id:?}");
        }
        // The one pack that records its material names confirms the index.
        let raw:serde_json::Value=serde_json::from_str(include_str!("../assets/maps/raindance/map.json")).unwrap();
        assert_eq!(raw["materials"][super::KIT_LIGHT_LAYER as usize],"light");
    }
}
