struct U {
    vp: mat4x4<f32>, inv_vp: mat4x4<f32>, eye: vec4<f32>, sun: vec4<f32>,
    fog: vec4<f32>, layers: vec4<f32>, sky0: vec4<f32>, sky1: vec4<f32>,
}
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var images: texture_2d_array<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var weights: texture_2d<f32>;
struct In {
    @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>, @location(3) lmuv: vec2<f32>,
    @location(4) layer: vec2<f32>,
}
struct Out {
    @builtin(position) pos: vec4<f32>, @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>,
    @location(3) lmuv: vec2<f32>, @location(4) @interpolate(flat) layer: vec2<f32>,
}
fn map_noise(p:vec2<f32>)->f32 {
    let i=floor(p);let f=fract(p);let t=f*f*(3.0-2.0*f);
    let corners=vec4<f32>(dot(i,vec2<f32>(127.1,311.7)),dot(i+vec2<f32>(1,0),vec2<f32>(127.1,311.7)),
        dot(i+vec2<f32>(0,1),vec2<f32>(127.1,311.7)),dot(i+vec2<f32>(1,1),vec2<f32>(127.1,311.7)));
    let n=fract(sin(corners)*43758.5453);
    return mix(mix(n.x,n.y,t.x),mix(n.z,n.w,t.x),t.y);
}
@vertex fn vs_map(v: In) -> Out {
    var o: Out; o.pos=u.vp*vec4<f32>(v.pos,1.0); o.world=v.pos;
    o.normal=v.normal; o.uv=v.uv; o.lmuv=v.lmuv; o.layer=v.layer; return o;
}
@fragment fn fs_map(v: Out) -> @location(0) vec4<f32> {
    var color=textureSample(images,samp,v.uv,i32(v.layer.x));
    var n=normalize(v.normal);
    if (v.layer.y == -2.0) {
        // Original four painted terrain layers, fixed 8m texture scale.
        var w=textureSample(weights,samp,v.lmuv);
        w/=max(dot(w,vec4<f32>(1.0)),0.0001);
        color=textureSample(images,samp,v.uv,i32(u.layers.x))*w.x
             +textureSample(images,samp,v.uv,i32(u.layers.y))*w.y
             +textureSample(images,samp,v.uv,i32(u.layers.z))*w.z
             +textureSample(images,samp,v.uv,i32(u.layers.w))*w.w;
        // World-scale variation suppresses visible texture repetition without
        // moving UVs or changing the geometry beneath a skier.
        color=vec4<f32>(color.rgb*(0.80+0.35*map_noise(v.world.xz/45.0)+0.10*map_noise(v.world.xz/11.0)),color.a);
    }
    if (color.a<0.4) {discard;}
    var light=vec3<f32>(0.55+0.45*max(dot(n,u.sun.xyz),0.0));
    if (v.layer.y>=0.0) {
        light=max(vec3<f32>(0.22),textureSampleLevel(images,samp,clamp(v.lmuv,vec2<f32>(0.002),vec2<f32>(0.998)),i32(v.layer.y),0.0).rgb);
    }
    if (v.layer.y<=-4.0) {
        let baked=textureSampleLevel(images,samp,clamp(v.lmuv,vec2<f32>(0.002),vec2<f32>(0.998)),i32(-4.0-v.layer.y),0.0).rgb;
        light=max(baked,vec3<f32>(0.55+0.7*max(dot(n,u.sun.xyz),0.0)));
    }
    var rgb=color.rgb*light;
    let fog=smoothstep(u.fog.w,u.eye.w,distance(u.eye.xyz,v.world));
    rgb=mix(rgb,u.fog.rgb,fog);
    return vec4<f32>(rgb,select(1.0,0.6,v.layer.y == -3.0));
}
struct SkyOut { @builtin(position) pos:vec4<f32>, @location(0) ndc:vec2<f32> }
@vertex fn vs_map_sky(@builtin(vertex_index) i:u32)->SkyOut {
    let points=array<vec2<f32>,3>(vec2<f32>(-1,-1),vec2<f32>(3,-1),vec2<f32>(-1,3));
    var o:SkyOut;o.pos=vec4<f32>(points[i],0.9999,1);o.ndc=points[i];return o;
}
@fragment fn fs_map_sky(v:SkyOut)->@location(0) vec4<f32> {
    let wp=u.inv_vp*vec4<f32>(v.ndc,1,1);
    let d=normalize(wp.xyz/wp.w-u.eye.xyz);let a=abs(d);
    var uv=vec2<f32>(0);var layer=u.sky0.x;
    if(a.y>max(a.x,a.z)) {
        uv=vec2<f32>(d.x,-d.z)/a.y;layer=u.sky1.x;
        if(d.y<0.0) {return vec4<f32>(u.fog.rgb,1);}
    } else if(a.x>a.z) {
        uv=vec2<f32>(select(-d.z,d.z,d.x>0.0),-d.y)/a.x;
        layer=select(u.sky0.w,u.sky0.y,d.x>0.0);
    } else {
        uv=vec2<f32>(select(d.x,-d.x,d.z>0.0),-d.y)/a.z;
        layer=select(u.sky0.x,u.sky0.z,d.z>0.0);
    }
    let c=textureSampleLevel(images,samp,clamp(uv*0.5+0.5,vec2<f32>(0.002),vec2<f32>(0.998)),i32(layer),0.0).rgb;
    return vec4<f32>(mix(u.fog.rgb,c,smoothstep(-0.02,0.12,d.y)),1);
}
