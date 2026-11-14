//! Render hook points for native mods (docs/native-mods.md): empty functions the renderer calls at
//! fixed places of a frame, which a mod's mixins hook like those of
//! `horizon-client/src/game/native_hooks.rs`, and listed with them in `native-hooks.json`. API
//! 58.
use super::Vertex;
use super::pack_draw::EntitySpan;
use glam::{DVec3, Mat4};
use std::sync::{OnceLock, atomic::{AtomicBool, Ordering}};
mod generated {
    include!(concat!(env!("OUT_DIR"), "/native_render_generated.rs"));
}
pub use generated::{HandDrawnEvent, RenderHooks, RenderNeedsEvent, WorldDrawnEvent};
pub use wgpu;
static RENDERERS: OnceLock<Box<[&'static dyn RenderHooks]>> = OnceLock::new();
/// Install trait mod renderers in dispatch order, once per process.
pub fn install(
    renderers: Box<[&'static dyn RenderHooks]>,
) -> Result<(), Box<[&'static dyn RenderHooks]>> {
    RENDERERS.set(renderers)
}
static MODS_BUILT_IN: AtomicBool = AtomicBool::new(false);
/// The game was built with native mods (the game sets it at start): one of them may draw in
/// [`native_world_drawn`], so the first-person hand keeps its own pass after it.
pub fn set_mods_built_in(on: bool) {
    MODS_BUILT_IN.store(on, Ordering::Relaxed);
}
/// Whether [`native_world_drawn`] may draw ([`set_mods_built_in`]).
pub fn world_drawn_hooked() -> bool {
    MODS_BUILT_IN.load(Ordering::Relaxed)
        || RENDERERS.get().is_some_and(|renderers| !renderers.is_empty())
}
/// What a mod can draw with once the world is drawn: after the sky, terrain, entities,
/// particles, weather and the world post pass, before the first-person hand ([`HandDrawn`]),
/// the HUD and the screens. `color` holds the world's picture (copy it out before reading it: it is the target
/// drawn into), `depth` the world's depth, reversed (0 is the sky).
#[non_exhaustive]
pub struct WorldDrawn<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// The frame's target and a view of it in `color_format`.
    pub color: &'a wgpu::Texture,
    pub color_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    /// The size of `color`.
    pub size: [u32; 2],
    /// The world's depth (Depth32Float, sampleable), and its size: smaller than `size` when the
    /// world draws at a lower resolution. It holds the frame's depth only when a mod asked for
    /// it ([`RenderNeeds::world_depth`]) or something else kept it.
    pub depth: &'a wgpu::TextureView,
    pub depth_size: [u32; 2],
    /// Camera-relative world to clip space, without TAA's jitter, and with it (what the world
    /// was drawn with).
    pub view_proj: Mat4,
    pub jittered: Mat4,
    /// The camera's position in the world.
    pub camera: DVec3,
    /// The camera-relative entity geometry of the frame (`Frame::entities`), quads of 4
    /// vertices, and its runs by entity when a mod asked for them
    /// ([`RenderNeeds::entity_spans`]) or a shader pack draws them.
    pub entities: &'a [Vertex],
    pub spans: &'a [EntitySpan],
    /// Which entity each run of `entities` is, recorded every frame (`Frame::entity_motion`):
    /// the run's first vertex and the entity, `(1 << 32) | id` for a mob or a player and
    /// `(2 << 32) | id` for a dropped item (their entity ids), 0 for anything else. A run lasts
    /// until the next. Unlike `spans`, whose ids name a type, these follow one entity from
    /// frame to frame (API 371).
    pub motion: &'a [(u32, u64)],
    /// The shared atlas entity UVs point into.
    pub atlas: Option<&'a wgpu::Texture>,
    /// Quad indices (0 1 2 0 2 3 for each 4 vertices), for as many quads as `entities` holds.
    pub quad_indices: &'a wgpu::Buffer,
    /// The block textures the terrain samples, for drawing blocks with the player's own
    /// textures (`native_block_sprites` says where a block's are). API 380.
    pub block_sprites: BlockSpriteArrays<'a>,
}
/// The block sprite arrays ([`WorldDrawn::block_sprites`]): each texture a whole square layer,
/// its UVs 0 to 1 across it, as the world uses them (gamma-encoded, animated layers at the frame
/// shown).
#[non_exhaustive]
pub struct BlockSpriteArrays<'a> {
    /// The arrays as `D2Array` views, terrain.wgsl's eight (`sprites`, `sprites_1`...): a 1-layer
    /// stand-in past the ones in use, each with the mip levels the Mipmap Levels option leaves.
    pub views: &'a [wgpu::TextureView],
    /// Layers per array on this device (2048 on most; fewer where the GPU holds fewer, and
    /// OpenGL pads each array past it): a sprite's `array` and `layer` are cut by it.
    pub split: u32,
    /// A layer's width and height in texels.
    pub size: u32,
    /// Nearest, repeating, the nearest mip level: texels drawn sharp.
    pub nearest: &'a wgpu::Sampler,
    /// The terrain's own (linear, clamped, anisotropic under that Texture Filtering).
    pub filtered: &'a wgpu::Sampler,
}
/// What a mod can draw with once the first-person hand is drawn ([`native_hand_drawn`]): after
/// the world and the hand, before entity outlines, post effects, screen effects and the HUD.
/// `color` holds the frame's picture with the hand in it (copy it out before reading it).
#[non_exhaustive]
pub struct HandDrawn<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// The frame's target and a view of it in `color_format`.
    pub color: &'a wgpu::Texture,
    pub color_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    /// The size of `color`.
    pub size: [u32; 2],
    /// The hand and held item just drawn, camera-relative quads of 4 vertices: empty when no
    /// hand drew (third person, F1, a sleeping or spectating player) or a shader pack drew it.
    pub hand: &'a [Vertex],
    /// Camera-relative world to clip space for the hand: its own field of view
    /// (`hand_fov_degrees`) and the view bob, without TAA's jitter (the hand has none). Its
    /// depth is the hand's own: the hand's pass clears depth and keeps none.
    pub view_proj: Mat4,
    /// The camera's position in the world.
    pub camera: DVec3,
    /// The shared atlas the hand's UVs point into.
    pub atlas: Option<&'a wgpu::Texture>,
    /// Quad indices (0 1 2 0 2 3 for each 4 vertices).
    pub quad_indices: &'a wgpu::Buffer,
}
/// What the mods need from a frame that the game doesn't make for itself. Each mod sets what it
/// needs and leaves the rest, so several can ask.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderNeeds {
    /// Record `Frame::entity_spans` without a shader pack: entity runs get a stable id from
    /// their type's name, block entity runs their block state.
    pub entity_spans: bool,
    /// Keep the world pass's depth for [`native_world_drawn`] when nothing else does.
    pub world_depth: bool,
}
/// The world is drawn (see [`WorldDrawn`]). Not called on the title screen's panorama.
#[inline(never)]
pub fn native_world_drawn(world: &mut WorldDrawn<'_>) {
    {
        fn motion_blur_world_drawn(
            world: &mut crate::gfx::native_render::WorldDrawn<'_>,
        ) {
            crate::gfx::native_render::motion_blur::world_drawn(world);
        }
        motion_blur_world_drawn(&mut *world)
    };
    generated::dispatch_world_drawn(
        RENDERERS.get().map_or(&[], |renderers| renderers),
        world,
    );
}
/// The first-person hand is drawn (see [`HandDrawn`]), on every frame [`native_world_drawn`]
/// runs, with no hand too. API 364.
#[inline(never)]
pub fn native_hand_drawn(hand: &mut HandDrawn<'_>) {
    generated::dispatch_hand_drawn(
        RENDERERS.get().map_or(&[], |renderers| renderers),
        hand,
    );
}
/// Asked as a frame is built and drawn: set what your mod needs.
#[inline(never)]
pub fn native_render_needs(needs: &mut RenderNeeds) {
    {
        fn motion_blur_needs(needs: &mut crate::gfx::native_render::RenderNeeds) {
            if crate::gfx::native_render::motion_blur::active() {
                needs.entity_spans = true;
                needs.world_depth = true;
            }
        }
        motion_blur_needs(&mut *needs)
    };
    generated::dispatch_render_needs(
        RENDERERS.get().map_or(&[], |renderers| renderers),
        needs,
    );
}
/// What the mods need this frame ([`native_render_needs`]).
pub fn render_needs() -> RenderNeeds {
    let mut needs = RenderNeeds::default();
    native_render_needs(&mut needs);
    needs
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Depth;
    impl RenderHooks for Depth {
        fn render_needs(&self, event: &mut RenderNeedsEvent<'_>) {
            event.needs.world_depth = true;
        }
    }
    struct Spans;
    impl RenderHooks for Spans {
        fn render_needs(&self, event: &mut RenderNeedsEvent<'_>) {
            event.needs.entity_spans = event.needs.world_depth;
        }
    }
    /// A mod samples `BlockSpriteArrays` as the renderer hands them over: a layer of the second
    /// array through its view, texels sharp and repeating with the nearest sampler.
    #[test]
    fn a_pass_samples_a_sprite_layer_sharp() {
        let instance = crate::gfx::test_gpu::instance();
        let Some((device, queue)) = pollster::block_on(
                instance.request_adapter(&Default::default()),
            )
            .ok()
            .and_then(|adapter| {
                pollster::block_on(adapter.request_device(&Default::default())).ok()
            }) else {
            return;
        };
        let gl = device.adapter_info().backend == wgpu::Backend::Gl;
        let layers = super::super::array_layers(2, gl);
        let quad = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255; 4]];
        let textures: Vec<wgpu::Texture> = (0..2)
            .map(|array| {
                let texture = device
                    .create_texture(
                        &wgpu::TextureDescriptor {
                            label: None,
                            size: wgpu::Extent3d {
                                width: 2,
                                height: 2,
                                depth_or_array_layers: layers,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING
                                | wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        },
                    );
                let mut texels = vec![0u8; 16 * layers as usize];
                if array == 1 {
                    texels[16..32].copy_from_slice(quad.as_flattened());
                }
                queue
                    .write_texture(
                        texture.as_image_copy(),
                        &texels,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(8),
                            rows_per_image: Some(2),
                        },
                        texture.size(),
                    );
                texture
            })
            .collect();
        let unused = textures[0]
            .create_view(
                &wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                    ..Default::default()
                },
            );
        let views = super::super::sprite_views(&textures, &unused, Some(1));
        let nearest = device.create_sampler(&super::super::SPRITE_NEAREST);
        let shader = device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    "@group(0) @binding(0) var sprites: texture_2d_array<f32>;
                 @group(0) @binding(1) var sprites_1: texture_2d_array<f32>;
                 @group(0) @binding(2) var nearest: sampler;
                 @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                     return vec4(f32(i & 1u) * 4.0 - 1.0, f32(i >> 1u) * 4.0 - 1.0, 0.0, 1.0);
                 }
                 @fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
                     // inside each texel, and past the edge (repeating); the last reads array 0
                     let uv = array(vec2(1.3, 0.2), vec2(0.7, 0.45), vec2(-0.8, 0.9), vec2(0.6, 0.6));
                     let x = u32(at.x);
                     if x == 4u {
                         return textureSampleLevel(sprites, nearest, vec2(0.3), 1, 0.0);
                     }
                     return textureSampleLevel(sprites_1, nearest, uv[x], 1, 0.0);
                 }"
                        .into(),
                ),
            });
        let pipeline = device
            .create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: None,
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                },
            );
        let group = device
            .create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&views[0]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&views[1]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&nearest),
                        },
                    ],
                },
            );
        let target = device
            .create_texture(
                &wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: 5,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                },
            );
        let out = device
            .create_buffer(
                &wgpu::BufferDescriptor {
                    label: None,
                    size: u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            );
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let view = target.create_view(&Default::default());
            let mut pass = encoder
                .begin_render_pass(
                    &wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[
                            Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            }),
                        ],
                        ..Default::default()
                    },
                );
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder
            .copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &out,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
                        rows_per_image: None,
                    },
                },
                target.size(),
            );
        queue.submit([encoder.finish()]);
        out.slice(..).map_async(wgpu::MapMode::Read, |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let texels = out.slice(..).get_mapped_range().unwrap()[..20].to_vec();
        let mut expected = quad.as_flattened().to_vec();
        expected.extend([0, 0, 0, 0]);
        assert_eq!(texels, expected);
    }
    #[test]
    fn needs_accumulate_in_mod_order() {
        let mut needs = RenderNeeds::default();
        generated::dispatch_render_needs(&[&Depth, &Spans], &mut needs);
        assert_eq!(needs, RenderNeeds { world_depth : true, entity_spans : true });
    }
}
pub mod motion_blur {
    use crate::gfx::native_render::WorldDrawn;
    use crate::gfx::pack_draw::EntitySpan;
    use glam::{DVec3, Mat4, Vec3};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;
    /// The mod's id: its row in Options > Mods and its settings folder.
    pub const ID: &str = "motion_blur";
    pub const VERSION: &str = "0.1.0";
    pub const DESCRIPTION: &str = "Motion blur from the renderer's motion vectors: moving mobs, items and vehicles, and optionally the camera.";
    /// The settings page's choices, in the order a click cycles through them.
    const SCALES: [f32; 9] = [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];
    const SHUTTERS: [f32; 6] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
    /// 0: by shutter, else a fixed exposure in milliseconds.
    const EXPOSURES: [f32; 5] = [0.0, 8.0, 12.0, 16.0, 33.0];
    /// The longest blur in pixels; the last is no limit.
    const MAX_BLURS: [f32; 7] = [32.0, 64.0, 128.0, 256.0, 512.0, 1024.0, 8192.0];
    const SAMPLES: [u32; 6] = [6, 8, 12, 16, 24, 32];
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Settings {
        pub enabled: bool,
        /// Scale of the blur from moving things: mobs, players, items, minecarts, boats.
        pub objects: f32,
        /// Scale of the blur from the camera's own motion (turning, walking).
        pub camera: f32,
        /// How long the shutter stays open, as a part of a frame: 0.5 is a 180 degree shutter.
        pub shutter: f32,
        /// Above 0, a fixed exposure in milliseconds instead: the same blur at any frame rate.
        pub exposure_ms: f32,
        /// The longest blur, in pixels.
        pub max_blur: f32,
        /// Samples along the blur: more is smoother and slower.
        pub samples: u32,
        /// Blur your own body in third person.
        pub blur_self: bool,
        /// Show the motion as colour instead of blurring.
        pub debug: bool,
    }
    impl Default for Settings {
        fn default() -> Self {
            Self {
                enabled: true,
                objects: 1.0,
                camera: 0.0,
                shutter: 0.5,
                exposure_ms: 0.0,
                max_blur: 64.0,
                samples: 12,
                blur_self: false,
                debug: false,
            }
        }
    }
    /// The next of `choices` after `value` (the first after the last, or after a value between).
    fn next<T: Copy + PartialOrd>(choices: &[T], value: T) -> T {
        choices.iter().copied().find(|c| *c > value).unwrap_or(choices[0])
    }
    impl Settings {
        /// From the kept values (`get` reads one key); the defaults for those missing or unreadable.
        pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
            let d = Self::default();
            let num = |key: &str, min: f32, max: f32, or: f32| {
                get(key)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .map_or(or, |v| v.clamp(min, max))
            };
            let flag = |key: &str, or: bool| get(key).map_or(or, |v| v == "true");
            Self {
                enabled: flag("enabled", d.enabled),
                objects: num("objects", 0.0, 4.0, d.objects),
                camera: num("camera", 0.0, 4.0, d.camera),
                shutter: num("shutter", 0.0, 4.0, d.shutter),
                exposure_ms: num("exposure_ms", 0.0, 1000.0, d.exposure_ms),
                max_blur: num("max_blur", 1.0, 8192.0, d.max_blur),
                samples: num("samples", 4.0, 32.0, d.samples as f32) as u32,
                blur_self: flag("blur_self", d.blur_self),
                debug: flag("debug", d.debug),
            }
        }
        /// Each setting's key and value, as `read` takes them back.
        pub fn stored(&self) -> Vec<(&'static str, String)> {
            vec![
                ("enabled", self.enabled.to_string()), ("objects", self.objects
                .to_string()), ("camera", self.camera.to_string()), ("shutter", self
                .shutter.to_string()), ("exposure_ms", self.exposure_ms.to_string()),
                ("max_blur", self.max_blur.to_string()), ("samples", self.samples
                .to_string()), ("blur_self", self.blur_self.to_string()), ("debug", self
                .debug.to_string()),
            ]
        }
        /// The settings page's buttons.
        pub fn labels(&self) -> Vec<String> {
            let on = |b: bool| if b { "ON" } else { "OFF" };
            let percent = |v: f32| format!("{}%", (v * 100.0).round());
            let exposure = if self.exposure_ms > 0.0 {
                format!("{} ms", self.exposure_ms)
            } else {
                "By shutter".into()
            };
            let longest = if self.max_blur >= 8192.0 {
                "No limit".into()
            } else {
                format!("{} px", self.max_blur)
            };
            vec![
                format!("Motion blur: {}", on(self.enabled)),
                format!("Moving things: {}", percent(self.objects)),
                format!("Camera motion: {}", percent(self.camera)),
                format!("Shutter: {}\u{b0}", (self.shutter * 360.0).round()),
                format!("Exposure: {exposure}"), format!("Longest blur: {longest}"),
                format!("Quality: {} samples", self.samples),
                format!("Blur yourself: {}", on(self.blur_self)),
                format!("Show motion: {}", on(self.debug)),
            ]
        }
        /// A click on button `i` of `labels`: the next choice.
        pub fn click(&mut self, i: usize) {
            match i {
                0 => self.enabled = !self.enabled,
                1 => self.objects = next(&SCALES, self.objects),
                2 => self.camera = next(&SCALES, self.camera),
                3 => self.shutter = next(&SHUTTERS, self.shutter),
                4 => self.exposure_ms = next(&EXPOSURES, self.exposure_ms),
                5 => self.max_blur = next(&MAX_BLURS, self.max_blur),
                6 => self.samples = next(&SAMPLES, self.samples),
                7 => self.blur_self = !self.blur_self,
                8 => self.debug = !self.debug,
                _ => {}
            }
        }
    }
    /// The settings in effect, set by the game (`set_settings`); none until it has read them.
    static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);
    /// Left on in Options > Mods.
    static ON: AtomicBool = AtomicBool::new(true);
    /// The settings in effect.
    pub fn settings() -> Settings {
        SETTINGS.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or_default()
    }
    pub fn set_settings(settings: Settings) {
        *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
    }
    /// Whether the game has handed the settings over yet.
    pub fn loaded() -> bool {
        SETTINGS.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }
    pub fn set_on(on: bool) {
        ON.store(on, Ordering::Relaxed);
    }
    /// Whether the blur draws: on in Options > Mods and in its own settings.
    pub fn active() -> bool {
        ON.load(Ordering::Relaxed) && settings().enabled
    }
    /// One entity vertex for the velocity pass: where it is now and where it was a frame ago, both
    /// relative to that frame's camera.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct MotionVertex {
        cur: [f32; 3],
        prev: [f32; 3],
        uv: [f32; 2],
    }
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct Uniforms {
        inv_view_proj: [[f32; 4]; 4],
        prev_view_proj: [[f32; 4]; 4],
        view_proj: [[f32; 4]; 4],
        jittered: [[f32; 4]; 4],
        /// The camera's motion since the last frame (blocks), w unused.
        delta: [f32; 4],
        /// Camera scale, object scale, shutter, longest blur in pixels.
        params: [f32; 4],
        /// Output width and height, velocity width and height.
        size: [f32; 4],
        /// Samples, frame number, debug, 0.
        misc: [f32; 4],
    }
    /// A run of the entity geometry, keyed to find it again next frame.
    #[derive(Clone, Copy)]
    struct Run {
        key: (u32, i32, u32),
        start: usize,
        len: usize,
        skip: bool,
    }
    struct Prev {
        view_proj: Mat4,
        camera: DVec3,
        runs: Vec<Run>,
        positions: Vec<[f32; 3]>,
    }
    struct Targets {
        vel_size: [u32; 2],
        out_size: [u32; 2],
        color_format: wgpu::TextureFormat,
        scratch_format: wgpu::TextureFormat,
        velocity: wgpu::TextureView,
        tiles: wgpu::TextureView,
        neighbours: wgpu::TextureView,
        scratch: wgpu::Texture,
        scratch_view: wgpu::TextureView,
    }
    struct Gpu {
        color_format: wgpu::TextureFormat,
        uniforms: wgpu::Buffer,
        sampler: wgpu::Sampler,
        camera_layout: wgpu::BindGroupLayout,
        entity_layout: wgpu::BindGroupLayout,
        one_texture_layout: wgpu::BindGroupLayout,
        gather_layout: wgpu::BindGroupLayout,
        camera_pipeline: wgpu::RenderPipeline,
        entity_pipeline: wgpu::RenderPipeline,
        tile_pipeline: wgpu::RenderPipeline,
        neighbour_pipeline: wgpu::RenderPipeline,
        gather_pipeline: wgpu::RenderPipeline,
        vertices: wgpu::Buffer,
        vertex_capacity: u64,
        targets: Option<Targets>,
    }
    struct State {
        settings: Settings,
        last_frame: Option<Instant>,
        frame: u32,
        prev: Option<Prev>,
        gpu: Option<Gpu>,
        warned_copy: bool,
        motion: Vec<MotionVertex>,
    }
    static STATE: Mutex<Option<State>> = Mutex::new(None);
    const VELOCITY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    const TILE: u32 = 16;
    /// Overlays (`Vertex::layer`'s low 16 bits) that don't write depth or aren't a surface:
    /// breaking cracks, the boat's water mask, shadows, translucent particles and items, energy
    /// swirls and world text.
    const SKIPPED_OVERLAYS: [u32; 9] = [11, 12, 13, 14, 15, 32, 33, 34, 35];
    pub fn world_drawn(world: &mut WorldDrawn<'_>) {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let state = guard
            .get_or_insert_with(|| State {
                settings: Settings::default(),
                last_frame: None,
                frame: 0,
                prev: None,
                gpu: None,
                warned_copy: false,
                motion: Vec::new(),
            });
        state.settings = settings();
        let now = Instant::now();
        let dt = state
            .last_frame
            .map_or(1.0 / 60.0, |t| now.duration_since(t).as_secs_f32())
            .max(1e-4);
        state.last_frame = Some(now);
        state.frame = state.frame.wrapping_add(1);
        let runs = runs(world.spans, world.entities, state.settings.blur_self);
        let prev = state.prev.take();
        let copyable = world.color.usage().contains(wgpu::TextureUsages::COPY_SRC);
        if !copyable && !state.warned_copy {
            state.warned_copy = true;
            eprintln!(
                "motion blur: this frame target can't be copied, so there is no blur"
            );
        }
        if active() && copyable && world.entities.len() < (1 << 26) {
            if let Some(prev) = &prev {
                let delta = world.camera - prev.camera;
                if delta.length() < 16.0 {
                    state.draw(world, prev, &runs, delta.as_vec3(), dt);
                }
            }
        }
        state.prev = Some(Prev {
            view_proj: world.view_proj,
            camera: world.camera,
            runs,
            positions: world.entities.iter().map(|v| v.pos).collect(),
        });
    }
    /// The runs of `entities` by span; one run of everything when there are no spans.
    fn runs(
        spans: &[EntitySpan],
        entities: &[crate::gfx::Vertex],
        blur_self: bool,
    ) -> Vec<Run> {
        let mut runs: Vec<Run> = Vec::with_capacity(spans.len() + 1);
        let mut seen: std::collections::HashMap<(u32, i32), u32> = std::collections::HashMap::new();
        let mut push = |stage: u32, id: i32, start: usize, end: usize, skip: bool| {
            if end <= start {
                return;
            }
            let ordinal = seen.entry((stage, id)).or_insert(0);
            runs.push(Run {
                key: (stage, id, *ordinal),
                start,
                len: end - start,
                skip,
            });
            *ordinal += 1;
        };
        if spans.is_empty() {
            push(0, 0, 0, entities.len(), false);
            return runs;
        }
        let first = (spans[0].start as usize).min(entities.len());
        push(u32::MAX, 0, 0, first, false);
        for (i, span) in spans.iter().enumerate() {
            let start = (span.start as usize).min(entities.len());
            let end = spans
                .get(i + 1)
                .map_or(
                    entities.len(),
                    |next| (next.start as usize).min(entities.len()),
                );
            let skip = span.hidden || span.translucent || (span.player && !blur_self);
            push(span.stage, span.id, start, end, skip);
        }
        runs
    }
    impl State {
        fn draw(
            &mut self,
            world: &mut WorldDrawn<'_>,
            prev: &Prev,
            runs: &[Run],
            delta: Vec3,
            dt: f32,
        ) {
            let settings = self.settings;
            self.motion.clear();
            let mut by_key = std::collections::HashMap::with_capacity(prev.runs.len());
            for run in &prev.runs {
                by_key.insert(run.key, (run.start, run.len));
            }
            for run in runs {
                if run.skip || run.len % 4 != 0 || settings.objects == 0.0 {
                    continue;
                }
                let Some(&(prev_start, prev_len)) = by_key.get(&run.key) else {
                    continue;
                };
                if prev_len != run.len || prev_start + prev_len > prev.positions.len() {
                    continue;
                }
                let cur = &world.entities[run.start..run.start + run.len];
                let old = &prev.positions[prev_start..prev_start + run.len];
                let moved = |i: usize| {
                    (Vec3::from(cur[i].pos) - Vec3::from(old[i]) + delta)
                        .length_squared()
                };
                if moved(0) > 16.0 || moved(run.len - 1) > 16.0 {
                    continue;
                }
                for quad in 0..run.len / 4 {
                    let q = quad * 4;
                    if SKIPPED_OVERLAYS.contains(&(cur[q].layer & 0xffff)) {
                        continue;
                    }
                    for i in q..q + 4 {
                        self.motion
                            .push(MotionVertex {
                                cur: cur[i].pos,
                                prev: old[i],
                                uv: cur[i].uv,
                            });
                    }
                }
            }
            if settings.debug && self.frame % 60 == 0 {
                eprintln!(
                    "motion blur: {} runs ({} last frame), {} of {} entity vertices matched, camera moved {:?}",
                    runs.len(), prev.runs.len(), self.motion.len(), world.entities.len(),
                    delta
                );
            }
            let shutter = if settings.exposure_ms > 0.0 {
                (settings.exposure_ms / 1000.0 / dt).min(8.0)
            } else {
                settings.shutter
            };
            let out_size = world.size;
            let vel_size = [world.depth_size[0].max(1), world.depth_size[1].max(1)];
            let uniforms = Uniforms {
                inv_view_proj: world.view_proj.inverse().to_cols_array_2d(),
                prev_view_proj: prev.view_proj.to_cols_array_2d(),
                view_proj: world.view_proj.to_cols_array_2d(),
                jittered: world.jittered.to_cols_array_2d(),
                delta: [delta.x, delta.y, delta.z, 0.0],
                params: [settings.camera, settings.objects, shutter, settings.max_blur],
                size: [
                    out_size[0] as f32,
                    out_size[1] as f32,
                    vel_size[0] as f32,
                    vel_size[1] as f32,
                ],
                misc: [
                    settings.samples as f32,
                    (self.frame % 1024) as f32,
                    if settings.debug { 1.0 } else { 0.0 },
                    0.0,
                ],
            };
            let gpu = match &mut self.gpu {
                Some(gpu) if gpu.color_format == world.color_format => gpu,
                gpu => gpu.insert(Gpu::new(world.device, world.color_format)),
            };
            world.queue.write_buffer(&gpu.uniforms, 0, bytemuck::bytes_of(&uniforms));
            let bytes = (self.motion.len() * std::mem::size_of::<MotionVertex>()) as u64;
            if bytes > gpu.vertex_capacity {
                gpu.vertex_capacity = bytes.next_power_of_two();
                gpu.vertices = world
                    .device
                    .create_buffer(
                        &wgpu::BufferDescriptor {
                            label: Some("Motion blur vertices"),
                            size: gpu.vertex_capacity,
                            usage: wgpu::BufferUsages::VERTEX
                                | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        },
                    );
            }
            if bytes > 0 {
                world
                    .queue
                    .write_buffer(&gpu.vertices, 0, bytemuck::cast_slice(&self.motion));
            }
            gpu.draw(world, vel_size, out_size, (self.motion.len() / 4) as u32);
        }
    }
    impl Gpu {
        fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
            let shader = device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Motion blur"),
                    source: wgpu::ShaderSource::Wgsl(SHADER.into()),
                });
            let uniforms = device
                .create_buffer(
                    &wgpu::BufferDescriptor {
                        label: Some("Motion blur uniforms"),
                        size: std::mem::size_of::<Uniforms>() as u64,
                        usage: wgpu::BufferUsages::UNIFORM
                            | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                );
            let sampler = device
                .create_sampler(
                    &wgpu::SamplerDescriptor {
                        label: Some("Motion blur atlas"),
                        ..Default::default()
                    },
                );
            let uniform_entry = |binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            };
            let texture_entry = |binding, sample_type| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            };
            let unfilterable = wgpu::TextureSampleType::Float {
                filterable: false,
            };
            let layout = |label, entries: &[wgpu::BindGroupLayoutEntry]| {
                device
                    .create_bind_group_layout(
                        &wgpu::BindGroupLayoutDescriptor {
                            label: Some(label),
                            entries,
                        },
                    )
            };
            let camera_layout = layout(
                "Motion blur camera",
                &[uniform_entry(0), texture_entry(1, wgpu::TextureSampleType::Depth)],
            );
            let entity_layout = layout(
                "Motion blur entities",
                &[
                    uniform_entry(0),
                    texture_entry(
                        1,
                        wgpu::TextureSampleType::Float {
                            filterable: true,
                        },
                    ),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
            );
            let one_texture_layout = layout(
                "Motion blur tiles",
                &[texture_entry(0, unfilterable)],
            );
            let gather_layout = layout(
                "Motion blur gather",
                &[
                    uniform_entry(0),
                    texture_entry(1, unfilterable),
                    texture_entry(2, unfilterable),
                    texture_entry(3, unfilterable),
                ],
            );
            let pipeline = |
                label,
                group: &wgpu::BindGroupLayout,
                vs: &str,
                fs: &str,
                format: wgpu::TextureFormat,
                buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                depth: Option<wgpu::DepthStencilState>|
            {
                let pipeline_layout = device
                    .create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some(label),
                            bind_group_layouts: &[Some(group)],
                            ..Default::default()
                        },
                    );
                device
                    .create_render_pipeline(
                        &wgpu::RenderPipelineDescriptor {
                            label: Some(label),
                            layout: Some(&pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: Some(vs),
                                compilation_options: Default::default(),
                                buffers,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: Some(fs),
                                compilation_options: Default::default(),
                                targets: &[
                                    Some(wgpu::ColorTargetState {
                                        format,
                                        blend: None,
                                        write_mask: wgpu::ColorWrites::ALL,
                                    }),
                                ],
                            }),
                            primitive: wgpu::PrimitiveState::default(),
                            depth_stencil: depth,
                            multisample: Default::default(),
                            multiview_mask: None,
                            cache: None,
                        },
                    )
            };
            let camera_pipeline = pipeline(
                "Motion blur camera",
                &camera_layout,
                "vs_full",
                "fs_camera",
                VELOCITY_FORMAT,
                &[],
                None,
            );
            let entity_pipeline = pipeline(
                "Motion blur entities",
                &entity_layout,
                "vs_entity",
                "fs_entity",
                VELOCITY_FORMAT,
                &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MotionVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x3, 2 => Float32x2
                        ],
                    }),
                ],
                Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 8,
                        slope_scale: 1.0,
                        clamp: 0.0,
                    },
                }),
            );
            let tile_pipeline = pipeline(
                "Motion blur tile max",
                &one_texture_layout,
                "vs_full",
                "fs_tile_max",
                VELOCITY_FORMAT,
                &[],
                None,
            );
            let neighbour_pipeline = pipeline(
                "Motion blur neighbour max",
                &one_texture_layout,
                "vs_full",
                "fs_neighbour_max",
                VELOCITY_FORMAT,
                &[],
                None,
            );
            let gather_pipeline = pipeline(
                "Motion blur gather",
                &gather_layout,
                "vs_full",
                "fs_gather",
                color_format,
                &[],
                None,
            );
            let vertices = device
                .create_buffer(
                    &wgpu::BufferDescriptor {
                        label: Some("Motion blur vertices"),
                        size: 1 << 16,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    },
                );
            Self {
                color_format,
                uniforms,
                sampler,
                camera_layout,
                entity_layout,
                one_texture_layout,
                gather_layout,
                camera_pipeline,
                entity_pipeline,
                tile_pipeline,
                neighbour_pipeline,
                gather_pipeline,
                vertices,
                vertex_capacity: 1 << 16,
                targets: None,
            }
        }
        fn targets(
            &mut self,
            device: &wgpu::Device,
            vel_size: [u32; 2],
            out_size: [u32; 2],
            scratch_format: wgpu::TextureFormat,
        ) -> &Targets {
            let fits = self
                .targets
                .as_ref()
                .is_some_and(|t| {
                    t.vel_size == vel_size && t.out_size == out_size
                        && t.color_format == self.color_format
                        && t.scratch_format == scratch_format
                });
            if !fits {
                let texture = |
                    label,
                    size: [u32; 2],
                    format,
                    usage,
                    view_formats: &[wgpu::TextureFormat]|
                {
                    device
                        .create_texture(
                            &wgpu::TextureDescriptor {
                                label: Some(label),
                                size: wgpu::Extent3d {
                                    width: size[0].max(1),
                                    height: size[1].max(1),
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: 1,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format,
                                usage,
                                view_formats,
                            },
                        )
                };
                let target = wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING;
                let tile_size = [vel_size[0].div_ceil(TILE), vel_size[1].div_ceil(TILE)];
                let view = |t: wgpu::Texture| t.create_view(&Default::default());
                let views = [self.color_format];
                let scratch = texture(
                    "Motion blur frame copy",
                    out_size,
                    scratch_format,
                    wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    if scratch_format == self.color_format { &[] } else { &views },
                );
                let scratch_view = scratch
                    .create_view(
                        &wgpu::TextureViewDescriptor {
                            format: Some(self.color_format),
                            ..Default::default()
                        },
                    );
                self.targets = Some(Targets {
                    vel_size,
                    out_size,
                    color_format: self.color_format,
                    scratch_format,
                    velocity: view(
                        texture(
                            "Motion blur velocity",
                            vel_size,
                            VELOCITY_FORMAT,
                            target,
                            &[],
                        ),
                    ),
                    tiles: view(
                        texture(
                            "Motion blur tiles",
                            tile_size,
                            VELOCITY_FORMAT,
                            target,
                            &[],
                        ),
                    ),
                    neighbours: view(
                        texture(
                            "Motion blur neighbours",
                            tile_size,
                            VELOCITY_FORMAT,
                            target,
                            &[],
                        ),
                    ),
                    scratch,
                    scratch_view,
                });
            }
            self.targets.as_ref().unwrap()
        }
        fn draw(
            &mut self,
            world: &mut WorldDrawn<'_>,
            vel_size: [u32; 2],
            out_size: [u32; 2],
            quads: u32,
        ) {
            let device = world.device;
            let scratch_format = world.color.format();
            self.targets(device, vel_size, out_size, scratch_format);
            let t = self.targets.as_ref().unwrap();
            let encoder = &mut *world.encoder;
            let group = |
                layout: &wgpu::BindGroupLayout,
                entries: &[wgpu::BindingResource<'_>]|
            {
                let entries: Vec<_> = entries
                    .iter()
                    .enumerate()
                    .map(|(i, resource)| wgpu::BindGroupEntry {
                        binding: i as u32,
                        resource: resource.clone(),
                    })
                    .collect();
                device
                    .create_bind_group(
                        &wgpu::BindGroupDescriptor {
                            label: Some("Motion blur"),
                            layout,
                            entries: &entries,
                        },
                    )
            };
            let uniforms = self.uniforms.as_entire_binding();
            let full_pass = |
                encoder: &mut wgpu::CommandEncoder,
                label,
                target: &wgpu::TextureView,
                pipeline: &wgpu::RenderPipeline,
                group: &wgpu::BindGroup|
            {
                let mut pass = encoder
                    .begin_render_pass(
                        &wgpu::RenderPassDescriptor {
                            label: Some(label),
                            color_attachments: &[
                                Some(wgpu::RenderPassColorAttachment {
                                    view: target,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                        store: wgpu::StoreOp::Store,
                                    },
                                }),
                            ],
                            ..Default::default()
                        },
                    );
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..3, 0..1);
            };
            let camera_group = group(
                &self.camera_layout,
                &[uniforms.clone(), wgpu::BindingResource::TextureView(world.depth)],
            );
            full_pass(
                encoder,
                "Motion blur camera velocity",
                &t.velocity,
                &self.camera_pipeline,
                &camera_group,
            );
            if quads > 0 && let Some(atlas) = world.atlas {
                let atlas_view = atlas.create_view(&Default::default());
                let entity_group = group(
                    &self.entity_layout,
                    &[
                        uniforms.clone(),
                        wgpu::BindingResource::TextureView(&atlas_view),
                        wgpu::BindingResource::Sampler(&self.sampler),
                    ],
                );
                let mut pass = encoder
                    .begin_render_pass(
                        &wgpu::RenderPassDescriptor {
                            label: Some("Motion blur entity velocity"),
                            color_attachments: &[
                                Some(wgpu::RenderPassColorAttachment {
                                    view: &t.velocity,
                                    resolve_target: None,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                }),
                            ],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: world.depth,
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }),
                            ..Default::default()
                        },
                    );
                pass.set_pipeline(&self.entity_pipeline);
                pass.set_bind_group(0, &entity_group, &[]);
                pass.set_vertex_buffer(0, self.vertices.slice(..));
                pass.set_index_buffer(
                    world.quad_indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..quads * 6, 0, 0..1);
            }
            let tile_group = group(
                &self.one_texture_layout,
                &[wgpu::BindingResource::TextureView(&t.velocity)],
            );
            full_pass(
                encoder,
                "Motion blur tile max",
                &t.tiles,
                &self.tile_pipeline,
                &tile_group,
            );
            let neighbour_group = group(
                &self.one_texture_layout,
                &[wgpu::BindingResource::TextureView(&t.tiles)],
            );
            full_pass(
                encoder,
                "Motion blur neighbour max",
                &t.neighbours,
                &self.neighbour_pipeline,
                &neighbour_group,
            );
            encoder
                .copy_texture_to_texture(
                    world.color.as_image_copy(),
                    t.scratch.as_image_copy(),
                    wgpu::Extent3d {
                        width: out_size[0],
                        height: out_size[1],
                        depth_or_array_layers: 1,
                    },
                );
            let gather_group = group(
                &self.gather_layout,
                &[
                    uniforms,
                    wgpu::BindingResource::TextureView(&t.scratch_view),
                    wgpu::BindingResource::TextureView(&t.velocity),
                    wgpu::BindingResource::TextureView(&t.neighbours),
                ],
            );
            full_pass(
                encoder,
                "Motion blur gather",
                world.color_view,
                &self.gather_pipeline,
                &gather_group,
            );
        }
    }
    const SHADER: &str = r#"
struct U {
    inv_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    jittered: mat4x4<f32>,
    delta: vec4<f32>,
    params: vec4<f32>,
    size: vec4<f32>,
    misc: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: U;

fn ndc_to_uv(ndc: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
}

// Screen motion (uv units) of what is at camera-relative homogeneous `rel` now, had it held
// still in the world.
fn camera_motion(rel: vec4<f32>, uv: vec2<f32>) -> vec2<f32> {
    let prev = u.prev_view_proj * (rel + vec4<f32>(u.delta.xyz * rel.w, 0.0));
    if prev.w <= 1e-6 {
        return vec2<f32>(0.0);
    }
    return uv - ndc_to_uv(prev.xy / prev.w);
}

struct Full {
    @builtin(position) pos: vec4<f32>,
}

@vertex fn vs_full(@builtin(vertex_index) i: u32) -> Full {
    let xy = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: Full;
    out.pos = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

// --- velocity: rg = screen motion over the shutter (uv), b = distance (blocks) ---

@group(0) @binding(1) var depth_tex: texture_depth_2d;

@fragment fn fs_camera(in: Full) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.pos.xy);
    let depth = textureLoad(depth_tex, pixel, 0);
    let uv = in.pos.xy / u.size.zw;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let rel = u.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    let motion = camera_motion(rel, uv) * u.params.x * u.params.z;
    var distance = 1e4;
    if depth > 0.0 && abs(rel.w) > 1e-9 {
        distance = length(rel.xyz / rel.w);
    }
    return vec4<f32>(motion, distance, 1.0);
}

@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct EntityOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) cur: vec4<f32>,
    @location(1) prev: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) rel: vec3<f32>,
}

@vertex fn vs_entity(@location(0) cur: vec3<f32>, @location(1) prev: vec3<f32>,
    @location(2) uv: vec2<f32>) -> EntityOut {
    var out: EntityOut;
    out.pos = u.jittered * vec4<f32>(cur, 1.0);
    out.cur = u.view_proj * vec4<f32>(cur, 1.0);
    out.prev = u.prev_view_proj * vec4<f32>(prev, 1.0);
    out.uv = uv;
    out.rel = cur;
    return out;
}

@fragment fn fs_entity(in: EntityOut) -> @location(0) vec4<f32> {
    if textureSample(atlas, atlas_sampler, in.uv).a < 0.1 {
        discard;
    }
    if in.prev.w <= 1e-6 {
        discard;
    }
    let uv = ndc_to_uv(in.cur.xy / in.cur.w);
    let full = uv - ndc_to_uv(in.prev.xy / in.prev.w);
    // What the camera's motion alone would do to it, and what its own motion adds.
    let still = u.prev_view_proj * vec4<f32>(in.rel + u.delta.xyz, 1.0);
    var camera = full;
    if still.w > 1e-6 {
        camera = uv - ndc_to_uv(still.xy / still.w);
    }
    let motion = (camera * u.params.x + (full - camera) * u.params.y) * u.params.z;
    return vec4<f32>(motion, length(in.rel), 1.0);
}

// --- tile max and neighbour max ---

@group(0) @binding(0) var source: texture_2d<f32>;

fn longer(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    if dot(b.xy, b.xy) > dot(a.xy, a.xy) {
        return b;
    }
    return a;
}

@fragment fn fs_tile_max(in: Full) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source));
    let base = vec2<i32>(in.pos.xy) * 16;
    var best = vec4<f32>(0.0);
    for (var y = 0; y < 16; y++) {
        for (var x = 0; x < 16; x++) {
            let p = base + vec2<i32>(x, y);
            if p.x < size.x && p.y < size.y {
                best = longer(best, textureLoad(source, p, 0));
            }
        }
    }
    return best;
}

@fragment fn fs_neighbour_max(in: Full) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(source));
    let base = vec2<i32>(in.pos.xy);
    var best = vec4<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let p = clamp(base + vec2<i32>(x, y), vec2<i32>(0), size - 1);
            best = longer(best, textureLoad(source, p, 0));
        }
    }
    return best;
}

// --- gather ---

@group(0) @binding(1) var frame: texture_2d<f32>;
@group(0) @binding(2) var velocity: texture_2d<f32>;
@group(0) @binding(3) var neighbours: texture_2d<f32>;

fn limit(v: vec2<f32>) -> vec2<f32> {
    let px = v * u.size.xy;
    let len = length(px);
    if len > u.params.w {
        return px * (u.params.w / len);
    }
    return px;
}

// The velocity texel under output pixel `p`: (motion in pixels, distance).
fn motion_at(p: vec2<f32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(velocity));
    let q = clamp(vec2<i32>(p / u.size.xy * u.size.zw), vec2<i32>(0), size - 1);
    let v = textureLoad(velocity, q, 0);
    return vec3<f32>(limit(v.xy), v.z);
}

fn color_at(p: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(frame));
    return textureLoad(frame, clamp(vec2<i32>(p), vec2<i32>(0), size - 1), 0);
}

fn cone(distance: f32, len: f32) -> f32 {
    return clamp(1.0 - distance / max(len * 0.5, 0.5), 0.0, 1.0);
}

fn cylinder(distance: f32, len: f32) -> f32 {
    let half = max(len * 0.5, 0.5);
    return 1.0 - smoothstep(0.95 * half, 1.05 * half, distance);
}

// 1 where `a` is nearer than or as near as `b`.
fn nearer(a: f32, b: f32) -> f32 {
    let soft = max(min(a, b) * 0.03, 0.05);
    return clamp(1.0 - (a - b) / soft, 0.0, 1.0);
}

fn noise(p: vec2<f32>) -> f32 {
    let q = p + vec2<f32>(u.misc.y * 7.0, u.misc.y * 13.0);
    return fract(52.9829189 * fract(dot(q, vec2<f32>(0.06711056, 0.00583715))));
}

@fragment fn fs_gather(in: Full) -> @location(0) vec4<f32> {
    let x = in.pos.xy;
    let center = color_at(x);
    if u.misc.z > 0.5 {
        let m = motion_at(x).xy / 32.0;
        return vec4<f32>(abs(m.x), abs(m.y), center.g * 0.3, 1.0);
    }
    let tiles = vec2<i32>(textureDimensions(neighbours));
    let tile = clamp(vec2<i32>(x / u.size.xy * u.size.zw) / 16, vec2<i32>(0), tiles - 1);
    let vn = limit(textureLoad(neighbours, tile, 0).xy);
    let vn_len = length(vn);
    if vn_len < 0.5 {
        return center;
    }
    let c = motion_at(x);
    let vc_len = length(c.xy);
    let samples = i32(u.misc.x);
    let jitter = noise(x) - 0.5;
    var weight = 1.0 / max(vc_len * 0.5, 1.0);
    var sum = center.rgb * weight;
    for (var i = 0; i < samples; i++) {
        let t = mix(-1.0, 1.0, (f32(i) + jitter + 1.0) / f32(samples + 1));
        let y = floor(x + vn * 0.5 * t) + 0.5;
        let distance = length(y - x);
        let s = motion_at(y);
        let vy_len = length(s.xy);
        let front = nearer(s.z, c.z);
        let back = nearer(c.z, s.z);
        let w = front * cone(distance, vy_len)
            + back * cone(distance, vc_len)
            + cylinder(distance, vy_len) * cylinder(distance, vc_len) * 2.0;
        weight += w;
        sum += color_at(y).rgb * w;
    }
    return vec4<f32>(sum / weight, center.a);
}
"#;
}
