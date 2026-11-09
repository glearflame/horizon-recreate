//! Mob rendering (`LivingEntityRenderer.submit` and the render layers):
//! variant/baby model and texture selection, setupRotations, per-species
//! scale, and extra layers (sheep wool, spider eyes, drowned/stray outer
//! layers, villager overlays, collars...).
use super::{
    Mob, MobData, ModelNormals, Part, Species, anim::{self, Poses},
    living::wrap,
};
use crate::mesher::Vertex;
use horizon_core::geometry::trig::Mth;
use std::borrow::Cow;
use std::f32::consts::PI;
/// A texture lookup into the shared atlas (name without namespace -> uv rect).
pub type Atlas<'a> = &'a dyn Fn(&str) -> Option<[f32; 4]>;
/// The overlay code of an enchanted armour layer's glint, past the ones
/// `gfx/entity_batch.rs` gives materials (13 is the boat water mask, which
/// draws depth only: armour with 13 vanished; 35 is text pulled toward the
/// camera). It marks the `ARMOR_ENTITY_GLINT_TEXTURING` sample: entity.wgsl
/// transforms the model UV and reads `enchanted_glint_armor`.
pub const ARMOR_GLINT: u32 = 36;
/// `RenderTypes.breezeWind`: the texture of the breeze's wind layer scrolls
/// by 0.02 of the texture per tick.
pub const BREEZE_WIND_SCROLL: f32 = 0.02;
/// `WindChargeRenderer.xOffset`: the wind charge's texture scrolls by 0.03 of
/// the texture per tick.
pub const WIND_CHARGE_SCROLL: f32 = 0.03;
/// The offset of a scrolling texture: `RenderTypes.breezeWind` takes the
/// texture's age times the speed, modulo one.
pub fn scroll_offset(age: f32, speed: f32) -> f32 {
    (age * speed) % 1.0
}
/// The vertex's U with the scroll: the texture matrix of
/// `RenderTypes.breezeWind` moves the texture under the model, so the U
/// moves by the offset and wraps around the texture.
fn scroll_u(u: f32, scroll: f32) -> f32 {
    if scroll == 0.0 { u } else { (u + scroll) % 1.0 }
}
/// Full sky and block light in a vertex's `light`.
pub const FULL_BRIGHT: u32 = 240 | 240 << 8;
/// How brightly the eyes render type's layers (spiders', endermen's and
/// phantoms' eyes, the breeze's, the creaking's, the copper golem's) shine in
/// HDR output, as `mesher::hdr_tier`.
const EYES_TIER: u8 = crate::mesher::BRIGHT;
/// Tags quads as drawn by a render type vanilla gives an `OitPipelineSet`
/// (`gfx::OIT_ENTITY`): improved transparency draws them in its stages.
fn mark_oit(quads: &mut [Vertex]) {
    for vertex in quads {
        vertex.layer |= crate::gfx::OIT_ENTITY;
    }
}
/// Whether the species' model is built with `RenderTypes::entityTranslucent`
/// (`Model.renderType`, which `LivingEntityRenderer.getRenderType` uses for the
/// body): the allay, the piglins, the breeze, the vex, the player model and the
/// sulfur cube.
fn translucent_body(m: &Mob) -> bool {
    matches!(
        m.species(), Species::Allay | Species::Piglin | Species::PiglinBrute |
        Species::ZombifiedPiglin | Species::Breeze | Species::Vex | Species::SulfurCube
    ) || avatar(m)
}
/// Whether a layer's render type has an `OitPipelineSet`: `RenderTypes.eyes` and
/// `entityTranslucentEmissive` (the full bright layers), `breezeWind`, and
/// `entityTranslucent` (the slime's outer cube, the sulfur cube's inner cube
/// and the horse's markings).
fn translucent_layer(m: &Mob, layer: &Layer) -> bool {
    layer.full_bright || layer.model == "breeze#wind" || layer.model == "slime#outer"
        || matches!(m.species(), Species::SulfurCube)
        || layer.texture.starts_with("entity/horse/horse_markings")
}
/// Whether a layer is the eyes render type's (`RenderTypes.eyes`: full bright, no
/// normal, at `EYES_TIER`) on a mob whose other draws are plain: spiders',
/// endermen's, phantoms', the creaking's and the copper golem's. The GPU path
/// poses them as [`super::skin::Skin`] `FLAT` draws when improved transparency
/// is off ([`super::skin::SkinModels::tagged`]).
fn eyes_layer(m: &Mob, layer: &Layer) -> bool {
    layer.full_bright && layer.scroll == 0.0
        && matches!(
            m.species(), Species::Spider | Species::CaveSpider | Species::Enderman |
            Species::Phantom | Species::Creaking | Species::CopperGolem
        )
}
/// One draw of a model with a texture.
pub struct Layer {
    pub model: Cow<'static, str>,
    pub texture: Cow<'static, str>,
    pub color: u32,
    pub full_bright: bool,
    /// `RenderTypes.breezeWind`: the texture scrolls with the entity's age.
    pub scroll: f32,
}
/// DyeColor.getTextureDiffuseColor (RGB).
pub const DYE: [u32; 16] = [
    0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52,
    0x9D9D97, 0x169C9C, 0x8932B8, 0x3C44AA, 0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
];
pub const DYE_NAMES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];
/// ABGR vertex colour from an RGB value.
pub fn abgr(rgb: u32) -> u32 {
    let (r, g, b) = ((rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255);
    0xFF00_0000 | b << 16 | g << 8 | r
}
/// `EquipmentLayerRenderer.getColorForLayer`: a dyeable layer takes the
/// stack's dye colour (`minecraft:dyed_color`), the colour it has when
/// undyed otherwise; a layer that is not dyeable is not tinted.
pub fn armor_tint(undyed: Option<u32>, stack: Option<&crate::items::ItemStack>) -> u32 {
    let Some(undyed) = undyed else {
        return 0xFFFF_FFFF;
    };
    let dyed = stack
        .and_then(|s| s.extra.as_ref())
        .and_then(|e| e.dyed_color)
        .map(|d| d as u32)
        .unwrap_or(undyed);
    abgr(dyed)
}
/// ColorLerper.Type.SHEEP colour: white is 0xE6E6E6, others 75% of the dye.
pub fn sheep_wool_rgb(color: u8) -> u32 {
    if color == 0 {
        return 0xE6E6E6;
    }
    let src = DYE[color as usize & 15];
    let f = |c: u32| ((c as f32 * 0.75).floor() as u32).min(255);
    f(src >> 16 & 255) << 16 | f(src >> 8 & 255) << 8 | f(src & 255)
}
/// `ColorLerper.getLerpedColor(ColorLerper.Type.SHEEP, ageInTicks)`: the 16
/// wool colours in DyeColor order, one per 25 ticks, lerped in between. The
/// colour a `jeb_` sheep's wool takes.
pub fn jeb_wool_rgb(age: f32) -> u32 {
    let i = age.floor();
    let ticks = i as u64;
    let j = ticks / 25;
    let f = ((ticks % 25) as f32 + (age - i)) / 25.0;
    let lo = sheep_wool_rgb((j % 16) as u8);
    let hi = sheep_wool_rgb(((j + 1) % 16) as u8);
    let mix = |shift: u32| {
        let (lo, hi) = ((lo >> shift & 255) as f32, (hi >> shift & 255) as f32);
        ((lo + (hi - lo) * f) as u32) << shift
    };
    mix(16) | mix(8) | mix(0)
}
pub const VILLAGER_TYPES: [&str; 7] = [
    "plains",
    "desert",
    "jungle",
    "savanna",
    "snow",
    "swamp",
    "taiga",
];
pub const PROFESSIONS: [&str; 15] = [
    "none",
    "armorer",
    "butcher",
    "cartographer",
    "cleric",
    "farmer",
    "fisherman",
    "fletcher",
    "leatherworker",
    "librarian",
    "mason",
    "nitwit",
    "shepherd",
    "toolsmith",
    "weaponsmith",
];
fn baby_or<'a>(
    m: &Mob,
    adult: impl Into<Cow<'a, str>>,
    baby: impl Into<Cow<'a, str>>,
) -> Cow<'a, str> {
    let baby = baby.into();
    if m.is_baby() && m.kind.layer(&baby).is_some() { baby } else { adult.into() }
}
/// A name built from parts (`entity/villager/profession/farmer`), kept for the
/// process so that drawing a mob allocates no names each frame: they come from
/// a few tables (types, professions, levels, materials, variants). Past 4096
/// names on a thread (an unknown item's material from a server) a new one comes
/// back owned.
fn interned(args: std::fmt::Arguments) -> Cow<'static, str> {
    use std::fmt::Write;
    thread_local! {
        static NAMES : std::cell::RefCell < (String, crate ::fast_hash::FxHashSet
        <&'static str >) > = std::cell::RefCell::default();
    }
    NAMES
        .with(|names| {
            let (buffer, set) = &mut *names.borrow_mut();
            buffer.clear();
            let _ = buffer.write_fmt(args);
            if let Some(name) = set.get(buffer.as_str()) {
                return Cow::Borrowed(*name);
            }
            if set.len() >= 4096 {
                return Cow::Owned(buffer.clone());
            }
            let name: &'static str = Box::leak(buffer.as_str().into());
            set.insert(name);
            Cow::Borrowed(name)
        })
}
fn climate(m: &Mob) -> &'static str {
    match m.extra.variant {
        1 => "warm",
        2 => "cold",
        _ => "temperate",
    }
}
/// `AvatarRenderer` entities: a mannequin draws exactly like a player —
/// the player model, its resolved skin and the same scale.
fn avatar(m: &Mob) -> bool {
    matches!(&* m.kind.id, "minecraft:player" | "minecraft:mannequin")
}
/// `AvatarRenderer.shouldRenderLayers`: a spectating player or mannequin
/// draws no layers at all (armour, held items, head item, cape, wings,
/// parrots, ears, stuck arrows), only its translucent head.
pub fn layers_shown(m: &Mob) -> bool {
    !(avatar(m) && m.extra.spectator)
}
/// The model layer key and texture for the mob's body.
pub fn body(m: &Mob) -> (Cow<'static, str>, Cow<'_, str>) {
    use Species as S;
    let baby = m.is_baby();
    let b = |s: &str| -> Cow<'static, str> {
        if baby {
            interned(format_args!("{s}_baby"))
        } else {
            interned(format_args!("{s}"))
        }
    };
    if avatar(m) {
        return (
            if m.slim { "player_slim#main" } else { "main" }.into(),
            m.skin.as_deref().unwrap_or(&m.kind.texture).into(),
        );
    }
    match m.species() {
        S::Llama | S::TraderLlama => {
            let name = if m.species() == S::Llama { "llama" } else { "trader_llama" };
            (
                baby_or(
                    m,
                    interned(format_args!("{name}#main")),
                    interned(format_args!("{name}_baby#main")),
                ),
                super::llama::texture(m).into(),
            )
        }
        s if super::equine::is_equine(s) => {
            let name = super::equine::model_name(s);
            (
                baby_or(
                    m,
                    interned(format_args!("{name}#main")),
                    interned(format_args!("{name}_baby#main")),
                ),
                super::equine::texture(m).into(),
            )
        }
        S::Wolf => {
            (baby_or(m, "wolf#main", "wolf_baby#main"), super::tame::texture(m).into())
        }
        S::Panda => {
            (
                baby_or(m, "panda#main", "panda_baby#main"),
                super::panda::texture(m).into(),
            )
        }
        S::HappyGhast => {
            (
                baby_or(m, "happy_ghast#main", "happy_ghast_baby#main"),
                if baby {
                    "entity/ghast/happy_ghast_baby".into()
                } else {
                    "entity/ghast/happy_ghast".into()
                },
            )
        }
        S::Nautilus | S::ZombieNautilus => {
            (super::nautilus::model(m).into(), super::nautilus::texture(m).into())
        }
        S::SulfurCube => {
            let (model, texture) = super::sulfur_cube::body(m);
            (model.into(), texture.into())
        }
        S::Sniffer => {
            (
                baby_or(m, "sniffer#main", "sniffer_baby#main"),
                if baby {
                    "entity/sniffer/snifflet".into()
                } else {
                    "entity/sniffer/sniffer".into()
                },
            )
        }
        S::Shulker => ("shulker#main".into(), super::shulker::texture(m).into()),
        S::Vex => ("vex#main".into(), super::vex::texture(m).into()),
        S::Wither => ("wither#main".into(), super::wither::texture(m).into()),
        S::Ghast => {
            (
                "ghast#main".into(),
                if m.extra.flag {
                    "entity/ghast/ghast_shooting".into()
                } else {
                    "entity/ghast/ghast".into()
                },
            )
        }
        S::Cat => {
            (baby_or(m, "cat#main", "cat_baby#main"), super::tame::cat_texture(m).into())
        }
        S::Frog => {
            (
                "frog#main".into(),
                interned(format_args!("entity/frog/frog_{}", climate(m))),
            )
        }
        S::Rabbit => {
            let name = if m.custom_name.as_deref() == Some("Toast") {
                "toast"
            } else {
                match m.extra.variant {
                    1 => "white",
                    2 => "black",
                    3 => "white_splotched",
                    4 => "gold",
                    5 => "salt",
                    99 => "caerbannog",
                    _ => "brown",
                }
            };
            (
                baby_or(m, "rabbit#main", "rabbit_baby#main"),
                b(&interned(format_args!("entity/rabbit/rabbit_{name}"))),
            )
        }
        S::Parrot => {
            let name = [
                "red_blue",
                "blue",
                "green",
                "yellow_blue",
                "grey",
            ][usize::from(m.extra.variant).min(4)];
            (
                m.kind.model_name().into(),
                interned(format_args!("entity/parrot/parrot_{name}")),
            )
        }
        S::Axolotl => {
            let name = [
                "lucy",
                "wild",
                "gold",
                "cyan",
                "blue",
            ][usize::from(m.extra.variant).min(4)];
            (
                baby_or(m, "axolotl#main", "axolotl_baby#main"),
                b(&interned(format_args!("entity/axolotl/axolotl_{name}"))),
            )
        }
        S::Salmon => {
            let model = match m.extra.variant {
                0 => "salmon_small#main",
                2 => "salmon_large#main",
                _ => "salmon#main",
            };
            (model.into(), "entity/fish/salmon".into())
        }
        S::Goat => (baby_or(m, "goat#main", "goat_baby#main"), b(&m.kind.texture)),
        S::PolarBear => {
            (baby_or(m, "polar_bear#main", "polar_bear_baby#main"), b(&m.kind.texture))
        }
        S::Ocelot => (baby_or(m, "ocelot#main", "ocelot_baby#main"), b(&m.kind.texture)),
        S::Turtle => (baby_or(m, "turtle#main", "turtle_baby#main"), b(&m.kind.texture)),
        S::Armadillo => {
            (baby_or(m, "armadillo#main", "armadillo_baby#main"), b(&m.kind.texture))
        }
        S::Fox => {
            (
                baby_or(m, "fox#main", "fox_baby#main"),
                interned(
                    format_args!(
                        "entity/fox/fox{}{}{}", if m.extra.variant == 0 { "" } else {
                        "_snow" }, if super::fox::flag(m, super::fox::SLEEPING) {
                        "_sleep" } else { "" }, if baby { "_baby" } else { "" },
                    ),
                ),
            )
        }
        S::Pig => {
            let model = if baby {
                "pig_baby#main".into()
            } else if m.extra.variant == 2 {
                "cold_pig#main".into()
            } else {
                "pig#main".into()
            };
            (model, b(&interned(format_args!("entity/pig/pig_{}", climate(m)))))
        }
        S::Cow => {
            let model = match (m.extra.variant, baby) {
                (1, false) => "warm_cow#main",
                (1, true) => "warm_cow_baby#main",
                (2, false) => "cold_cow#main",
                (2, true) => "cold_cow_baby#main",
                (_, true) => "cow_baby#main",
                _ => "cow#main",
            };
            (model.into(), b(&interned(format_args!("entity/cow/cow_{}", climate(m)))))
        }
        S::Mooshroom => {
            (
                baby_or(m, "mooshroom#main", "mooshroom_baby#main"),
                b(
                    if m.extra.variant == 1 {
                        "entity/cow/mooshroom_brown"
                    } else {
                        "entity/cow/mooshroom_red"
                    },
                ),
            )
        }
        S::Chicken => {
            let model = if baby {
                "chicken_baby#main"
            } else if m.extra.variant == 2 {
                "cold_chicken#main"
            } else {
                "chicken#main"
            };
            (
                model.into(),
                b(&interned(format_args!("entity/chicken/chicken_{}", climate(m)))),
            )
        }
        S::Sheep => {
            (baby_or(m, "sheep#main", "sheep_baby#main"), b("entity/sheep/sheep"))
        }
        S::Zombie => {
            (baby_or(m, "zombie#main", "zombie_baby#main"), b("entity/zombie/zombie"))
        }
        S::Husk => (baby_or(m, "husk#main", "husk_baby#main"), b("entity/zombie/husk")),
        S::Drowned => {
            (baby_or(m, "drowned#main", "drowned_baby#main"), b("entity/zombie/drowned"))
        }
        S::ZombieVillager => {
            (
                baby_or(m, "zombie_villager#main", "zombie_villager_baby#main"),
                b("entity/zombie_villager/zombie_villager"),
            )
        }
        S::Villager => {
            (
                baby_or(m, "villager#main", "villager_baby#main"),
                b("entity/villager/villager"),
            )
        }
        S::Piglin => {
            (baby_or(m, "piglin#main", "piglin_baby#main"), b("entity/piglin/piglin"))
        }
        S::ZombifiedPiglin => {
            (
                baby_or(m, "zombified_piglin#main", "zombified_piglin_baby#main"),
                b("entity/piglin/zombified_piglin"),
            )
        }
        S::Hoglin => {
            (baby_or(m, "hoglin#main", "hoglin_baby#main"), b("entity/hoglin/hoglin"))
        }
        S::Zoglin => {
            (baby_or(m, "zoglin#main", "zoglin_baby#main"), b("entity/hoglin/zoglin"))
        }
        S::Strider => {
            (
                baby_or(m, "strider#main", "strider_baby#main"),
                b(
                    if m.extra.flag {
                        "entity/strider/strider_cold"
                    } else {
                        "entity/strider/strider"
                    },
                ),
            )
        }
        S::Bee => {
            let angry = super::bee::get(m).is_some_and(|b| b.anger_end > 0);
            let nectar = super::bee::has_nectar(m);
            let texture = match (angry, nectar) {
                (true, true) => "entity/bee/bee_angry_nectar",
                (true, false) => "entity/bee/bee_angry",
                (false, true) => "entity/bee/bee_nectar",
                (false, false) => "entity/bee/bee",
            };
            let layer = baby_or(m, "bee#main", "bee_baby#main");
            let texture = if layer.starts_with("bee_baby") {
                b(texture)
            } else {
                texture.into()
            };
            (layer, texture)
        }
        S::Squid => {
            (baby_or(m, "squid#main", "squid_baby#main"), b("entity/squid/squid"))
        }
        S::GlowSquid => {
            (
                baby_or(m, "glow_squid#main", "glow_squid_baby#main"),
                b("entity/squid/glow_squid"),
            )
        }
        S::Dolphin => {
            (
                baby_or(m, "dolphin#main", "dolphin_baby#main"),
                b("entity/dolphin/dolphin"),
            )
        }
        S::Pufferfish => {
            let model = match m.extra.size {
                0 => "pufferfish_small#main",
                1 => "pufferfish_medium#main",
                _ => "pufferfish_big#main",
            };
            (model.into(), "entity/fish/pufferfish".into())
        }
        S::TropicalFish => {
            if m.extra.variant < 6 {
                ("tropical_fish_small#main".into(), "entity/fish/tropical_a".into())
            } else {
                ("tropical_fish_large#main".into(), "entity/fish/tropical_b".into())
            }
        }
        S::CopperGolem => {
            (m.kind.model_name().into(), super::copper_golem::texture(m).into())
        }
        _ => (m.kind.model_name().into(), m.kind.texture.as_str().into()),
    }
}
/// LivingEntityRenderer.getModelTint (tropical fish base colour).
pub fn body_color(m: &Mob, alpha: f32) -> u32 {
    match m.species() {
        Species::Wolf => super::tame::tint(m, alpha),
        Species::TropicalFish => abgr(DYE[m.extra.variant2 as usize & 15]),
        _ if avatar(m) && m.extra.spectator => 0x26FF_FFFF,
        _ => 0xFFFF_FFFF,
    }
}
/// Extra render layers drawn over the body with the same pose. `age` is the
/// render state's `ageInTicks` (`Mob.age` plus the partial tick).
pub fn layers(m: &Mob, age: f32) -> Vec<Layer> {
    use Species as S;
    let mut v = Vec::new();
    if avatar(m) {
        return v;
    }
    let white = 0xFFFF_FFFF;
    fn push(
        v: &mut Vec<Layer>,
        model: impl Into<Cow<'static, str>>,
        texture: impl Into<Cow<'static, str>>,
        color: u32,
        full_bright: bool,
    ) {
        v.push(Layer {
            model: model.into(),
            texture: texture.into(),
            color,
            full_bright,
            scroll: 0.0,
        })
    }
    match m.species() {
        S::Wolf if m.extra.tame => {
            let (model, texture) = if m.is_baby() {
                ("wolf_baby#main", "entity/wolf/wolf_collar_baby")
            } else {
                ("wolf#main", "entity/wolf/wolf_collar")
            };
            if !m.is_baby() && m.extra.body.as_deref() == Some("minecraft:wolf_armor") {
                let left = (super::WOLF_ARMOR_DURABILITY - m.extra.body_damage) as f32
                    / super::WOLF_ARMOR_DURABILITY as f32;
                push(
                    &mut v,
                    "wolf_armor#main",
                    "entity/equipment/wolf_body/armadillo_scute",
                    white,
                    false,
                );
                let crack = [(0.32, "high"), (0.69, "medium"), (0.95, "low")]
                    .iter()
                    .find(|(t, _)| left < *t);
                if let Some((_, level)) = crack {
                    push(
                        &mut v,
                        "wolf_armor#main",
                        interned(
                            format_args!("entity/wolf/wolf_armor_crackiness_{level}"),
                        ),
                        white,
                        false,
                    );
                }
            }
            push(&mut v, model, texture, abgr(DYE[m.extra.color as usize & 15]), false);
        }
        S::HappyGhast => {
            let harness = m
                .extra
                .body
                .as_deref()
                .and_then(super::happy_ghast::harness_texture);
            if !m.is_baby() && let Some(texture) = &harness {
                push(&mut v, "happy_ghast_harness#main", texture.clone(), white, false);
            }
            if harness.is_some() && super::happy_ghast::is_leash_holder(m) {
                let model = if m.is_baby() {
                    "happy_ghast_baby_ropes#main"
                } else {
                    "happy_ghast_ropes#main"
                };
                push(&mut v, model, "entity/ghast/happy_ghast_ropes", white, false);
            }
        }
        S::Breeze => {
            v.push(Layer {
                model: "breeze#wind".into(),
                texture: "entity/breeze/breeze_wind".into(),
                color: white,
                full_bright: false,
                scroll: scroll_offset(age, BREEZE_WIND_SCROLL),
            });
            push(&mut v, "breeze#eyes", "entity/breeze/breeze_eyes", white, true);
        }
        S::Llama | S::TraderLlama => {
            if let Some((model, texture)) = super::llama::decor(m) {
                push(&mut v, model, texture, white, false);
            }
        }
        s if super::equine::is_equine(s) => {
            let name = super::equine::model_name(s);
            let body = if m.is_baby() {
                interned(format_args!("{name}_baby#main"))
            } else {
                interned(format_args!("{name}#main"))
            };
            if let Some(tex) = super::equine::markings(m) {
                push(&mut v, body, tex, white, false);
            }
            if m.extra.saddled && !m.is_baby() {
                push(
                    &mut v,
                    interned(format_args!("{name}#saddle")),
                    super::equine::saddle_texture(s),
                    white,
                    false,
                );
            }
            if let Some(armor) = m.extra.body.as_deref()
                && let Some(material) = armor
                    .strip_prefix("minecraft:")
                    .and_then(|a| a.strip_suffix("_horse_armor"))
            {
                let tex = interned(
                    format_args!("entity/equipment/horse_body/{material}"),
                );
                let color = if material == "leather" { abgr(0xA06540) } else { white };
                let layer = if matches!(s, S::ZombieHorse | S::SkeletonHorse) {
                    "undead_horse_armor#main"
                } else {
                    "horse_armor#main"
                };
                push(&mut v, layer, tex, color, false);
            }
        }
        S::Pig if m.extra.saddled && !m.is_baby() => {
            push(
                &mut v,
                "pig#saddle",
                "entity/equipment/pig_saddle/saddle",
                white,
                false,
            )
        }
        S::Strider if m.extra.saddled && !m.is_baby() => {
            push(
                &mut v,
                "strider#saddle",
                "entity/equipment/strider_saddle/saddle",
                white,
                false,
            )
        }
        S::Cat if m.extra.tame => {
            let (model, texture) = if m.is_baby() {
                ("cat_baby#collar", "entity/cat/cat_collar_baby")
            } else {
                ("cat#collar", "entity/cat/cat_collar")
            };
            push(&mut v, model, texture, abgr(DYE[m.extra.color as usize & 15]), false);
        }
        S::IronGolem => {
            if let Some(texture) = super::golem::crackiness(m) {
                push(&mut v, "iron_golem#main", texture, white, false);
            }
        }
        S::Nautilus | S::ZombieNautilus if !m.is_baby() => {
            if let Some(texture) = m
                .extra
                .body
                .as_deref()
                .and_then(super::nautilus::armor_texture)
            {
                push(&mut v, "nautilus_armor#main", texture, white, false);
            }
            if m.extra.saddled {
                push(
                    &mut v,
                    "nautilus#saddle",
                    "entity/equipment/nautilus_saddle/saddle",
                    white,
                    false,
                );
            }
        }
        S::SulfurCube => {
            if let Some((model, texture)) = super::sulfur_cube::inner(m) {
                push(&mut v, model, texture, white, false);
            }
        }
        S::Slime => push(&mut v, "slime#outer", "entity/slime/slime", white, false),
        S::Sheep => {
            let jeb = m.custom_name.as_deref() == Some("jeb_");
            let color = abgr(
                if jeb { jeb_wool_rgb(age) } else { sheep_wool_rgb(m.extra.color) },
            );
            if !m.is_baby() && (jeb || m.extra.color != 0) {
                push(
                    &mut v,
                    "sheep#wool_undercoat",
                    "entity/sheep/sheep_wool_undercoat",
                    color,
                    false,
                );
            }
            if !m.extra.sheared {
                let (model, texture) = if m.is_baby() {
                    ("sheep_baby#wool", "entity/sheep/sheep_wool_baby")
                } else {
                    ("sheep#wool", "entity/sheep/sheep_wool")
                };
                push(&mut v, model, texture, color, false);
            }
        }
        S::Creaking if super::creaking::eyes_glowing(m) => {
            push(&mut v, "creaking#eyes", "entity/creaking/creaking_eyes", white, true)
        }
        S::CopperGolem => {
            let eyes = super::copper_golem::eye_texture(m);
            push(&mut v, m.kind.model_name(), eyes, white, true);
        }
        S::Spider | S::CaveSpider => {
            push(&mut v, m.kind.model_name(), "entity/spider/spider_eyes", white, true)
        }
        S::Enderman => {
            push(&mut v, "enderman#main", "entity/enderman/enderman_eyes", white, true)
        }
        S::Phantom => {
            push(&mut v, "phantom#main", "entity/phantom/phantom_eyes", white, true)
        }
        S::Drowned => {
            if m.is_baby() {
                push(
                    &mut v,
                    "drowned_baby#outer",
                    "entity/zombie/drowned_outer_layer_baby",
                    white,
                    false,
                );
            } else {
                push(
                    &mut v,
                    "drowned#outer",
                    "entity/zombie/drowned_outer_layer",
                    white,
                    false,
                );
            }
        }
        S::Stray => {
            push(&mut v, "stray#outer", "entity/skeleton/stray_overlay", white, false)
        }
        S::Bogged if !m.extra.sheared => {
            push(&mut v, "bogged#outer", "entity/skeleton/bogged_overlay", white, false)
        }
        S::TropicalFish => {
            let (model, letter, n) = if m.extra.variant < 6 {
                ("tropical_fish_small#pattern", "a", m.extra.variant + 1)
            } else {
                ("tropical_fish_large#pattern", "b", m.extra.variant - 5)
            };
            push(
                &mut v,
                model,
                interned(format_args!("entity/fish/tropical_{letter}_pattern_{n}")),
                abgr(DYE[m.extra.variant3 as usize & 15]),
                false,
            );
        }
        S::Villager | S::ZombieVillager => {
            let base = if m.species() == S::Villager {
                "villager"
            } else {
                "zombie_villager"
            };
            let model = if m.is_baby() {
                interned(format_args!("{base}_baby#main"))
            } else {
                interned(format_args!("{base}#main"))
            };
            let t = VILLAGER_TYPES[m.extra.variant as usize % VILLAGER_TYPES.len()];
            let folder = if m.species() == S::Villager {
                "entity/villager"
            } else {
                "entity/zombie_villager"
            };
            let group = if m.is_baby() { "baby" } else { "type" };
            push(
                &mut v,
                model.clone(),
                interned(format_args!("{folder}/{group}/{t}")),
                white,
                false,
            );
            let prof = PROFESSIONS[m.extra.variant2 as usize % PROFESSIONS.len()];
            if !m.is_baby() && prof != "none" {
                push(
                    &mut v,
                    model.clone(),
                    interned(format_args!("{folder}/profession/{prof}")),
                    white,
                    false,
                );
                if prof != "nitwit" {
                    let level = [
                        "stone",
                        "iron",
                        "gold",
                        "emerald",
                        "diamond",
                    ][(m.extra.variant3.max(1) as usize - 1).min(4)];
                    push(
                        &mut v,
                        model,
                        interned(format_args!("{folder}/profession_level/{level}")),
                        white,
                        false,
                    );
                }
            }
        }
        _ => {}
    }
    humanoid_armor(m, &mut v);
    v
}
/// `HumanoidArmorLayer.renderArmorPiece`: a baby wears the `humanoid_baby`
/// layer type for every slot (leggings included), an adult the leggings layer
/// for its legs and the `humanoid` one otherwise.
fn humanoid_layer_folder(baby: bool, layer: &str) -> &'static str {
    if baby {
        "humanoid_baby"
    } else if layer == "leggings" {
        "humanoid_leggings"
    } else {
        "humanoid"
    }
}
/// `HumanoidArmorLayer`: the armour models of the mob's main model with the
/// equipment textures (leather tinted, with its overlay).
fn humanoid_armor(m: &Mob, v: &mut Vec<Layer>) {
    if super::equine::is_equine(m.species()) || avatar(m) {
        return;
    }
    let e = &m.extra;
    if e.head.is_none() && e.chest.is_none() && e.legs.is_none() && e.feet.is_none() {
        return;
    }
    let (model, _) = body(m);
    let Some(base) = model.strip_suffix("#main") else {
        return;
    };
    let slots = [
        (&m.extra.head, &m.extra.armor[0], "helmet"),
        (&m.extra.chest, &m.extra.armor[1], "chestplate"),
        (&m.extra.legs, &m.extra.armor[2], "leggings"),
        (&m.extra.feet, &m.extra.armor[3], "boots"),
    ];
    for (item, stack, layer) in slots {
        let Some(name) = item.as_deref().and_then(|i| i.strip_prefix("minecraft:")) else {
            continue;
        };
        let Some(material) = name.strip_suffix(layer).and_then(|n| n.strip_suffix('_'))
        else {
            continue;
        };
        let asset = match material {
            "golden" => "gold",
            "turtle" => "turtle_scute",
            other => other,
        };
        let folder = humanoid_layer_folder(m.is_baby(), layer);
        let model = interned(format_args!("{base}#{layer}"));
        let tint = if asset == "leather" {
            armor_tint(Some(0xA06540), stack.as_ref())
        } else {
            0xFFFF_FFFF
        };
        v.push(Layer {
            model: model.clone(),
            texture: interned(format_args!("entity/equipment/{folder}/{asset}")),
            color: tint,
            full_bright: false,
            scroll: 0.0,
        });
        if asset == "leather" {
            v.push(Layer {
                model,
                texture: interned(
                    format_args!("entity/equipment/{folder}/leather_overlay"),
                ),
                color: 0xFFFF_FFFF,
                full_bright: false,
                scroll: 0.0,
            });
        }
    }
}
impl super::MobType {
    /// The main model layer key without the namespace.
    pub fn model_key(&self) -> String {
        self.model_name().to_owned()
    }
    /// `model_key` without the copy.
    pub fn model_name(&self) -> &'static str {
        super::species::def(self.species).model
    }
}
/// The mob's root transform (LivingEntityRenderer.submit + setupRotations).
pub fn base_transform(m: &Mob, alpha: f32, camera: glam::DVec3) -> glam::Mat4 {
    let a = f64::from(alpha);
    let p = glam::DVec3::new(
        m.previous.x + (m.position.x - m.previous.x) * a,
        m.previous.y + (m.position.y - m.previous.y) * a,
        m.previous.z + (m.position.z - m.previous.z) * a,
    ) - camera + enderman_render_offset(m, alpha)
        + super::shulker::render_offset(m, alpha);
    base_transform_at(m, alpha, glam::Mat4::from_translation(p.as_vec3()))
}
/// `EndermanRenderer.getRenderOffset`: a creepy (angry) enderman is moved by
/// a gaussian 0.02 blocks sideways in x and z every frame. Vanilla draws from
/// the renderer's own random; this one is seeded by the mob and the frame's
/// age, so the body, its carried block and its eyes agree within a frame.
fn enderman_render_offset(m: &Mob, alpha: f32) -> glam::DVec3 {
    use horizon_core::random::{LegacyRandom, RandomSource};
    if m.species() != Species::Enderman || !m.extra.flag {
        return glam::DVec3::ZERO;
    }
    let frame = (m.age as f32 + alpha).to_bits();
    let mut random = LegacyRandom::new((i64::from(m.id) << 32) ^ i64::from(frame));
    let d = 0.02 * f64::from(m.model_scale());
    let x = random.next_gaussian() * d;
    let z = random.next_gaussian() * d;
    glam::DVec3::new(x, 0.0, z)
}
/// `base_transform` from a pose of the entity's origin (`EntityRenderDispatcher.submit`
/// under a pose stack, as `SpawnerRenderer` draws the mob inside a spawner).
pub fn base_transform_at(m: &Mob, alpha: f32, origin: glam::Mat4) -> glam::Mat4 {
    let mut body = m.previous_body_yaw + wrap(m.body_yaw - m.previous_body_yaw) * alpha;
    if shaking(m) {
        let ticks = (m.age as f32 + alpha).floor();
        body += (f64::from(ticks * 3.25).cos() * std::f64::consts::PI * 0.4) as f32;
    }
    let sleep = (m.species() == Species::Villager || avatar(m))
        .then(|| super::villager::sleep_pose(m))
        .flatten();
    let (sleep_offset, rotation) = match sleep {
        Some((angle, step)) => {
            let eye = if avatar(m) {
                m.kind.eye_height as f32 * m.model_scale()
            } else {
                m.awake_dims().2 as f32
            };
            let head = eye - 0.1;
            (
                glam::Mat4::from_translation(
                    glam::Vec3::new(-step[0] * head, 0.0, -step[1] * head),
                ),
                glam::Mat4::from_rotation_y(angle.to_radians())
                    * glam::Mat4::from_rotation_z(90.0_f32.to_radians())
                    * glam::Mat4::from_rotation_y(270.0_f32.to_radians()),
            )
        }
        None => {
            (
                glam::Mat4::IDENTITY,
                super::aquatic::rotations(m, alpha, body)
                    .unwrap_or_else(|| glam::Mat4::from_rotation_y(
                        (180.0 - body).to_radians(),
                    )),
            )
        }
    };
    let mut base = origin * sleep_offset
        * glam::Mat4::from_scale(glam::Vec3::splat(m.model_scale()))
        * pufferfish_bob(m, alpha) * rotation;
    if m.death_time > 0 && !super::creaking::tearing_down(m) {
        let f = ((m.death_time as f32 + alpha - 1.0) / 20.0 * 1.6).sqrt().min(1.0);
        base *= glam::Mat4::from_rotation_z(f * flip_degrees(m).to_radians());
    } else if m.extra.auto_spin_attack {
        let pitch = m.previous_head_pitch
            + (m.head_pitch - m.previous_head_pitch) * alpha;
        base
            *= glam::Mat4::from_rotation_x((-90.0 - pitch).to_radians())
                * glam::Mat4::from_rotation_y(
                    ((m.age as f32 + alpha) * -75.0).to_radians(),
                );
    } else if matches!(m.custom_name.as_deref(), Some("Dinnerbone" | "Grumm")) {
        base
            *= glam::Mat4::from_translation(glam::Vec3::new(0.0, m.height() + 0.1, 0.0))
                * glam::Mat4::from_rotation_z(PI);
    }
    if m.species() == Species::IronGolem {
        let s = anim::state(m, alpha);
        if s.speed >= 0.01 {
            let wp = s.pos + 6.0;
            let triangle_wave = ((wp % 13.0 - 6.5).abs() - 3.25) / 3.25;
            base *= glam::Mat4::from_rotation_z((6.5 * triangle_wave).to_radians());
        }
    }
    if m.extra.fall_flying {
        let ticks = m.extra.fall_fly_ticks as f32 + alpha;
        let scale = (ticks * ticks / 100.0).min(1.0);
        let pitch = m.previous_head_pitch
            + (m.head_pitch - m.previous_head_pitch) * alpha;
        if !m.extra.auto_spin_attack {
            base *= glam::Mat4::from_rotation_x((scale * (-90.0 - pitch)).to_radians());
        }
        let yaw = m.previous_head_yaw + wrap(m.head_yaw - m.previous_head_yaw) * alpha;
        let (look_x, look_z) = (-(yaw.to_radians()).msin(), (yaw.to_radians()).mcos());
        let v = m.extra.previous_velocity.lerp(m.velocity, f64::from(alpha));
        let view_scale = (pitch.to_radians()).mcos();
        let (look_x, look_z) = (look_x * view_scale, look_z * view_scale);
        let view_length = f64::from(look_x * look_x + look_z * look_z);
        let speed = v.x * v.x + v.z * v.z;
        if speed > f64::from(1.0e-5f32) && view_length > f64::from(1.0e-5f32) {
            let dot = (v.x * f64::from(look_x) + v.z * f64::from(look_z))
                / (speed * view_length).sqrt();
            let sign = (v.x * f64::from(look_z) - v.z * f64::from(look_x)).signum();
            base
                *= glam::Mat4::from_rotation_y(
                    (sign * horizon_core::jmath::acos(dot.abs().min(1.0))) as f32,
                );
        }
    } else if m.species() == Species::Drowned {
        let amount = m.extra.swim.amount(alpha);
        if amount > 0.0 {
            let pitch = m.previous_head_pitch
                + (m.head_pitch - m.previous_head_pitch) * alpha;
            let middle = glam::Vec3::new(0.0, m.height() / 2.0, 0.0);
            base
                *= glam::Mat4::from_translation(middle)
                    * glam::Mat4::from_rotation_x(
                        (amount * (-10.0 - pitch)).to_radians(),
                    ) * glam::Mat4::from_translation(-middle);
        }
    } else if m.extra.swim.amount(alpha) > 0.0 {
        let amount = m.extra.swim.amount(alpha);
        let pitch = m.previous_head_pitch
            + (m.head_pitch - m.previous_head_pitch) * alpha;
        let target = if m.extra.swim.in_water { -90.0 - pitch } else { -90.0 };
        base *= glam::Mat4::from_rotation_x((amount * target).to_radians());
        if m.extra.swim.visually {
            base *= glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.0, 0.3));
        }
    }
    base *= species_rotations(m, alpha);
    base *= glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
    base *= species_scale(m, alpha);
    base * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0))
}
/// `PufferfishRenderer.setupRotations`: `translate(0, cos(ageInTicks * 0.05) * 0.08, 0)`
/// before the usual rotations, so a pufferfish bobs up and down.
fn pufferfish_bob(m: &Mob, alpha: f32) -> glam::Mat4 {
    if m.species() != Species::Pufferfish {
        return glam::Mat4::IDENTITY;
    }
    glam::Mat4::from_translation(
        glam::Vec3::new(0.0, pufferfish_bob_y(m.age as f32 + alpha), 0.0),
    )
}
fn pufferfish_bob_y(age_in_ticks: f32) -> f32 {
    (age_in_ticks * 0.05).mcos() * 0.08
}
/// `LivingEntityRenderer.isShaking`: a fully frozen mob (the skeleton's own
/// `isShaking` is its stray conversion only), converting zombies, piglins
/// and hoglins, a zombie villager being cured and a cold strider.
fn shaking(m: &Mob) -> bool {
    if m.is_fully_frozen() && m.species() != Species::Skeleton {
        return true;
    }
    match m.species() {
        Species::Strider => super::strider::suffocating(m),
        Species::ZombieVillager => super::zombie_villager::is_converting(m),
        Species::Zombie | Species::Husk | Species::Skeleton => m.extra.counter2 > 0,
        Species::Piglin | Species::PiglinBrute | Species::Hoglin => {
            !m.extra.flag2 && m.extra.counter2 > 0
        }
        _ => false,
    }
}
fn flip_degrees(m: &Mob) -> f32 {
    match m.species() {
        Species::Spider
        | Species::CaveSpider
        | Species::Silverfish
        | Species::Endermite => 180.0,
        _ => 90.0,
    }
}
fn species_rotations(m: &Mob, alpha: f32) -> glam::Mat4 {
    match m.species() {
        Species::Cat => super::tame::cat_rotations(m, alpha),
        Species::Panda => super::panda::rotations(m, alpha),
        Species::Shulker => super::shulker::rotations(m),
        Species::Fox if super::fox::flag(
            m,
            super::fox::POUNCING | super::fox::FACEPLANTED,
        ) => {
            let pitch = m.previous_head_pitch
                + (m.head_pitch - m.previous_head_pitch) * alpha;
            glam::Mat4::from_rotation_x((-pitch).to_radians())
        }
        Species::Phantom => {
            let pitch = m.previous_head_pitch
                + (m.head_pitch - m.previous_head_pitch) * alpha;
            glam::Mat4::from_rotation_x(pitch.to_radians())
        }
        _ => glam::Mat4::IDENTITY,
    }
}
/// Creeper.getSwelling: the fuse's progress, past 1 in its last two ticks.
fn creeper_swelling(m: &Mob, alpha: f32) -> f32 {
    (m.previous_swell as f32 + alpha * (m.swell - m.previous_swell) as f32)
        / (m.max_swell - 2) as f32
}
/// LivingEntityRenderer.getOverlayCoords for the body: the hurt red, or a
/// swelling creeper's white flash. Layers take the red only (progress 0).
fn body_overlay(m: &Mob, alpha: f32) -> u32 {
    let white = if m.species() == Species::Creeper {
        crate::overlay::creeper_progress(creeper_swelling(m, alpha))
    } else {
        0.0
    };
    crate::overlay::living(white, red_overlay(m))
}
/// LivingEntityRenderState.hasRedOverlay: hurt or dying (not a creaking
/// tearing down).
fn red_overlay(m: &Mob) -> bool {
    m.hurt_time > 0 || (m.death_time > 0 && !super::creaking::tearing_down(m))
}
fn species_scale(m: &Mob, alpha: f32) -> glam::Mat4 {
    if avatar(m) {
        return glam::Mat4::from_scale(glam::Vec3::splat(0.9375));
    }
    match m.species() {
        Species::Wither => {
            glam::Mat4::from_scale(glam::Vec3::splat(super::wither::scale(m, alpha)))
        }
        Species::Creeper => {
            let g = creeper_swelling(m, alpha);
            let wobble = 1.0 + (g * 100.0).msin() * g * 0.01;
            let s = g.clamp(0.0, 1.0);
            let s4 = s * s * s * s;
            let xz = (1.0 + s4 * 0.4) * wobble;
            let y = (1.0 + s4 * 0.1) / wobble;
            glam::Mat4::from_scale(glam::Vec3::new(xz, y, xz))
        }
        Species::Slime | Species::MagmaCube => super::cube::scale(m, alpha),
        Species::SulfurCube => super::sulfur_cube::scale(m, alpha),
        Species::Phantom => {
            let s = 1.0 + 0.15 * m.extra.size as f32;
            glam::Mat4::from_scale(glam::Vec3::splat(s))
                * glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.3125, 0.1875))
        }
        _ => glam::Mat4::IDENTITY,
    }
}
/// Draw a part tree with a pose map, texture rect, colour and light.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    part: &Part,
    base: glam::Mat4,
    poses: &Poses,
    uv: [f32; 4],
    color: u32,
    light: u32,
    overlay: u32,
    out: &mut Vec<Vertex>,
) {
    draw_scrolled(part, base, poses, uv, 0.0, color, light, overlay, out);
}
/// `draw` for a texture that scrolls: the vertex's U is moved by the offset
/// and wraps inside the texture, as the texture matrix of
/// `RenderTypes.breezeWind` does.
#[allow(clippy::too_many_arguments)]
pub fn draw_scrolled(
    part: &Part,
    base: glam::Mat4,
    poses: &Poses,
    uv: [f32; 4],
    scroll: f32,
    color: u32,
    light: u32,
    overlay: u32,
    out: &mut Vec<Vertex>,
) {
    draw_normals(
        part,
        base,
        ModelNormals::new(base),
        poses,
        uv,
        scroll,
        color,
        light,
        overlay,
        out,
    );
}
#[allow(clippy::too_many_arguments)]
fn draw_normals(
    part: &Part,
    base: glam::Mat4,
    normals: ModelNormals,
    poses: &Poses,
    uv: [f32; 4],
    scroll: f32,
    color: u32,
    light: u32,
    overlay: u32,
    out: &mut Vec<Vertex>,
) {
    let p = poses.pose(part);
    let m = posed(base, p);
    if poses.is_hidden(part) || p[6] == 0.0 && p[7] == 0.0 && p[8] == 0.0 {
        return;
    }
    let normals = normals.part(p);
    if !poses.skips(part) {
        for poly in &part.polygons {
            let normal = poly.vertex_normal(normals);
            for v in poly.vertices {
                let pos = m.transform_point3(glam::Vec3::new(v[0], v[1], v[2]) / 16.0);
                let glint_uv = if overlay == ARMOR_GLINT {
                    [v[3], v[4]]
                } else {
                    [0.0, 0.0]
                };
                out.push(Vertex {
                    normal,
                    pos: pos.to_array(),
                    uv: [
                        uv[0] + (uv[2] - uv[0]) * scroll_u(v[3], scroll),
                        uv[1] + (uv[3] - uv[1]) * v[4],
                    ],
                    layer: overlay,
                    color,
                    light,
                    glint_uv,
                });
            }
        }
    }
    for c in &part.children {
        draw_normals(c, m, normals, poses, uv, scroll, color, light, overlay, out);
    }
}
/// A part's matrix under `base` for its pose (`ModelPart.translateAndRotate`
/// and its scale), as `draw` and the GPU path (`super::skin`) both use it.
pub(super) fn posed(base: glam::Mat4, p: [f32; 9]) -> glam::Mat4 {
    base * glam::Mat4::from_translation(glam::Vec3::new(p[0], p[1], p[2]) / 16.0)
        * glam::Mat4::from_quat(
            glam::Quat::from_euler(glam::EulerRot::ZYX, p[5], p[4], p[3]),
        ) * glam::Mat4::from_scale(glam::Vec3::new(p[6], p[7], p[8]))
}
/// Keeps the original multiplication order when the local rotation was reused.
pub(super) fn posed_rotated(
    base: glam::Mat4,
    p: [f32; 9],
    rotation: glam::Mat4,
) -> glam::Mat4 {
    base * glam::Mat4::from_translation(glam::Vec3::new(p[0], p[1], p[2]) / 16.0)
        * rotation * glam::Mat4::from_scale(glam::Vec3::new(p[6], p[7], p[8]))
}
/// `draw_scrolled` through the GPU path when the mob takes it (`skin`), on
/// the CPU otherwise.
#[allow(clippy::too_many_arguments)]
pub fn put(
    skin: &mut Option<&mut super::skin::Skin>,
    part: &std::sync::Arc<Part>,
    base: glam::Mat4,
    poses: &Poses,
    uv: [f32; 4],
    scroll: f32,
    color: u32,
    light: u32,
    overlay: u32,
    out: &mut Vec<Vertex>,
) {
    let paint = super::skin::Paint {
        uv,
        scroll,
        color,
        light,
        overlay,
    };
    if let Some(skin) = skin && skin.draw(part, base, poses, paint) {
        return;
    }
    draw_scrolled(part, base, poses, uv, scroll, color, light, overlay, out);
}
/// Render a mob with its variant model, texture and layers.
pub fn render(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) {
    render_with(m, alpha, camera, atlas, light, data, &[], out);
}
/// `render` plus extra layers posed like the body (`HumanoidArmorLayer`:
/// the armour model's parts copy the body's): (model, texture rect, colour,
/// overlay: 0 or `ARMOR_GLINT`, as `EquipmentLayerRenderer` never tints them).
#[allow(clippy::too_many_arguments)]
pub fn render_with(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    extra: &[(&Part, [f32; 4], u32, u32, bool)],
    out: &mut Vec<Vertex>,
) {
    if super::trader::is_invisible(m) {
        if m.species() == Species::Illusioner {
            let age = m.age as f32 + alpha;
            for (i, off) in super::illusioner::offsets(m, alpha).iter().enumerate() {
                let w = super::illusioner::wobble(i, age);
                let shifted = camera
                    - glam::DVec3::new(off[0] + w[0], off[1] + w[1], off[2] + w[2]);
                let base = base_transform(m, alpha, shifted);
                render_visible(
                    m,
                    alpha,
                    base,
                    atlas,
                    light,
                    data,
                    extra,
                    true,
                    out,
                    None,
                );
            }
        }
        return;
    }
    render_visible(
        m,
        alpha,
        base_transform(m, alpha, camera),
        atlas,
        light,
        data,
        extra,
        true,
        out,
        None,
    );
}
/// `render` with the model's ordinary draws posed on the GPU when the mob
/// allows it (`super::skin`): the same picture, no vertices on the CPU.
#[allow(clippy::too_many_arguments)]
pub fn render_skinned(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
    skin: &mut super::skin::Skin,
) {
    if super::trader::is_invisible(m) {
        render_with(m, alpha, camera, atlas, light, data, &[], out);
        return;
    }
    render_visible(
        m,
        alpha,
        base_transform(m, alpha, camera),
        atlas,
        light,
        data,
        &[],
        true,
        out,
        Some(skin),
    );
}
/// `CapeLayer.submit` with `PlayerCapeModel`: the player model's body posed
/// as the player's, its cape turned by `ModelPart.rotateBy` with the cape's
/// flap and leans. `base` is the player's root transform (moved out over a
/// chestplate); `uv` the cape texture.
#[allow(clippy::too_many_arguments)]
pub fn cape(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    uv: [f32; 4],
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) {
    let Some(model) = m.kind.layer("player#cape") else {
        return;
    };
    let s = anim::state(m, alpha);
    let mut poses = Poses::new(model);
    anim::setup(m, &s, &mut poses, &data.animations);
    let a = f64::from(alpha);
    let at = [
        m.previous.x + (m.position.x - m.previous.x) * a,
        m.previous.y + (m.position.y - m.previous.y) * a,
        m.previous.z + (m.position.z - m.previous.z) * a,
    ];
    let body = m.previous_body_yaw + wrap(m.body_yaw - m.previous_body_yaw) * alpha;
    let ticks = m.extra.fall_fly_ticks as f32 + alpha;
    let gliding = (ticks * ticks / 100.0).clamp(0.0, 1.0);
    let [flap, lean, lean2] = m.extra.avatar.cape(alpha, at, body, gliding);
    let pose = poses.get("cape");
    let by = glam::Quat::from_rotation_y(-PI)
        * glam::Quat::from_rotation_x((6.0 + lean / 2.0 + flap).to_radians())
        * glam::Quat::from_rotation_z((lean2 / 2.0).to_radians())
        * glam::Quat::from_rotation_y((180.0 - lean2 / 2.0).to_radians());
    let now = glam::Quat::from_euler(glam::EulerRot::ZYX, pose[5], pose[4], pose[3]);
    let (z, y, x) = (now * by).to_euler(glam::EulerRot::ZYX);
    (pose[3], pose[4], pose[5]) = (x, y, z);
    draw(model, base, &poses, uv, 0xFFFF_FFFF, light, 0, out);
}
/// `Deadmau5EarsLayer.submit` with `PlayerEarsModel`: the player model's
/// head carrying two 6x6x1 ears (the model's other cubes are cleared),
/// drawn with the player's skin as `RenderTypes.entitySolid`. The caller has
/// checked `showExtraEars`; an invisible avatar draws no ears.
#[allow(clippy::too_many_arguments)]
pub fn ears(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    uv: [f32; 4],
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) {
    if super::trader::is_invisible(m) {
        return;
    }
    let Some(model) = m.kind.layer("player#ears") else {
        return;
    };
    let s = anim::state(m, alpha);
    let mut poses = Poses::new(model);
    anim::setup(m, &s, &mut poses, &data.animations);
    draw(model, base, &poses, uv, 0xFFFF_FFFF, light, body_overlay(m, alpha), out);
}
/// `PlayerModel.bodyParts`: what `getRandomBodyPart` picks from.
const BODY_PARTS: [&str; 6] = [
    "head",
    "body",
    "left_arm",
    "right_arm",
    "left_leg",
    "right_leg",
];
/// The cubes of a part as `ModelPart.cubes` has them: min and max corners in
/// model units. The part keeps six faces per cube, in cube order.
fn cube_bounds(part: &Part) -> Vec<[f32; 6]> {
    let size = if part.polygons.len().is_multiple_of(6) {
        6
    } else {
        part.polygons.len().max(1)
    };
    part.polygons
        .chunks(size)
        .map(|faces| {
            let mut b = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
            for v in faces.iter().flat_map(|f| f.vertices) {
                for k in 0..3 {
                    b[k] = b[k].min(v[k]);
                    b[3 + k] = b[3 + k].max(v[k]);
                }
            }
            b
        })
        .collect()
}
/// `StuckInBodyLayer.snapToFace`.
fn snap_to_face(value: f32) -> f32 {
    if value > 0.5 { 1.0 } else { 0.5 }
}
/// `StuckInBodyLayer.submit` (with `submitStuckItem`) for a player: `count`
/// copies of `item` (`ArrowLayer`'s arrow model, `BeeStingerLayer`'s stinger)
/// stuck in random cubes of the posed body, placed from a random source
/// seeded by the entity id `id`. `on_surface` is `PlacementStyle.ON_SURFACE`.
#[allow(clippy::too_many_arguments)]
pub fn stuck_in_body(
    m: &Mob,
    id: i32,
    alpha: f32,
    camera: glam::DVec3,
    data: &MobData,
    count: i32,
    on_surface: bool,
    item: &Part,
    uv: [f32; 4],
    light: u32,
    out: &mut Vec<Vertex>,
) {
    use horizon_core::random::{LegacyRandom, RandomSource};
    if count <= 0 {
        return;
    }
    let base = base_transform(m, alpha, camera);
    let s = anim::state(m, alpha);
    let (model_key, _) = body(m);
    let model = m
        .kind
        .layer(&model_key)
        .cloned()
        .unwrap_or_else(|| m.kind.model.clone());
    let mut poses = Poses::new(&model);
    anim::setup(m, &s, &mut poses, &data.animations);
    let mut random = LegacyRandom::new(i64::from(id));
    for _ in 0..count {
        let name = BODY_PARTS[random.next_int_bound(BODY_PARTS.len() as i32).unwrap_or(0)
            as usize];
        let Some(part) = find_part(&model, name) else {
            return;
        };
        let cubes = cube_bounds(part);
        let Ok(index) = random.next_int_bound(cubes.len() as i32) else {
            return;
        };
        let cube = cubes[index as usize];
        let Some(mut t) = part_matrix(&model, base, &poses, name, 0.0) else {
            return;
        };
        let mut mid = [random.next_float(), random.next_float(), random.next_float()];
        if on_surface {
            let plane = random.next_int_bound(3).unwrap_or(0) as usize;
            if plane < 3 {
                mid[plane] = snap_to_face(mid[plane]);
            }
        }
        let lerp = |k: usize| cube[k] + mid[k] * (cube[3 + k] - cube[k]);
        t
            *= glam::Mat4::from_translation(
                glam::Vec3::new(lerp(0), lerp(1), lerp(2)) / 16.0,
            );
        let (dx, dy, dz) = (
            -(mid[0] * 2.0 - 1.0),
            -(mid[1] * 2.0 - 1.0),
            -(mid[2] * 2.0 - 1.0),
        );
        let dxz = f64::from(dx * dx + dz * dz).sqrt() as f32;
        let degrees = |y: f32, x: f32| {
            (horizon_core::jmath::atan2(f64::from(y), f64::from(x)) * 180.0
                / f64::from(std::f32::consts::PI)) as f32
        };
        let y_rot = degrees(dx, dz);
        let x_rot = degrees(dy, dxz);
        t
            *= glam::Mat4::from_rotation_y((y_rot - 90.0).to_radians())
                * glam::Mat4::from_rotation_z(x_rot.to_radians());
        super::render_model(item, t, &|p| p.pose, uv, light, 0, out);
    }
}
/// `ParrotOnShoulderLayer.submitOnShoulder`: a parrot of `variant` on the
/// avatar's left or right shoulder, in `ParrotModel.Pose.ON_SHOULDER` with
/// the avatar's walk animation, age and head turn. `base` is the avatar's
/// root transform; `parrot` the parrot's entity type (its model).
#[allow(clippy::too_many_arguments)]
pub fn shoulder_parrot(
    avatar: &Mob,
    parrot: &super::MobType,
    variant: i32,
    left: bool,
    alpha: f32,
    base: glam::Mat4,
    atlas: Atlas,
    light: u32,
    out: &mut Vec<Vertex>,
) {
    let Some(uv) = atlas(&shoulder_parrot_texture(variant)) else {
        return;
    };
    let model = parrot.layer(&parrot.model_key()).unwrap_or(&parrot.model);
    let s = anim::State {
        baby: false,
        ..anim::state(avatar, alpha)
    };
    let mut poses = Poses::new(model);
    super::body_anim::parrot(
        &mut poses,
        &s,
        super::body_anim::ParrotPose::OnShoulder,
        0.0,
    );
    let y = if avatar.extra.crouching { -1.3 } else { -1.5 };
    let x = if left { 0.4 } else { -0.4 };
    let base = base * glam::Mat4::from_translation(glam::Vec3::new(x, y, 0.0));
    draw(model, base, &poses, uv, 0xFFFF_FFFF, light, 0, out);
}
/// `ParrotRenderer.getVariantTexture` for `Parrot.Variant.byId` (an id out
/// of range is clamped).
fn shoulder_parrot_texture(variant: i32) -> String {
    const NAMES: [&str; 5] = ["red_blue", "blue", "green", "yellow_blue", "grey"];
    let name = usize::try_from(variant)
        .ok()
        .and_then(|i| NAMES.get(i))
        .unwrap_or(if variant < 0 { &NAMES[0] } else { &NAMES[4] });
    format!("entity/parrot/parrot_{name}")
}
/// `render_with` without the body, for an avatar the viewer can't see:
/// `LivingEntityRenderer.submit` gets no render type for an invisible body
/// (`isBodyVisible`, `isInvisibleTo`), but its layers still draw
/// (`HumanoidArmorLayer`: the armour in `extra`).
#[allow(clippy::too_many_arguments)]
pub fn render_layers(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    extra: &[(&Part, [f32; 4], u32, u32, bool)],
    out: &mut Vec<Vertex>,
) {
    let base = base_transform(m, alpha, camera);
    render_visible(m, alpha, base, atlas, light, data, extra, false, out, None);
}
/// A mob drawn under `origin` instead of at its position (the spinning mob
/// of `SpawnerRenderer.submitEntityInSpawner`).
pub fn render_posed(
    m: &Mob,
    alpha: f32,
    origin: glam::Mat4,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) {
    let base = base_transform_at(m, alpha, origin);
    render_visible(m, alpha, base, atlas, light, data, &[], true, out, None);
}
/// The custom model's key of a body model: `main` is the wide player model, `kind.model`.
fn cem_key(model_key: &str) -> &str {
    if model_key == "main" { "player#main" } else { model_key }
}
/// Every texture [`render_visible`] may look up for `m` now (keep the two in step: a texture
/// drawn but not listed here never gets a random pick): the body's and each layer's, a
/// custom model's own texture beside them (`cem_texture` falls back to the vanilla one when
/// the atlas lacks it, and only the layer a `.jem` was made for takes it, `paints_with_jem`),
/// and the warden's glowing layers (whether they glow this frame or not). Random textures
/// choose a variant for each.
pub fn drawn_textures(m: &Mob, data: &MobData) -> Vec<String> {
    fn push(
        names: &mut Vec<String>,
        custom: Option<&super::cem::CemLayer>,
        vanilla: String,
    ) {
        if let Some(texture) = custom.and_then(|c| c.texture.clone()) {
            names.push(texture);
        }
        names.push(vanilla);
    }
    let cem = data.cem.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (model_key, texture) = body(m);
    let cem_key = cem_key(&model_key);
    let mut names = Vec::new();
    push(&mut names, cem.layer(cem_key), texture.into_owned());
    let mut all_layers = layers(m, m.age as f32);
    if m.species() == Species::Warden {
        for (model, texture, _) in super::warden::emissive_layers(m, 0.0) {
            all_layers
                .push(Layer {
                    model: model.into(),
                    texture: texture.into(),
                    color: 0,
                    full_bright: true,
                    scroll: 0.0,
                });
        }
    }
    let body_keys = [&*model_key, cem_key];
    for (i, layer) in all_layers.iter().enumerate() {
        let custom = cem
            .layer(&layer.model)
            .filter(|_| paints_with_jem(&all_layers, i, body_keys));
        push(&mut names, custom, layer.texture.to_string());
    }
    names
}
/// The texture a model drawn as `custom` takes: the `.jem`'s when the atlas has it, else
/// `vanilla` (a texture the atlas lacks would hide the model).
fn cem_texture<'a>(
    custom: Option<&'a super::cem::CemLayer>,
    vanilla: &'a str,
    atlas: Atlas<'_>,
) -> &'a str {
    custom
        .and_then(|c| c.texture.as_deref())
        .filter(|t| atlas(t).is_some())
        .unwrap_or(vanilla)
}
/// Whether `layers[i]` is the layer its model's `.jem` was made for, so the `.jem`'s texture
/// paints it: the first drawn with that model (`sheep#wool`, a wolf's armour). Later layers
/// on the same model are overlays (the armour's cracks), like those on the body's model
/// (`body`: eyes, collars, a villager's type and profession), and keep their own textures
/// over the new geometry.
fn paints_with_jem(layers: &[Layer], i: usize, body: [&str; 2]) -> bool {
    let key = &*layers[i].model;
    !body.contains(&key) && layers[..i].iter().all(|l| l.model != key)
}
/// The model a mob's body draws with — the packs' custom one when it has one — posed the way
/// the body draws it (the vanilla animation, then the pack's own), with the `attachments`
/// anchors its bones declare. Everything that anchors to a mob — an item in its hand, a
/// block on its head — walks the same tree the player saw.
fn posed_body(
    m: &Mob,
    alpha: f32,
    data: &MobData,
) -> (std::sync::Arc<Part>, Poses, Vec<(String, String, [f32; 3])>) {
    let s = anim::state(m, alpha);
    let (model_key, _) = body(m);
    let cem = data.cem.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let custom = cem.layer(cem_key(&model_key));
    let model = match custom {
        Some(c) => c.model.clone(),
        None => m.kind.layer(&model_key).cloned().unwrap_or_else(|| m.kind.model.clone()),
    };
    let mut poses = Poses::new(&model);
    anim::setup(m, &s, &mut poses, &data.animations);
    if let Some(c) = custom && let Some(animation) = &c.animation {
        animation.run(m, &s, &mut poses);
    }
    let anchors = custom.map_or(Vec::new(), |c| c.anchors.clone());
    (model, poses, anchors)
}
#[allow(clippy::too_many_arguments)]
fn render_visible(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    atlas: Atlas,
    light: u32,
    data: &MobData,
    extra: &[(&Part, [f32; 4], u32, u32, bool)],
    body_visible: bool,
    out: &mut Vec<Vertex>,
    skin: Option<&mut super::skin::Skin>,
) {
    let extra = if layers_shown(m) { extra } else { &[] };
    let overlay = u32::from(red_overlay(m));
    let s = anim::state(m, alpha);
    let (model_key, texture) = body(m);
    let cem = data.cem.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let cem_key = cem_key(&model_key);
    let custom_body = cem.layer(cem_key);
    let body_animation = custom_body.and_then(|c| c.animation.as_ref());
    let model = match custom_body {
        Some(custom) => custom.model.clone(),
        None => {
            m.skin_3d
                .clone()
                .or_else(|| m.kind.layer(&model_key).cloned())
                .unwrap_or_else(|| m.kind.model.clone())
        }
    };
    let texture = cem_texture(custom_body, &texture, atlas);
    let random = data
        .random_textures
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let blinks = data.blinks.read().unwrap_or_else(std::sync::PoisonError::into_inner);
    let sprite = |texture: &str| {
        let variant = (!random.is_empty())
            .then(|| super::random_textures::texture(m, &random, texture))
            .flatten()
            .filter(|v| atlas(v).is_some());
        let frame = (!blinks.is_empty())
            .then(|| blinks.get(variant.unwrap_or(texture)))
            .flatten()
            .and_then(|b| {
                if super::villager::sleeping(m) {
                    Some(b.shut.as_str())
                } else {
                    b.frame(s.age as u32, m.id.wrapping_mul(0x9E37_79B1))
                }
            })
            .filter(|f| atlas(f).is_some());
        let name = frame.or(variant)?;
        Some((name, atlas(name)?))
    };
    let emissive = data
        .emissive
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let emissive_uv = |drawn: &str| {
        if emissive.is_empty() {
            return None;
        }
        atlas(emissive.overlay(drawn)?)
    };
    let mut poses = Poses::pooled();
    poses.reset(&model);
    anim::setup(m, &s, &mut poses, &data.animations);
    if let Some(animation) = body_animation {
        animation.run(m, &s, &mut poses);
    }
    let mut all_layers = layers(m, s.age);
    if m.species() == Species::Warden {
        for (model, texture, a) in super::warden::emissive_layers(m, alpha) {
            if a > 0.0 {
                all_layers
                    .push(Layer {
                        model: model.into(),
                        texture: texture.into(),
                        color: ((a.clamp(0.0, 1.0) * 255.0) as u32) << 24 | 0x00FF_FFFF,
                        full_bright: true,
                        scroll: 0.0,
                    });
            }
        }
    }
    let mut skin = skin;
    let posable = match skin.as_deref_mut() {
        Some(skin) => {
            custom_body.is_none() && extra.is_empty()
                && m.species() != Species::SulfurCube && !translucent_body(m)
                && energy_swirl(m, s.age).is_none() && skin.supports(&model)
                && all_layers
                    .iter()
                    .all(|layer| {
                        (skin.models.tagged
                            && (eyes_layer(m, layer) || !layer.full_bright)
                            || !layer.full_bright && !translucent_layer(m, layer))
                            && cem.layer(&layer.model).is_none()
                            && m
                                .kind
                                .layer(&layer.model)
                                .is_none_or(|part| skin.supports(part))
                    })
        }
        None => false,
    };
    if !posable {
        skin = None;
    }
    let draw_body = |out: &mut Vec<Vertex>, skin: &mut Option<&mut super::skin::Skin>| {
        let drawn = sprite(texture).or_else(|| Some((texture, atlas(texture)?)));
        let uv = drawn
            .map(|(_, uv)| uv)
            .or_else(|| { if avatar(m) { atlas(&m.kind.texture) } else { None } });
        if body_visible && let Some(uv) = uv {
            let start = out.len();
            put(
                skin,
                &model,
                base,
                &poses,
                uv,
                0.0,
                body_color(m, alpha),
                light,
                body_overlay(m, alpha),
                out,
            );
            if let Some(glow) = drawn.and_then(|(name, _)| emissive_uv(name)) {
                put(
                    skin,
                    &model,
                    base,
                    &poses,
                    glow,
                    0.0,
                    body_color(m, alpha),
                    FULL_BRIGHT,
                    0,
                    out,
                );
            }
            if translucent_body(m) {
                mark_oit(&mut out[start..]);
            }
        }
    };
    let shell_last = m.species() == Species::SulfurCube;
    if !shell_last {
        draw_body(out, &mut skin);
    }
    for (part, uv, color, layer_overlay, glows) in extra {
        let light = if *glows { FULL_BRIGHT } else { light };
        draw(part, base, &poses, *uv, *color, light, *layer_overlay, out);
    }
    let mut layer_poses = Poses::pooled();
    let body_keys = [&*model_key, cem_key];
    for (i, layer) in all_layers.iter().enumerate() {
        let custom = cem.layer(&layer.model);
        let texture = cem_texture(
            custom.filter(|_| paints_with_jem(&all_layers, i, body_keys)),
            &layer.texture,
            atlas,
        );
        let part = match custom {
            Some(custom) => custom.model.clone(),
            None => {
                match m.kind.layer(&layer.model) {
                    Some(part) => part.clone(),
                    None => continue,
                }
            }
        };
        let Some((drawn, uv)) = sprite(texture)
            .or_else(|| Some((texture, atlas(texture)?))) else {
            continue;
        };
        if custom.is_none() && custom_body.is_none()
            && std::sync::Arc::ptr_eq(&part, &model)
        {
            layer_poses.copy_from(&poses);
        } else {
            layer_poses.reset(&part);
            anim::setup(m, &s, &mut layer_poses, &data.animations);
            if let Some(animation) = custom.and_then(|c| c.animation.as_ref()) {
                animation.run(m, &s, &mut layer_poses);
            }
        }
        if matches!(m.species(), Species::Villager | Species::ZombieVillager)
            && let Some((folder, variant)) = layer
                .texture
                .split_once("/type/")
                .or_else(|| layer.texture.split_once("/baby/"))
        {
            let profession = PROFESSIONS[m.extra.variant2 as usize % PROFESSIONS.len()];
            data.villager_hats
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .apply(folder, variant, profession, &mut layer_poses);
        }
        let light = if layer.full_bright { FULL_BRIGHT } else { light };
        let overlay = match armor_slot(&layer.model) {
            Some(slot) if super::gear::enchanted(m, slot) => ARMOR_GLINT,
            Some(_) => 0,
            None => overlay,
        };
        let start = out.len();
        let first_job = skin.as_deref().map(|skin| skin.frame.jobs.len());
        put(
            &mut skin,
            &part,
            base,
            &layer_poses,
            uv,
            layer.scroll,
            layer.color,
            light,
            overlay,
            out,
        );
        if let Some(glow) = emissive_uv(drawn) {
            put(
                &mut skin,
                &part,
                base,
                &layer_poses,
                glow,
                layer.scroll,
                layer.color,
                FULL_BRIGHT,
                0,
                out,
            );
        }
        if translucent_layer(m, layer) {
            mark_oit(&mut out[start..]);
        }
        if let (Some(skin), Some(first)) = (skin.as_deref_mut(), first_job) {
            let tagged = translucent_layer(m, layer);
            let eyes = layer.full_bright && m.species() != Species::Warden;
            for job in &mut skin.frame.jobs[first..] {
                if tagged {
                    job.overlay |= crate::gfx::OIT_ENTITY;
                }
                if eyes {
                    job.overlay |= u32::from(EYES_TIER) << 16;
                    job.mode = crate::gfx::entity_skin::SkinJob::FLAT;
                }
            }
        }
        if layer.full_bright && m.species() != Species::Warden {
            for vertex in &mut out[start..] {
                vertex.normal = [0; 4];
                vertex.layer |= u32::from(EYES_TIER) << 16;
            }
        }
        if layer.full_bright && m.species() == Species::Warden {
            for vertex in &mut out[start..] {
                vertex.layer |= u32::from(EYES_TIER) << 16;
            }
        }
    }
    if let Some((model, texture, x_offset)) = energy_swirl(m, s.age)
        && let Some(part) = m.kind.layer(model)
        && let Some([u0, v0, u1, v1]) = atlas(texture)
    {
        layer_poses.reset(part);
        anim::setup(m, &s, &mut layer_poses, &data.animations);
        let (du, dv) = (x_offset % 1.0, s.age * 0.01 % 1.0);
        let start = out.len();
        draw(
            part,
            base,
            &layer_poses,
            [du, dv, 1.0 + du, 1.0 + dv],
            crate::breaking::stage_light([u1 - u0, v1 - v0]),
            crate::breaking::stage_light([u0, v0]),
            crate::gfx::ENERGY_SWIRL_OVERLAY,
            out,
        );
        for vertex in &mut out[start..] {
            vertex.normal = [0; 4];
        }
    }
    if shell_last {
        draw_body(out, &mut skin);
    }
}
/// `EnergySwirlLayer`'s model, texture and `xOffset(t)` for a charged creeper
/// (`CreeperPowerLayer`) or a powered wither (`WitherArmorLayer`); `t` is `ageInTicks`.
fn energy_swirl(m: &Mob, t: f32) -> Option<(&'static str, &'static str, f32)> {
    match m.species() {
        Species::Creeper if m.extra.charged => {
            Some(("creeper#armor", "entity/creeper/creeper_armor", t * 0.01))
        }
        Species::Wither if super::wither::powered(m) => {
            Some(("wither#armor", "entity/wither/wither_armor", (t * 0.02).mcos() * 3.0))
        }
        _ => None,
    }
}
/// The equipment slot of a `HumanoidArmorLayer` model (`zombie#helmet`).
fn armor_slot(model: &str) -> Option<usize> {
    Some(
        match model.rsplit('#').next()? {
            "helmet" => super::gear::HEAD,
            "chestplate" => super::gear::CHEST,
            "leggings" => super::gear::LEGS,
            "boots" => super::gear::FEET,
            _ => return None,
        },
    )
}
/// Render with a fixed texture rect (the earlier API).
pub fn render_single(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    uv: [f32; 4],
    light: u32,
    out: &mut Vec<Vertex>,
) {
    let base = base_transform(m, alpha, camera);
    let overlay = body_overlay(m, alpha);
    let s = anim::state(m, alpha);
    let model = m.kind.model.clone();
    let mut poses = Poses::new(&model);
    anim::setup(m, &s, &mut poses, &std::collections::HashMap::new());
    draw(&model, base, &poses, uv, 0xFFFF_FFFF, light, overlay, out);
}
/// The transform of a named part (ModelPart.translateAndRotate down the tree).
pub fn part_matrix(
    part: &Part,
    parent: glam::Mat4,
    poses: &Poses,
    name: &str,
    x_offset: f32,
) -> Option<glam::Mat4> {
    let mut p = poses.pose(part);
    if part.name == name {
        p[0] += x_offset;
    }
    let m = parent
        * glam::Mat4::from_translation(glam::Vec3::new(p[0], p[1], p[2]) / 16.0)
        * glam::Mat4::from_quat(
            glam::Quat::from_euler(glam::EulerRot::ZYX, p[5], p[4], p[3]),
        ) * glam::Mat4::from_scale(glam::Vec3::new(p[6], p[7], p[8]));
    if part.name == name {
        return Some(m);
    }
    part.children.iter().find_map(|c| part_matrix(c, m, poses, name, x_offset))
}
/// The first part named `name`, depth first.
pub(super) fn find_part<'a>(part: &'a Part, name: &str) -> Option<&'a Part> {
    if part.name == name {
        return Some(part);
    }
    part.children.iter().find_map(|c| find_part(c, name))
}
/// The matrix up to a part that declares an item anchor (`attachments` in the pack's file,
/// EMF's `EMFAttachment`): the walk the positioner takes — every part on the way applies its
/// pose, like `part_matrix` — then the anchor's own offset, in model pixels.
fn attachment_matrix(
    part: &Part,
    parent: glam::Mat4,
    poses: &Poses,
    anchors: &[(String, String, [f32; 3])],
    key: &str,
) -> Option<glam::Mat4> {
    let p = poses.pose(part);
    let m = parent
        * glam::Mat4::from_translation(glam::Vec3::new(p[0], p[1], p[2]) / 16.0)
        * glam::Mat4::from_quat(
            glam::Quat::from_euler(glam::EulerRot::ZYX, p[5], p[4], p[3]),
        ) * glam::Mat4::from_scale(glam::Vec3::new(p[6], p[7], p[8]));
    if let Some((_, _, [x, y, z])) = anchors
        .iter()
        .find(|(bone, kind, _)| *kind == key && part.name == *bone)
    {
        return Some(
            m * glam::Mat4::from_translation(glam::Vec3::new(*x, *y, *z) / 16.0),
        );
    }
    part.children.iter().find_map(|c| attachment_matrix(c, m, poses, anchors, key))
}
/// `AllayModel.translateToHand`: the root and body, then the right arm's
/// pitch, a 0.7 scale and a shift. Both hands use the right arm's pitch.
fn allay_hand(model: &Part, base: glam::Mat4, poses: &Poses) -> Option<glam::Mat4> {
    let body = part_matrix(model, base, poses, "body", 0.0)?;
    let x_rot = poses.pose(find_part(model, "right_arm")?)[3];
    Some(
        body * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0625, 0.1875))
            * glam::Mat4::from_rotation_x(x_rot)
            * glam::Mat4::from_scale(glam::Vec3::splat(0.7))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0625, 0.0, 0.0)),
    )
}
/// The hand of a mob in model space, up to the item's own transform
/// (`ItemInHandLayer.submitArmWithItem`).
fn hand_base(
    m: &Mob,
    model: &Part,
    base: glam::Mat4,
    poses: &Poses,
    anchors: &[(String, String, [f32; 3])],
    left: bool,
) -> Option<glam::Mat4> {
    if let Some(t) = attachment_matrix(
        model,
        base,
        poses,
        anchors,
        if left { "left_handheld_item" } else { "right_handheld_item" },
    ) {
        return Some(t);
    }
    let skeleton = matches!(
        m.species(), Species::Skeleton | Species::Stray | Species::Bogged |
        Species::Parched | Species::WitherSkeleton
    );
    let arm = if left { "left_arm" } else { "right_arm" };
    let offset = if skeleton {
        if left { -1.0 } else { 1.0 }
    } else if avatar(m) && m.slim {
        if left { -0.5 } else { 0.5 }
    } else {
        0.0
    };
    let mut t = if m.species() == Species::Allay {
        allay_hand(model, base, poses)?
    } else {
        part_matrix(model, base, poses, arm, offset)?
    };
    if m.species() == Species::Vex {
        t *= glam::Mat4::from_scale(glam::Vec3::splat(0.55));
        t
            *= glam::Mat4::from_translation(
                glam::Vec3::new(
                    if left { -0.046_875 } else { 0.046_875 },
                    -0.156_25,
                    0.078_125,
                ),
            );
    }
    if m.species() == Species::CopperGolem {
        if m.extra.variant3 == super::copper_golem::IDLE {
            t
                *= glam::Mat4::from_rotation_y(
                    (if left { 90.0f32 } else { -90.0 }).to_radians(),
                ) * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 0.125));
        } else {
            t
                *= glam::Mat4::from_scale(glam::Vec3::splat(0.55))
                    * glam::Mat4::from_translation(
                        glam::Vec3::new(-0.125, 0.3125, -0.1875),
                    );
        }
    }
    let (ox, oy, oz) = if m.is_baby() { (0.0, 1.0, -4.5) } else { (1.0, 2.0, -10.0) };
    t
        *= glam::Mat4::from_rotation_x((-90.0f32).to_radians())
            * glam::Mat4::from_rotation_y(PI);
    t
        *= glam::Mat4::from_translation(
            glam::Vec3::new(if left { -ox } else { ox } / 16.0, oy / 16.0, oz / 16.0),
        );
    Some(t)
}
/// The transform of one of a mob's model parts, in camera space
/// (`ModelPart.translateAndRotate` down the tree).
pub fn part_transform(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    data: &MobData,
    part: &str,
) -> Option<glam::Mat4> {
    let base = base_transform(m, alpha, camera);
    let (model, poses, _) = posed_body(m, alpha, data);
    part_matrix(&model, base, &poses, part, 0.0)
}
/// `PlayerItemInHandLayer.renderItemHeldToEye` up to the item: the root and
/// the head with its pitch clamped to -30 to 90 degrees, `translateToHead`
/// (`CustomHeadLayer.translateToHead`), then the hand's side of the eye.
pub fn held_to_eye_matrix(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    data: &MobData,
    left: bool,
) -> Option<glam::Mat4> {
    use glam::{Mat4, Vec3};
    let base = base_transform(m, alpha, camera);
    let (model, mut poses, _) = posed_body(m, alpha, data);
    let pitch = poses
        .get("head")[3]
        .clamp(-std::f32::consts::FRAC_PI_6, std::f32::consts::FRAC_PI_2);
    poses.xrot("head", pitch);
    let head = part_matrix(&model, base, &poses, "head", 0.0)?;
    let side = if left { -2.5 / 16.0 } else { 2.5 / 16.0 };
    Some(
        head * Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0))
            * Mat4::from_rotation_y(std::f32::consts::PI)
            * Mat4::from_scale(Vec3::new(0.625, -0.625, -0.625))
            * Mat4::from_translation(Vec3::new(side, -0.0625, 0.0)),
    )
}
/// `FoxHeldItemLayer`: the item sits at the head's position under the render
/// state's head roll, yaw and pitch (not the posed head's, which differ while
/// it sleeps), pushed out to the mouth.
fn fox_item_transform(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    data: &MobData,
) -> glam::Mat4 {
    use glam::{Mat4, Vec3};
    let s = anim::state(m, alpha);
    let (model, mut poses, anchors) = posed_body(m, alpha, data);
    let head = *poses.get("head");
    let fox = super::fox::pose(m, alpha);
    let baby = m.is_baby();
    let mut t = base_transform(m, alpha, camera)
        * Mat4::from_translation(Vec3::new(head[0], head[1], head[2]) / 16.0);
    if baby {
        t *= Mat4::from_scale(Vec3::splat(0.75));
    }
    t
        *= Mat4::from_rotation_z(fox.head_roll_angle)
            * Mat4::from_rotation_y(s.y_rot.to_radians())
            * Mat4::from_rotation_x(s.x_rot.to_radians());
    if let Some(anchor) = attachment_matrix(&model, t, &poses, &anchors, "fox_item") {
        t = anchor;
    } else {
        let (x, y, z) = match (baby, fox.sleeping) {
            (true, true) => (0.4, 0.26, 0.15),
            (true, false) => (0.06, 0.26, -0.5),
            (false, true) => (0.46, 0.26, 0.22),
            (false, false) => (0.06, 0.27, -0.5),
        };
        t *= Mat4::from_translation(Vec3::new(x, y, z));
    }
    t *= Mat4::from_rotation_x(90.0f32.to_radians());
    if fox.sleeping {
        t *= Mat4::from_rotation_z(90.0f32.to_radians());
    }
    t
}
/// `CrossedArmsItemLayer.applyTranslation`: `translateToArms`, then the item's
/// placement over the folded arms.
fn crossed_arms_matrix(
    model: &Part,
    base: glam::Mat4,
    poses: &Poses,
) -> Option<glam::Mat4> {
    let arms = part_matrix(model, base, poses, "arms", 0.0)?;
    Some(
        arms * glam::Mat4::from_rotation_x(0.75)
            * glam::Mat4::from_scale(glam::Vec3::splat(1.07))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.13, -0.34))
            * glam::Mat4::from_rotation_x(std::f32::consts::PI),
    )
}
/// The pose a mob's held item is submitted under (`ItemInHandLayer`:
/// `translateToHand`, then the hand offset; the item model adds its own
/// display transform), or `None` when the mob shows no held items.
pub fn held_item_transform(
    m: &Mob,
    alpha: f32,
    camera: glam::DVec3,
    data: &MobData,
    left: bool,
) -> Option<glam::Mat4> {
    if m.species() == Species::Panda {
        return super::panda::item_offset(m, alpha)
            .filter(|_| !left)
            .map(|offset| base_transform(m, alpha, camera) * offset);
    }
    if m.species() == Species::Fox {
        return (!left).then(|| fox_item_transform(m, alpha, camera, data));
    }
    if m.species() == Species::Dolphin {
        return (!left)
            .then(|| {
                let pitch = anim::state(m, alpha).x_rot;
                let percent = pitch.abs() / 60.0;
                let offset = if pitch < 0.0 {
                    glam::Vec3::new(0.0, 1.0 - percent * 0.5, -1.0 + percent * 0.5)
                } else {
                    glam::Vec3::new(0.0, 1.0 + percent * 0.8, -1.0 + percent * 0.2)
                };
                base_transform(m, alpha, camera) * glam::Mat4::from_translation(offset)
            });
    }
    if !super::illager::shows_item(m) {
        return None;
    }
    let base = base_transform(m, alpha, camera);
    let s = anim::state(m, alpha);
    let (model, poses, anchors) = posed_body(m, alpha, data);
    if m.species() == Species::Witch {
        let item = m.extra.main_hand.as_deref()?;
        if left != m.main_arm_left() {
            return None;
        }
        if super::witch::holds_potion(item) {
            let nose = part_matrix(&model, base, &poses, "nose", 0.0)?;
            return Some(
                nose * glam::Mat4::from_translation(glam::Vec3::new(0.0625, 0.25, 0.0))
                    * glam::Mat4::from_rotation_z(180.0f32.to_radians())
                    * glam::Mat4::from_rotation_x(140.0f32.to_radians())
                    * glam::Mat4::from_rotation_z(10.0f32.to_radians())
                    * glam::Mat4::from_rotation_x(180.0f32.to_radians()),
            );
        }
        return crossed_arms_matrix(&model, base, &poses);
    }
    if matches!(m.species(), Species::Villager | Species::WanderingTrader) {
        m.extra.main_hand.as_deref()?;
        if left != m.main_arm_left() {
            return None;
        }
        return crossed_arms_matrix(&model, base, &poses);
    }
    let mut t = hand_base(m, &model, base, &poses, &anchors, left)?;
    let swing_left = m.extra.swing_right_arm.map_or(m.main_arm_left(), |r| !r);
    if m.extra.swing_stab && swing_left == left {
        let forward = m
            .extra
            .main_hand
            .as_deref()
            .and_then(crate::spear::Kinetic::of)
            .map_or(0.0, |k| k.forward_movement);
        t *= crate::spear::third_person_attack_item(s.swing, forward);
    }
    let off_hand = left != m.main_arm_left();
    if super::spear_use::is_using_item(m) && m.extra.using_off_hand == off_hand
        && let Some(kinetic) = (if off_hand {
            &m.extra.off_hand
        } else {
            &m.extra.main_hand
        })
            .as_deref()
            .and_then(crate::spear::Kinetic::of)
    {
        let time_held = m.extra.using_item as f32 + s.partial;
        t
            *= crate::spear::third_person_use_item(
                &kinetic,
                time_held,
                s.swing,
                0.0,
                !left,
            );
    }
    Some(t)
}
/// `ArmedEntityRenderState.extractArmedEntityRenderState`: each arm's item
/// model reads the stack held by that arm. Only the main hand currently
/// retains a full stack; the off hand must not inherit its item or components.
pub fn held_item_stack(
    extra: &super::Extra,
    off_hand: bool,
) -> Option<crate::items::ItemStack> {
    let item = if off_hand {
        extra.off_hand.as_ref()?
    } else {
        if let Some(stack) = extra.main_hand_stack() {
            return Some(stack.clone());
        }
        extra.main_hand.as_ref()?
    };
    Some(crate::items::ItemStack::new(item, 1))
}
#[cfg(test)]
mod tests {
    use super::*;
    /// A baby zombie's armour reads the `humanoid_baby` textures for every
    /// slot, not the adult `humanoid`/`humanoid_leggings` ones (#7302).
    #[test]
    fn a_baby_zombies_armour_uses_the_humanoid_baby_textures() {
        let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
        kind.species = Species::Zombie;
        let model = kind.model.clone();
        kind.layers.insert("zombie_baby#main".into(), model);
        let mut mob = Mob::new(
            1,
            std::sync::Arc::new(kind),
            horizon_core::geometry::Vec3::ZERO,
            0.0,
        );
        mob.extra.age = -24000;
        mob.extra.head = Some("minecraft:iron_helmet".into());
        mob.extra.legs = Some("minecraft:iron_leggings".into());
        mob.extra.feet = Some("minecraft:leather_boots".into());
        let mut layers = Vec::new();
        humanoid_armor(&mob, &mut layers);
        let textures: Vec<&str> = layers.iter().map(|l| &*l.texture).collect();
        assert_eq!(
            textures, ["entity/equipment/humanoid_baby/iron",
            "entity/equipment/humanoid_baby/iron",
            "entity/equipment/humanoid_baby/leather",
            "entity/equipment/humanoid_baby/leather_overlay",]
        );
        mob.extra.age = 0;
        let mut adult = Vec::new();
        humanoid_armor(&mob, &mut adult);
        assert_eq!(&* adult[1].texture, "entity/equipment/humanoid_leggings/iron");
    }
    #[test]
    fn creeper_swelling_uses_local_fuse_and_keeps_java_zero_division() {
        let mut mob = Mob::new(
            1,
            crate::mobs::sensor_scan_tests::kind(),
            horizon_core::geometry::Vec3::ZERO,
            0.0,
        );
        mob.previous_swell = 19;
        mob.swell = 20;
        assert_eq!(creeper_swelling(& mob, 0.5), 19.5 / 28.0);
        mob.max_swell = 40;
        assert_eq!(creeper_swelling(& mob, 0.5), 19.5 / 38.0);
        mob.max_swell = 2;
        assert!(creeper_swelling(& mob, 0.5).is_infinite());
        mob.previous_swell = 0;
        mob.swell = 0;
        assert!(creeper_swelling(& mob, 0.5).is_nan());
    }
    #[test]
    fn offhand_does_not_inherit_mainhand_components() {
        use crate::items::ItemStack;
        let mut extra = super::super::Extra::default();
        let mut main = ItemStack::new("minecraft:crossbow", 1);
        main.data_mut().custom_name = Some(
            horizon_world::nbt::Tag::String("Main hand".into()),
        );
        extra.set_main_hand_stack(Some(main));
        extra.off_hand = Some("minecraft:crossbow".into());
        assert_eq!(
            held_item_stack(& extra, false).unwrap().custom_name().as_deref(),
            Some("Main hand")
        );
        assert!(held_item_stack(& extra, true).unwrap().custom_name().is_none());
        extra.off_hand = Some("minecraft:gold_ingot".into());
        assert_eq!(
            &* held_item_stack(& extra, true).unwrap().item, "minecraft:gold_ingot"
        );
        assert_eq!(
            &* held_item_stack(& extra, false).unwrap().item, "minecraft:crossbow"
        );
        extra.off_hand = None;
        assert!(held_item_stack(& extra, true).is_none());
    }
    /// The elytra's flight yaw (`AvatarRenderer.extractFlightData`) lerps
    /// the velocity of the previous tick towards this tick's, so the model
    /// does not step once per tick while the view turns (#4104).
    #[test]
    fn the_elytra_flight_yaw_lerps_the_velocity_between_ticks() {
        use crate::mobs::{Anim, Brain, Flags, MobType, Part};
        use horizon_core::geometry::Vec3;
        use serde_json::json;
        use std::sync::Arc;
        let model = Arc::new(
            Part::parse("root", &json!({ "children" : { "head" : {} } })),
        );
        let kind = Arc::new(MobType {
            id: "minecraft:player".into(),
            species: Species::Zombie,
            width: 0.6,
            height: 1.8,
            eye_height: 1.62,
            category: "misc".into(),
            max_health: 20.0,
            speed: 0.1,
            flying_speed: 0.02,
            attack_damage: 1.0,
            attack_knockback: 0.0,
            follow_range: 0.0,
            step_height: 0.6,
            armor: 0.0,
            armor_toughness: 0.0,
            knockback_resistance: 0.0,
            jump_strength: 0.42,
            attributes: Arc::default(),
            item_stack_limits: Default::default(),
            gravity: 0.08,
            safe_fall_distance: 3.0,
            fall_damage_multiplier: 1.0,
            water_efficiency: 0.0,
            water_slowdown: 0.8,
            air_speed: 0.02,
            baby_scale: 0.5,
            head_rot_speed: 10.0,
            max_head_x_rot: 40.0,
            max_head_y_rot: 75.0,
            ambient_interval: 80,
            flags: Flags::default(),
            model,
            layers: Default::default(),
            texture: String::new(),
            brain: Brain::Passive,
            anim: Anim::Camel,
            sounds: [""; 3],
            step_sound: None,
            food: Arc::default(),
            tempt: Arc::default(),
        });
        let gliding = |previous: Vec3, current: Vec3| {
            let mut m = Mob::new(1, kind.clone(), Vec3::ZERO, 0.0);
            m.extra.fall_flying = true;
            m.extra.fall_fly_ticks = 20;
            m.extra.previous_velocity = previous;
            m.velocity = current;
            m
        };
        let from = Vec3::new(0.4, 0.0, 0.4);
        let to = Vec3::new(0.0, 0.0, 0.5);
        let at = |m: &Mob, alpha: f32| base_transform_at(m, alpha, glam::Mat4::IDENTITY);
        let differs = |a: glam::Mat4, b: glam::Mat4| {
            a.to_cols_array()
                .iter()
                .zip(b.to_cols_array())
                .any(|(x, y)| (x - y).abs() > 1.0e-4)
        };
        let m = gliding(from, to);
        assert!(! differs(at(& m, 0.0), at(& gliding(from, from), 0.0)));
        assert!(! differs(at(& m, 1.0), at(& gliding(to, to), 1.0)));
        let mid = Vec3::new(0.2, 0.0, 0.45);
        let half = at(&m, 0.5);
        assert!(! differs(half, at(& gliding(mid, mid), 0.5)));
        assert!(differs(half, at(& gliding(to, to), 0.5)));
        assert!(differs(half, at(& gliding(from, from), 0.5)));
    }
    /// Six faces whose corners span `min` to `max`.
    fn cube(min: [f32; 3], max: [f32; 3]) -> Vec<super::super::Polygon> {
        let corner = |p: [f32; 3]| [p[0], p[1], p[2], 0.0, 0.0];
        (0..6)
            .map(|_| super::super::Polygon {
                normal: [0.0; 3],
                vertices: [corner(min), corner(max), corner(min), corner(max)],
                cull_back: false,
            })
            .collect()
    }
    /// `ModelPart.cubes`: the corners of each cube of a part, six faces apiece.
    #[test]
    fn cube_bounds_follow_the_faces_of_each_cube() {
        let mut polygons = cube([-4.0, -8.0, -2.0], [4.0, 0.0, 2.0]);
        polygons.extend(cube([0.0, 0.0, 0.0], [1.0, 2.0, 3.0]));
        let part = Part {
            name: "body".into(),
            pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            polygons,
            children: Vec::new(),
        };
        assert_eq!(
            cube_bounds(& part), vec![[- 4.0, - 8.0, - 2.0, 4.0, 0.0, 2.0], [0.0, 0.0,
            0.0, 1.0, 2.0, 3.0]]
        );
    }
    /// `StuckInBodyLayer.snapToFace`.
    #[test]
    fn a_stuck_item_snaps_to_the_far_or_the_middle_face() {
        assert_eq!(snap_to_face(0.75), 1.0);
        assert_eq!(snap_to_face(0.5), 0.5);
        assert_eq!(snap_to_face(0.1), 0.5);
    }
    #[test]
    fn shoulder_parrot_textures_follow_the_variant() {
        assert_eq!(shoulder_parrot_texture(0), "entity/parrot/parrot_red_blue");
        assert_eq!(shoulder_parrot_texture(4), "entity/parrot/parrot_grey");
        assert_eq!(shoulder_parrot_texture(5), "entity/parrot/parrot_grey");
        assert_eq!(shoulder_parrot_texture(- 1), "entity/parrot/parrot_red_blue");
    }
    /// VillagerProfessionLayer's textures (type, or baby for a baby;
    /// profession; level) of every villager and zombie villager are in the
    /// prepared resources: a missing one draws nothing.
    #[test]
    fn villager_layers_name_textures_that_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        for kind in ["minecraft:villager", "minecraft:zombie_villager"] {
            let kind = std::sync::Arc::new(types.types[kind].clone());
            for baby in [false, true] {
                for t in 0..VILLAGER_TYPES.len() {
                    for p in 0..PROFESSIONS.len() {
                        let mut m = Mob::new(
                            1,
                            kind.clone(),
                            horizon_core::geometry::Vec3::ZERO,
                            0.0,
                        );
                        m.extra.age = if baby { -24000 } else { 0 };
                        (m.extra.variant, m.extra.variant2, m.extra.variant3) = (
                            t as u8,
                            p as u8,
                            5,
                        );
                        let layers = layers(&m, m.age as f32);
                        let group = if baby { "/baby/" } else { "/type/" };
                        assert!(
                            layers.iter().any(| l | l.texture.contains(group)),
                            "{kind:?} baby={baby}: no {group} layer"
                        );
                        for layer in layers {
                            let png = root
                                .join(
                                    format!("assets/minecraft/textures/{}.png", layer.texture),
                                );
                            assert!(png.is_file(), "missing {}", png.display());
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn sulfur_cube_opaque_core_is_submitted_before_its_translucent_shell() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let mut types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(
            types.types.remove("minecraft:sulfur_cube").unwrap(),
        );
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |name: &str| {
            Some(
                if name.contains("_inner") {
                    [0.0, 0.0, 0.25, 0.25]
                } else {
                    [0.5, 0.5, 0.75, 0.75]
                },
            )
        };
        for baby in [false, true] {
            let mut mob = Mob::new(
                1,
                kind.clone(),
                horizon_core::geometry::Vec3::ZERO,
                0.0,
            );
            mob.extra.age = if baby { -24000 } else { 0 };
            super::super::sulfur_cube::finalize(&mut mob);
            assert_eq!(mob.is_baby(), baby);
            for posed in [false, true] {
                let mut vertices = Vec::new();
                if posed {
                    render_posed(
                        &mob,
                        0.5,
                        glam::Mat4::IDENTITY,
                        &atlas,
                        0xf0f0,
                        &data,
                        &mut vertices,
                    );
                } else {
                    render(
                        &mob,
                        0.5,
                        glam::DVec3::ZERO,
                        &atlas,
                        0xf0f0,
                        &data,
                        &mut vertices,
                    );
                }
                assert_eq!(
                    vertices.len(), 48, "both real six-face models must be drawn"
                );
                assert!(
                    vertices[..24].iter().all(| v | v.uv[0] <= 0.25),
                    "opaque core must precede shell"
                );
                assert!(vertices[24..].iter().all(| v | v.uv[0] >= 0.5));
            }
            mob.extra.body = Some("minecraft:stone".into());
            let mut vertices = Vec::new();
            render(&mob, 0.5, glam::DVec3::ZERO, &atlas, 0xf0f0, &data, &mut vertices);
            assert_eq!(
                vertices.len(), 24, "a swallowed block still hides the default core"
            );
            assert!(vertices.iter().all(| v | v.uv[0] >= 0.5));
        }
    }
    /// CopperGolemRenderer: the emissive eye layer draws the eye texture on
    /// the body's model, every polygon of it, at each oxidation level. The
    /// prepared `copper_golem#eyes` model has no polygons, so the eyes
    /// showed the body texture's black sockets.
    #[test]
    fn copper_golem_eyes_are_drawn_on_the_body_model() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(types.types["minecraft:copper_golem"].clone());
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |name: &str| {
            Some(
                if name.contains("copper_golem_eyes") {
                    [0.5, 0.5, 0.75, 0.75]
                } else {
                    [0.0, 0.0, 0.25, 0.25]
                },
            )
        };
        for variant in 0..4 {
            let mut mob = Mob::new(
                1,
                kind.clone(),
                horizon_core::geometry::Vec3::ZERO,
                0.0,
            );
            mob.extra.variant = variant;
            let mut vertices = Vec::new();
            render(&mob, 0.5, glam::DVec3::ZERO, &atlas, 0xf0f0, &data, &mut vertices);
            let body = vertices.iter().filter(|v| v.uv[0] <= 0.25).count();
            let eyes = vertices.iter().filter(|v| v.uv[0] >= 0.5).count();
            assert!(body > 0, "variant {variant}: no body drawn");
            assert_eq!(eyes, body, "variant {variant}: the eye layer covers the body");
        }
    }
    /// `VexModel.translateToHand`: the held item is drawn at 0.55 scale and
    /// moved by the vex's own offset, so the vex's sword is the tiny one
    /// vanilla shows. It was drawn at the full size, like a zombie's.
    #[test]
    fn the_vex_holds_its_item_at_the_small_scale() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(types.types["minecraft:vex"].clone());
        let mob = Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        for left in [false, true] {
            let t = held_item_transform(&mob, 0.5, glam::DVec3::ZERO, &data, left)
                .unwrap();
            for axis in [t.x_axis, t.y_axis, t.z_axis] {
                let scale = axis.truncate().length();
                assert!(
                    (scale - 0.55).abs() < 1.0e-5,
                    "the vex's item is drawn at 0.55 scale, got {scale}"
                );
            }
        }
    }
    /// `CrossedArmsItemLayer`: a villager and a wandering trader show their
    /// main hand's item over the folded arms, and nothing in the off hand.
    #[test]
    fn villagers_and_traders_hold_items_in_crossed_arms() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let assets = crate::assets::Assets::load(&root).unwrap();
        let facts = crate::blocks::BlockFacts::load(&root, assets.registry).unwrap();
        let data = MobData::load(&root, &facts);
        for id in ["minecraft:villager", "minecraft:wandering_trader"] {
            let kind = std::sync::Arc::new(types.types[id].clone());
            let mut mob = Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
            let main = mob.main_arm_left();
            assert!(
                held_item_transform(& mob, 1.0, glam::DVec3::ZERO, & data, main)
                .is_none()
            );
            mob.extra.set_main_hand(Some("minecraft:emerald".into()));
            assert!(
                held_item_transform(& mob, 1.0, glam::DVec3::ZERO, & data, main)
                .is_some()
            );
            assert!(
                held_item_transform(& mob, 1.0, glam::DVec3::ZERO, & data, ! main)
                .is_none()
            );
        }
    }
    /// PhantomRenderer.scale moves the model down into its 0.5-high box:
    /// the model's origin ends 1.501 - 1.3125 blocks above the feet (times
    /// the size's scale), not 1.501 like other mobs (#3008).
    #[test]
    fn the_phantom_model_sits_in_its_box() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(types.types["minecraft:phantom"].clone());
        let mut mob = Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
        for (size, scale) in [(0, 1.0f32), (5, 1.75)] {
            mob.extra.size = size;
            let origin = base_transform(&mob, 1.0, glam::DVec3::ZERO)
                .transform_point3(glam::Vec3::ZERO);
            let y = (1.501 - 1.3125) * scale;
            assert!((origin.y - y).abs() < 1.0e-4, "size {size}: origin {origin}");
            assert!(
                (origin.z.abs() - 0.1875 * scale).abs() < 1.0e-4,
                "size {size}: origin {origin}"
            );
        }
    }
    /// `EquipmentLayerRenderer.renderLayers`: the extra armour layers (a
    /// player's) take their own overlay, `NO_OVERLAY` or the armour glint,
    /// never the hurt tint of the body, and the glint samples the model UV.
    #[test]
    fn extra_armour_layers_carry_their_glint_not_the_hurt_tint() {
        use crate::gfx::{
            ENERGY_SWIRL_OVERLAY, SHADOW_OVERLAY, TEXT_POLYGON_OFFSET_OVERLAY,
            TRANSLUCENT_GLINT_ITEM_OVERLAY, TRANSLUCENT_ITEM_OVERLAY,
            TRANSLUCENT_PARTICLE_OVERLAY, WATER_MASK_OVERLAY,
        };
        assert!(
            ! [WATER_MASK_OVERLAY, SHADOW_OVERLAY, TRANSLUCENT_PARTICLE_OVERLAY,
            TRANSLUCENT_ITEM_OVERLAY, TRANSLUCENT_GLINT_ITEM_OVERLAY,
            ENERGY_SWIRL_OVERLAY, TEXT_POLYGON_OFFSET_OVERLAY,].contains(& ARMOR_GLINT)
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(types.types["minecraft:zombie"].clone());
        let mut mob = Mob::new(1, kind.clone(), horizon_core::geometry::Vec3::ZERO, 0.0);
        mob.hurt_time = 5;
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |_: &str| Some([0.0, 0.0, 0.25, 0.25]);
        let part = &kind.model;
        for overlay in [ARMOR_GLINT, 0] {
            let mut vertices = Vec::new();
            render_with(
                &mob,
                0.5,
                glam::DVec3::ZERO,
                &atlas,
                0xf0f0,
                &data,
                &[(part, [0.5, 0.5, 0.75, 0.75], 0xFFFF_FFFF, overlay, false)],
                &mut vertices,
            );
            let (armour, body): (Vec<&Vertex>, Vec<&Vertex>) = vertices
                .iter()
                .partition(|v| v.uv[0] >= 0.5);
            assert!(! armour.is_empty() && ! body.is_empty());
            let hurt = body_overlay(&mob, 0.5);
            assert_ne!(hurt, 0);
            assert!(body.iter().all(| v | v.layer == hurt));
            assert!(armour.iter().all(| v | v.layer == overlay));
            for v in &armour {
                if overlay == ARMOR_GLINT {
                    assert!((v.glint_uv[0] - (v.uv[0] - 0.5) * 4.0).abs() < 1e-4);
                    assert!((v.glint_uv[1] - (v.uv[1] - 0.5) * 4.0).abs() < 1e-4);
                } else {
                    assert_eq!(v.glint_uv, [0.0, 0.0]);
                }
            }
        }
    }
    #[test]
    fn a_dyed_leather_armour_keeps_its_colour() {
        let mut stack = crate::items::ItemStack {
            item: "minecraft:leather_helmet".into(),
            count: 1,
            damage: 0,
            extra: None,
        };
        assert_eq!(armor_tint(Some(0xA06540), Some(& stack)), abgr(0xA06540));
        stack.extra = Some(
            Box::new(crate::item_data::StackData {
                dyed_color: Some(0x0000FF),
                ..Default::default()
            }),
        );
        assert_eq!(armor_tint(Some(0xA06540), Some(& stack)), abgr(0x0000FF));
        assert_eq!(armor_tint(None, Some(& stack)), 0xFFFF_FFFF);
    }
    /// `ColorLerper.getLerpedColor(Type.SHEEP, ageInTicks)`: the points the
    /// 26.3 jar gave — 0, half a step, one whole step, a step boundary, the
    /// last step back into white, and the 400-tick (16-step) wrap.
    #[test]
    fn jeb_wool_lerps_through_the_sixteen_dye_colours() {
        assert_eq!(jeb_wool_rgb(0.0), 0xE6E6E6);
        assert_eq!(jeb_wool_rgb(12.5), 0xD0A37D);
        assert_eq!(jeb_wool_rgb(25.0), 0xBA6015);
        assert_eq!(jeb_wool_rgb(25.0), sheep_wool_rgb(1));
        assert_eq!(jeb_wool_rgb(50.0), 0x953A8D);
        assert_eq!(jeb_wool_rgb(399.0), 0xDDDDDD);
        assert_eq!(jeb_wool_rgb(400.0), 0xE6E6E6);
    }
    fn sheep_layer(m: &Mob, model: &str, age: f32) -> Option<Layer> {
        layers(m, age).into_iter().find(|l| l.model == model)
    }
    /// SheepRenderer.extractRenderState's `jeb_` magic name: the wool layer
    /// takes the lerped colour and cycles with the age, a white `jeb_` sheep
    /// keeps its undercoat (SheepWoolUndercoatLayer draws for it too), and
    /// the dye colour, the baby model and the shears still win.
    #[test]
    fn a_jeb_sheep_cycles_its_wool_colour_and_keeps_the_undercoat() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(types.types["minecraft:sheep"].clone());
        let mut mob = Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
        mob.custom_name = Some("jeb_".into());
        let young = sheep_layer(&mob, "sheep#wool", 0.0)
            .expect("an unsheared sheep draws its wool")
            .color;
        assert_eq!(young, abgr(0xE6E6E6));
        let older = sheep_layer(&mob, "sheep#wool", 100.0)
            .expect("an unsheared sheep draws its wool")
            .color;
        assert_ne!(young, older, "the colour cycles with the age");
        let undercoat = sheep_layer(&mob, "sheep#wool_undercoat", 0.0)
            .expect("a jeb_ sheep keeps its undercoat");
        assert_eq!(undercoat.color, abgr(0xE6E6E6));
        mob.custom_name = Some("jeb".into());
        mob.extra.color = 14;
        let dyed = abgr(sheep_wool_rgb(14));
        assert_eq!(
            sheep_layer(& mob, "sheep#wool", 0.0)
            .expect("an unsheared sheep draws its wool").color, dyed
        );
        assert_eq!(
            sheep_layer(& mob, "sheep#wool_undercoat", 0.0)
            .expect("a dyed sheep keeps its undercoat").color, dyed
        );
        mob.custom_name = Some("jeb_".into());
        mob.extra.color = 0;
        mob.extra.age = -24000;
        assert!(mob.is_baby());
        assert_eq!(
            sheep_layer(& mob, "sheep_baby#wool", 50.0)
            .expect("a baby sheep draws its wool").color, abgr(0x953A8D)
        );
        assert!(sheep_layer(& mob, "sheep#wool_undercoat", 50.0).is_none());
        mob.extra.age = 0;
        mob.extra.sheared = true;
        assert!(sheep_layer(& mob, "sheep#wool", 0.0).is_none());
    }
}
#[cfg(test)]
mod scroll_tests {
    #[test]
    fn a_pufferfish_bobs_by_eight_hundredths() {
        assert!((super::pufferfish_bob_y(0.0) - 0.08).abs() < 1e-6);
        let quarter = std::f32::consts::FRAC_PI_2 / 0.05;
        assert!(super::pufferfish_bob_y(quarter).abs() < 1e-3);
        assert!(super::pufferfish_bob_y(2.0 * quarter) < - 0.0799);
    }
    use super::*;
    #[test]
    fn the_scrolling_texture_wraps_around() {
        assert_eq!(scroll_offset(0.0, BREEZE_WIND_SCROLL), 0.0);
        assert!((scroll_offset(25.0, BREEZE_WIND_SCROLL) - 0.5).abs() < 1e-6);
        assert!((scroll_offset(50.0, BREEZE_WIND_SCROLL) - 0.0).abs() < 1e-6);
        assert!((scroll_offset(50.0, WIND_CHARGE_SCROLL) - 0.5).abs() < 1e-6);
        assert!((scroll_offset(100.0, WIND_CHARGE_SCROLL) - 0.0).abs() < 1e-6);
    }
    #[test]
    fn the_scrolled_u_wraps_inside_the_texture() {
        assert_eq!(scroll_u(0.25, 0.0), 0.25);
        assert_eq!(scroll_u(1.0, 0.0), 1.0);
        assert!((scroll_u(0.25, 0.5) - 0.75).abs() < 1e-6);
        assert!((scroll_u(0.75, 0.5) - 0.25).abs() < 1e-6);
        assert!((scroll_u(1.0, 0.5) - 0.5).abs() < 1e-6);
    }
    /// `SheepWoolLayer.submit` returns early on a sheared sheep, but
    /// `SheepWoolUndercoatLayer.submit` has no isSheared guard: a sheared
    /// dyed sheep keeps its undercoat and loses only its wool.
    #[test]
    fn a_sheared_dyed_sheep_keeps_its_wool_undercoat() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kind = std::sync::Arc::new(types.types["minecraft:sheep"].clone());
        let sheep = |color: u8, sheared: bool, baby: bool| {
            let mut m = Mob::new(
                1,
                kind.clone(),
                horizon_core::geometry::Vec3::ZERO,
                0.0,
            );
            m.extra.color = color;
            m.extra.sheared = sheared;
            m.extra.age = if baby { -24000 } else { 0 };
            m
        };
        let wool = |m: &Mob| -> Vec<(String, String, u32)> {
            layers(m, 0.0)
                .into_iter()
                .filter(|l| l.model.contains("#wool"))
                .map(|l| (l.model.into_owned(), l.texture.into_owned(), l.color))
                .collect()
        };
        let dyed = abgr(sheep_wool_rgb(1));
        let layer = |model: &str, texture: &str| (
            model.to_owned(),
            texture.to_owned(),
            dyed,
        );
        assert_eq!(
            wool(& sheep(1, true, false)), [layer("sheep#wool_undercoat",
            "entity/sheep/sheep_wool_undercoat")]
        );
        assert!(wool(& sheep(0, true, false)).is_empty());
        assert_eq!(
            wool(& sheep(1, false, true)), [layer("sheep_baby#wool",
            "entity/sheep/sheep_wool_baby")]
        );
        assert!(wool(& sheep(1, true, true)).is_empty());
        assert_eq!(
            wool(& sheep(1, false, false)), [layer("sheep#wool_undercoat",
            "entity/sheep/sheep_wool_undercoat"), layer("sheep#wool",
            "entity/sheep/sheep_wool")]
        );
    }
    /// `anim::Poses` keys parts by a hash of their name: within any vanilla model, two
    /// different names never share one (they would share a pose).
    #[test]
    fn no_two_part_names_share_a_key() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        fn names<'a>(p: &'a Part, out: &mut Vec<&'a str>) {
            out.push(&p.name);
            for c in &p.children {
                names(c, out);
            }
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let mut models = 0;
        for kind in types.types.values() {
            for model in std::iter::once(&kind.model).chain(kind.layers.values()) {
                let mut all = Vec::new();
                names(model, &mut all);
                let mut keys = std::collections::HashMap::new();
                for name in all {
                    let other = *keys
                        .entry(super::super::anim::key(name))
                        .or_insert(name);
                    assert_eq!(other, name, "{} shares a key", kind.id);
                }
                models += 1;
            }
        }
        assert!(models > 100);
    }
    /// The CPU cost of posing a mob farm (tools/stress_scenes.py `stress_mobs`): 1000 mobs of
    /// 8 kinds through `render_with`, as `mob_geometry` draws them each frame. Prints the median
    /// thread CPU time per frame (busy machines stretch wall time), the vertices and a hash of them (the same hash before and after a speed-up
    /// means the same picture). `cargo test --profile fast -p horizon-client --lib
    /// benchmark_mob_farm_geometry -- --ignored --nocapture`.
    #[test]
    #[ignore = "manual benchmark; needs the prepared resources"]
    fn benchmark_mob_farm_geometry() {
        use std::{hash::Hasher, hint::black_box, time::Instant};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client/26.3");
        if !root.join("entity-models.json").exists() {
            return;
        }
        let types = super::super::MobTypes::load(&root).unwrap();
        let kinds = [
            "husk",
            "creeper",
            "cow",
            "pig",
            "sheep",
            "chicken",
            "villager",
            "spider",
        ]
            .map(|k| std::sync::Arc::new(
                types.types[&format!("minecraft:{k}")].clone(),
            ));
        let mobs: Vec<Mob> = (0..1000u32)
            .map(|i| {
                let (layer, row, col) = (i / 200, i / 20 % 10, i % 20);
                let at = horizon_core::geometry::Vec3::new(
                    -14.25 + 1.5 * f64::from(col),
                    1.0 + 2.5 * f64::from(layer),
                    8.0 + 2.0 * f64::from(row),
                );
                let mut m = Mob::new(i + 1, kinds[i as usize % 8].clone(), at, 180.0);
                m.previous = at;
                m.age = i;
                m
            })
            .collect();
        let data = MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: Default::default(),
            emissive: Default::default(),
            blinks: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        };
        let atlas = |name: &str| {
            let h = name.len() as f32 / 64.0;
            Some([h, h, h + 0.25, h + 0.25])
        };
        let camera = glam::DVec3::new(0.5, 9.6, 0.5);
        let mut out = Vec::new();
        let frame = |out: &mut Vec<Vertex>| {
            out.clear();
            for m in &mobs {
                render_with(m, 0.5, camera, &atlas, 0xf0f0, &data, &[], out);
            }
        };
        frame(&mut out);
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        hash.write(bytemuck::cast_slice(&out));
        let timed = |f: &mut dyn FnMut()| {
            let (wall, cpu) = (
                Instant::now(),
                horizon_client_render::spans::thread_cpu(),
            );
            f();
            match (horizon_client_render::spans::thread_cpu(), cpu) {
                (Some(end), Some(start)) => (end - start).as_secs_f64() * 1000.0,
                _ => wall.elapsed().as_secs_f64() * 1000.0,
            }
        };
        let mut times = Vec::new();
        for _ in 0..30 {
            times.push(timed(&mut || frame(black_box(&mut out))));
        }
        times.sort_by(f64::total_cmp);
        println!(
            "mob farm: 1000 mobs, {} vertices, {:.3} ms/frame (median of 30, min {:.3}), hash {:016x}",
            out.len(), times[15], times[0], hash.finish()
        );
        let median = |mut f: Box<dyn FnMut() + '_>| {
            let mut t: Vec<f64> = (0..30).map(|_| timed(&mut f)).collect();
            t.sort_by(f64::total_cmp);
            t[15]
        };
        let posed: Vec<(std::sync::Arc<Part>, Poses)> = mobs
            .iter()
            .map(|m| {
                let model = m.kind.model.clone();
                let mut poses = Poses::new(&model);
                anim::setup(m, &anim::state(m, 0.5), &mut poses, &data.animations);
                (model, poses)
            })
            .collect();
        let poses_ms = median(
            Box::new(|| {
                for m in &mobs {
                    let model = m.kind.model.clone();
                    let mut poses = Poses::new(&model);
                    anim::setup(m, &anim::state(m, 0.5), &mut poses, &data.animations);
                    black_box(&poses);
                }
            }),
        );
        let mut body = Vec::new();
        let draw_ms = median(
            Box::new(|| {
                body.clear();
                for (m, (model, poses)) in mobs.iter().zip(&posed) {
                    let base = base_transform(m, 0.5, camera);
                    draw(
                        model,
                        base,
                        poses,
                        [0.0, 0.0, 0.25, 0.25],
                        u32::MAX,
                        0xf0f0,
                        0,
                        &mut body,
                    );
                }
                black_box(&body);
            }),
        );
        println!(
            "  body poses only {poses_ms:.3} ms; body vertices only {draw_ms:.3} ms ({} vertices)",
            body.len()
        );
    }
}
#[cfg(test)]
mod emissive_tests {
    use super::*;
    use crate::mobs::{Polygon, Species};
    use horizon_optifine::{
        blink::Blinks, emissive::Emissive, random_textures::RandomTextures,
    };
    use std::sync::{Arc, RwLock};
    /// A silverfish (drawn with its type's texture) whose model is one quad.
    fn silverfish() -> Mob {
        let mut kind = crate::mobs::sensor_scan_tests::kind();
        let k = Arc::make_mut(&mut kind);
        k.species = Species::Silverfish;
        k.texture = "entity/silverfish".into();
        k.model = Arc::new(Part {
            name: "root".into(),
            pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            polygons: vec![
                Polygon { normal : [0.0, 0.0, - 1.0], cull_back : false, vertices :
                [[0.0, 0.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0, 0.0], [1.0, 1.0, 0.0,
                1.0, 1.0], [0.0, 1.0, 0.0, 0.0, 1.0],], }
            ],
            children: Vec::new(),
        });
        Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0)
    }
    fn data(files: &[&str]) -> MobData {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let root = std::env::temp_dir()
            .join(
                format!(
                    "horizon-render-emissive-{}-{}", std::process::id(), N.fetch_add(1,
                    std::sync::atomic::Ordering::Relaxed)
                ),
            );
        let _ = std::fs::remove_dir_all(&root);
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let text = if file.ends_with(".properties") { "textures.1=2" } else { "" };
            std::fs::write(path, text).unwrap();
        }
        let random = RandomTextures::scan(&root);
        let blinks = Blinks::scan(&root, &random);
        let mut emissive = Emissive::scan(&root, &random);
        emissive.pair_beside(&root, blinks.files());
        let _ = std::fs::remove_dir_all(&root);
        MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: RwLock::new(random),
            emissive: RwLock::new(Arc::new(emissive)),
            blinks: RwLock::new(blinks),
            pois: Default::default(),
            species_placements: Default::default(),
        }
    }
    /// Every sprite the test atlas has, each at its own rect.
    fn atlas(name: &str) -> Option<[f32; 4]> {
        match name {
            "entity/silverfish" => Some([0.0, 0.0, 0.5, 0.5]),
            "entity/silverfish_e" => Some([0.5, 0.5, 1.0, 1.0]),
            "entity/silverfish_blink" => Some([0.0, 0.5, 0.5, 1.0]),
            _ => None,
        }
    }
    #[test]
    fn an_emissive_overlay_draws_the_body_again_at_full_brightness() {
        let lit = (3 * 16) | ((5 * 16) << 8);
        let m = silverfish();
        let draw = |data: &MobData| {
            let mut out = Vec::new();
            render_with(&m, 0.0, glam::DVec3::ZERO, &atlas, lit, data, &[], &mut out);
            out
        };
        let plain = draw(&data(&["assets/minecraft/textures/entity/silverfish.png"]));
        assert_eq!(plain.len(), 4);
        assert!(plain.iter().all(| v | v.light == lit));
        let glowing = draw(
            &data(
                &[
                    "assets/minecraft/textures/entity/silverfish.png",
                    "assets/minecraft/textures/entity/silverfish_e.png",
                ],
            ),
        );
        assert_eq!(glowing.len(), 8);
        let (body, glow) = glowing.split_at(4);
        assert!(body.iter().all(| v | v.light == lit));
        assert!(glow.iter().all(| v | v.light == FULL_BRIGHT));
        assert!(glow.iter().all(| v | v.uv[0] >= 0.5 && v.uv[1] >= 0.5));
        assert!(body.iter().all(| v | v.uv[0] <= 0.5 && v.uv[1] <= 0.5));
        assert_eq!(
            glow.iter().map(| v | v.normal).collect::< Vec < _ >> (), body.iter().map(| v
            | v.normal).collect::< Vec < _ >> ()
        );
    }
    #[test]
    fn a_blink_frame_replaces_the_texture_for_a_few_ticks() {
        let data = data(
            &[
                "assets/minecraft/textures/entity/silverfish.png",
                "assets/minecraft/textures/entity/silverfish_blink.png",
            ],
        );
        let uvs = |age: u32| {
            let mut m = silverfish();
            m.age = age;
            let mut out = Vec::new();
            render_with(&m, 0.0, glam::DVec3::ZERO, &atlas, 0, &data, &[], &mut out);
            out.iter().map(|v| v.uv).collect::<Vec<_>>()
        };
        let shut = |uv: &Vec<[f32; 2]>| uv.iter().all(|uv| uv[0] <= 0.5 && uv[1] >= 0.5);
        let open = |uv: &Vec<[f32; 2]>| uv.iter().all(|uv| uv[0] <= 0.5 && uv[1] <= 0.5);
        assert!(open(& uvs(88)));
        assert!(shut(& uvs(89)));
        assert!(shut(& uvs(93)));
        assert!(open(& uvs(94)));
        assert!(shut(& uvs(89 + 150)));
    }
    #[test]
    fn a_blink_shows_its_own_glow_or_none() {
        let atlas = |name: &str| {
            let u = match name {
                "entity/silverfish" => 0.0,
                "entity/silverfish_e" => 0.25,
                "entity/silverfish_blink" => 0.5,
                "entity/silverfish_blink_e" => 0.75,
                _ => return None,
            };
            Some([u, 0.0, u + 0.25, 1.0])
        };
        let base = [
            "assets/minecraft/textures/entity/silverfish.png",
            "assets/minecraft/textures/entity/silverfish_e.png",
            "assets/minecraft/textures/entity/silverfish_blink.png",
        ];
        let at = |data: &MobData, age: u32| {
            let mut m = silverfish();
            m.age = age;
            drawn(&m, data, &atlas)
        };
        let plain = data(&base);
        let open = at(&plain, 0);
        assert_eq!(open.len(), 8);
        assert!(inside(& open[4..], 0.25), "the texture's glow");
        let shut = at(&plain, 89);
        assert_eq!(shut.len(), 4, "no glowing open eyes over the shut ones");
        assert!(inside(& shut, 0.5));
        let mut own = base.to_vec();
        own.push("assets/minecraft/textures/entity/silverfish_blink_e.png");
        let shut = at(&data(&own), 89);
        assert_eq!(shut.len(), 8);
        assert!(inside(& shut[4..], 0.75), "the frame's own glow");
    }
    /// Draws `m` and returns each vertex's (uv, light).
    fn drawn(m: &Mob, data: &MobData, atlas: Atlas) -> Vec<([f32; 2], u32)> {
        let mut out = Vec::new();
        render_with(m, 0.0, glam::DVec3::ZERO, atlas, 7, data, &[], &mut out);
        out.iter().map(|v| (v.uv, v.light)).collect()
    }
    /// Whether every vertex samples inside the rect starting at `at` (each rect is 0.25 wide).
    fn inside(vertices: &[([f32; 2], u32)], at: f32) -> bool {
        vertices.iter().all(|(uv, _)| (at..=at + 0.25).contains(&uv[0]))
    }
    #[test]
    fn a_layer_glows_with_its_own_overlay() {
        let mut m = silverfish();
        let k = Arc::make_mut(&mut m.kind);
        k.species = Species::Sheep;
        k.layers.insert("sheep#wool".into(), k.model.clone());
        let data = data(
            &[
                "assets/minecraft/textures/entity/sheep/sheep.png",
                "assets/minecraft/textures/entity/sheep/sheep_wool.png",
                "assets/minecraft/textures/entity/sheep/sheep_wool_e.png",
            ],
        );
        let atlas = |name: &str| {
            let u = match name {
                "entity/sheep/sheep" => 0.0,
                "entity/sheep/sheep_wool" => 0.25,
                "entity/sheep/sheep_wool_e" => 0.5,
                _ => return None,
            };
            Some([u, 0.0, u + 0.25, 1.0])
        };
        let v = drawn(&m, &data, &atlas);
        assert_eq!(v.len(), 12, "body, wool, wool's glow");
        assert!(
            inside(& v[..4], 0.0) && inside(& v[4..8], 0.25) && inside(& v[8..], 0.5)
        );
        assert!(v[..8].iter().all(| (_, light) | * light == 7));
        assert!(v[8..].iter().all(| (_, light) | * light == FULL_BRIGHT));
    }
    #[test]
    fn a_random_variant_glows_with_its_own_overlay_only() {
        const RANDOM: &str = "assets/minecraft/optifine/random/entity";
        let atlas = |name: &str| {
            let u = match name {
                "entity/silverfish" => 0.0,
                "entity/silverfish_e" => 0.25,
                "minecraft:optifine/random/entity/silverfish2" => 0.5,
                "minecraft:optifine/random/entity/silverfish2_e" => 0.75,
                _ => return None,
            };
            Some([u, 0.0, u + 0.25, 1.0])
        };
        let seen = || crate::mobs::random_textures::Seen {
            biome: None,
            y: 64,
            blocks: Vec::new(),
            moon_phase: 0,
            day_time: 0,
            weather: horizon_optifine::random::Weather::Clear,
        };
        let props = format!("{RANDOM}/silverfish.properties");
        let variant = format!("{RANDOM}/silverfish2.png");
        let variant_e = format!("{RANDOM}/silverfish2_e.png");
        let base = [
            "assets/minecraft/textures/entity/silverfish.png",
            "assets/minecraft/textures/entity/silverfish_e.png",
        ];
        for own_overlay in [true, false] {
            let mut files = vec![base[0], base[1], props.as_str(), variant.as_str()];
            if own_overlay {
                files.push(variant_e.as_str());
            }
            let data = data(&files);
            let mut m = silverfish();
            {
                let textures = data.random_textures.read().unwrap();
                crate::mobs::random_textures::choose(
                    &mut m,
                    &textures,
                    &data,
                    Some(seen()),
                    0,
                );
            }
            let v = drawn(&m, &data, &atlas);
            assert!(inside(& v[..4], 0.5), "the variant");
            if own_overlay {
                assert_eq!(v.len(), 8);
                assert!(inside(& v[4..], 0.75), "its own overlay, not silverfish_e");
            } else {
                assert_eq!(v.len(), 4, "silverfish_e doesn't glow over silverfish2");
            }
        }
    }
    #[test]
    fn worn_armour_glows_when_its_extra_layer_says_so() {
        let m = silverfish();
        let data = data(&[]);
        let atlas = |_: &str| Some([0.0, 0.0, 0.5, 0.5]);
        let part = m.kind.model.clone();
        let extra = [
            (part.as_ref(), [0.5, 0.0, 1.0, 0.5], 0xFFFF_FFFF, 0, false),
            (part.as_ref(), [0.5, 0.5, 1.0, 1.0], 0xFFFF_FFFF, 0, true),
        ];
        let mut out = Vec::new();
        render_with(&m, 0.0, glam::DVec3::ZERO, &atlas, 7, &data, &extra, &mut out);
        assert_eq!(out.len(), 12, "body, armour, armour's glow");
        assert!(out[..8].iter().all(| v | v.light == 7));
        assert!(out[8..].iter().all(| v | v.light == FULL_BRIGHT));
    }
}
#[cfg(test)]
mod random_texture_tests {
    use super::*;
    use crate::mobs::{Polygon, Species};
    use horizon_optifine::random_textures::RandomTextures;
    use std::sync::{Arc, RwLock};
    /// A silverfish (drawn with its type's texture) whose model is one quad.
    fn silverfish() -> Mob {
        let mut kind = crate::mobs::sensor_scan_tests::kind();
        let k = Arc::make_mut(&mut kind);
        k.species = Species::Silverfish;
        k.texture = "entity/silverfish".into();
        k.model = Arc::new(Part {
            name: "root".into(),
            pose: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            polygons: vec![
                Polygon { normal : [0.0, 0.0, - 1.0], cull_back : false, vertices :
                [[0.0, 0.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0, 0.0], [1.0, 1.0, 0.0,
                1.0, 1.0], [0.0, 1.0, 0.0, 0.0, 1.0],], }
            ],
            children: Vec::new(),
        });
        Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0)
    }
    /// A pack whose silverfish has a second variant, every silverfish wearing it.
    fn data() -> MobData {
        let root = std::env::temp_dir()
            .join(format!("horizon-render-random-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("assets/minecraft/optifine/random/entity");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("silverfish.properties"), "textures.1=2").unwrap();
        std::fs::write(dir.join("silverfish2.png"), "").unwrap();
        let random = RandomTextures::scan(&root);
        let _ = std::fs::remove_dir_all(&root);
        MobData {
            states: Vec::new(),
            placements: Default::default(),
            heightmaps: Default::default(),
            animations: Default::default(),
            villager_hats: Default::default(),
            cem: Default::default(),
            random_textures: RwLock::new(random),
            emissive: Default::default(),
            blinks: Default::default(),
            biome_tags: Default::default(),
            entity_tags: Default::default(),
            pois: Default::default(),
            species_placements: Default::default(),
        }
    }
    fn uvs(m: &Mob, data: &MobData, atlas: Atlas) -> Vec<[f32; 2]> {
        let mut out = Vec::new();
        render_with(m, 0.0, glam::DVec3::ZERO, atlas, 0, data, &[], &mut out);
        out.iter().map(|v| v.uv).collect()
    }
    #[test]
    fn a_mob_draws_its_variant_and_the_texture_when_the_atlas_lacks_it() {
        let data = data();
        let mut m = silverfish();
        {
            let textures = data.random_textures.read().unwrap();
            let seen = crate::mobs::random_textures::Seen {
                biome: None,
                y: 64,
                blocks: Vec::new(),
                moon_phase: 0,
                day_time: 0,
                weather: horizon_optifine::random::Weather::Clear,
            };
            crate::mobs::random_textures::choose(
                &mut m,
                &textures,
                &data,
                Some(seen),
                0,
            );
        }
        let both = |name: &str| match name {
            "entity/silverfish" => Some([0.0, 0.0, 0.5, 0.5]),
            "minecraft:optifine/random/entity/silverfish2" => Some([0.5, 0.5, 1.0, 1.0]),
            _ => None,
        };
        let drawn = uvs(&m, &data, &both);
        assert_eq!(drawn.len(), 4);
        assert!(drawn.iter().all(| uv | uv[0] >= 0.5 && uv[1] >= 0.5));
        let vanilla_only = |name: &str| {
            (name == "entity/silverfish").then_some([0.0, 0.0, 0.5, 0.5])
        };
        let drawn = uvs(&m, &data, &vanilla_only);
        assert_eq!(drawn.len(), 4, "still drawn");
        assert!(drawn.iter().all(| uv | uv[0] <= 0.5 && uv[1] <= 0.5));
    }
    /// `EndermanRenderer.getRenderOffset` moves a creepy enderman by
    /// `0.02 * state.scale` blocks sideways: at `minecraft:scale` 2 the same
    /// gaussian offset doubles, and at the default it is the old 0.02.
    #[test]
    fn a_scaled_creepy_endermans_render_offset_scales() {
        use horizon_core::geometry::Vec3;
        let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
        kind.id = "minecraft:enderman".into();
        kind.species = Species::Enderman;
        let mut m = Mob::new(1, std::sync::Arc::new(kind), Vec3::ZERO, 0.0);
        m.extra.flag = true;
        m.extra.model_scale = 1.0;
        let one = enderman_render_offset(&m, 0.5);
        assert!(one.length() > 0.0, "the offset is the renderer's gaussian");
        m.extra.model_scale = 2.0;
        let two = enderman_render_offset(&m, 0.5);
        assert!((two.x - 2.0 * one.x).abs() < 1.0e-12, "{two:?} vs {one:?}");
        assert!((two.z - 2.0 * one.z).abs() < 1.0e-12, "{two:?} vs {one:?}");
        assert_eq!(two.y, 0.0);
    }
    /// `LivingEntityRenderer.submit` translates a sleeper toward its pillow
    /// before `poseStack.scale(state.scale)`: the bed offset is not scaled and
    /// the pose after it is. The uniform scale after `setupRotations` instead
    /// would leave the pose's own translations unscaled; the one before the
    /// sleeping translation would scale the bed.
    #[test]
    fn a_scaled_sleepers_bed_offset_is_not_scaled() {
        use crate::mobs::Species;
        use horizon_core::geometry::Vec3;
        let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
        kind.id = "minecraft:villager".into();
        kind.species = Species::Villager;
        (kind.width, kind.height, kind.eye_height) = (0.6, 1.95, 1.62);
        let mut m = Mob::new(1, std::sync::Arc::new(kind), Vec3::ZERO, 0.0);
        m.extra.ints[1] = 1;
        let bed = |m: &Mob| {
            let eye = m.awake_dims().2 as f32;
            glam::Vec3::new(0.0, 0.0, eye - 0.1)
        };
        let unscaled = base_transform_at(&m, 0.5, glam::Mat4::IDENTITY);
        let unscaled_pose = glam::Mat4::from_translation(-bed(&m)) * unscaled;
        m.extra.model_scale = 2.0;
        let scaled = base_transform_at(&m, 0.5, glam::Mat4::IDENTITY);
        let scaled_pose = glam::Mat4::from_translation(-bed(&m)) * scaled;
        let expected = glam::Mat4::from_scale(glam::Vec3::splat(2.0)) * unscaled_pose;
        let got = scaled_pose.to_cols_array();
        let want = expected.to_cols_array();
        for i in 0..16 {
            assert!(
                (got[i] - want[i]).abs() < 1.0e-4,
                "element {i}: {} != {}\nscaled: {got:?}\nunscaled x2: {want:?}", got[i],
                want[i]
            );
        }
    }
    /// A sleeping avatar's bed offset is `getEyeHeight(Pose.STANDING) - 0.1`,
    /// and the standing eye height carries `minecraft:scale`: at scale 2 the
    /// bed offset is 3.14, not the type's 1.52, and the pose after it doubles.
    #[test]
    fn a_scaled_sleeping_avatar_uses_its_scaled_eye_height() {
        use horizon_core::geometry::Vec3;
        let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
        kind.id = "minecraft:player".into();
        (kind.width, kind.height, kind.eye_height) = (0.6, 1.8, 1.62);
        let mut m = Mob::new(1, std::sync::Arc::new(kind), Vec3::ZERO, 0.0);
        m.extra.ints[1] = 1;
        let bed = |m: &Mob| {
            let eye = m.kind.eye_height as f32 * m.model_scale();
            glam::Vec3::new(0.0, 0.0, eye - 0.1)
        };
        let unscaled = base_transform_at(&m, 0.5, glam::Mat4::IDENTITY);
        let unscaled_pose = glam::Mat4::from_translation(-bed(&m)) * unscaled;
        m.extra.model_scale = 2.0;
        let scaled = base_transform_at(&m, 0.5, glam::Mat4::IDENTITY);
        let scaled_pose = glam::Mat4::from_translation(-bed(&m)) * scaled;
        let expected = glam::Mat4::from_scale(glam::Vec3::splat(2.0)) * unscaled_pose;
        let got = scaled_pose.to_cols_array();
        let want = expected.to_cols_array();
        for i in 0..16 {
            assert!(
                (got[i] - want[i]).abs() < 1.0e-4,
                "element {i}: {} != {}\nscaled: {got:?}\nunscaled x2: {want:?}", got[i],
                want[i]
            );
        }
    }
    /// The avatar's 0.9375 `AvatarRenderer.scale` and the entity's own
    /// `minecraft:scale` are two factors: at scale 2 the model is 1.875 times
    /// the model's, not 3.75, so the submitted scale is not applied twice.
    #[test]
    fn an_avatar_at_scale_two_scales_its_model_once() {
        use horizon_core::geometry::Vec3;
        let mut kind = (*crate::mobs::sensor_scan_tests::kind()).clone();
        kind.id = "minecraft:player".into();
        let mut m = Mob::new(1, std::sync::Arc::new(kind), Vec3::ZERO, 0.0);
        m.extra.model_scale = 2.0;
        let transform = base_transform_at(&m, 0.5, glam::Mat4::IDENTITY);
        for axis in [glam::Vec3::X, glam::Vec3::Y, glam::Vec3::Z] {
            let length = transform.transform_vector3(axis).length();
            assert!(
                (length - 1.875).abs() < 1.0e-5,
                "{axis:?} scales by {length}, not 2 * 0.9375"
            );
        }
    }
}
pub(crate) mod umbx_skins3d_build {
    pub type Quad = ([f32; 3], [[f32; 5]; 4]);
    pub const INNER: f32 = -0.2;
    pub const OUTER: f32 = 0.55;
    pub const MIN_ALPHA: u8 = 8;
    pub const MAX_FACES: usize = 24_000;
    pub struct Image<'a> {
        pub width: usize,
        pub height: usize,
        pub rgba: &'a [u8],
    }
    impl Image<'_> {
        pub fn new(width: usize, height: usize, rgba: &[u8]) -> Option<Image<'_>> {
            (width > 0 && height > 0 && rgba.len() == width * height * 4)
                .then_some(Image { width, height, rgba })
        }
        fn solid(&self, x: i64, y: i64) -> bool {
            x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
                && self.rgba[(y as usize * self.width + x as usize) * 4 + 3] > MIN_ALPHA
        }
    }
    fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| a[i] + b[i])
    }
    fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| a[i] - b[i])
    }
    fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
        a.map(|v| v * k)
    }
    fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
    }
    fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    fn normalized(a: [f32; 3]) -> [f32; 3] {
        let length = dot(a, a).sqrt();
        if length > 0.0 { scale(a, 1.0 / length) } else { a }
    }
    fn quad(
        points: [[f32; 3]; 4],
        uvs: [[f32; 2]; 4],
        normal: [f32; 3],
        winding: f32,
    ) -> Quad {
        let facing = dot(
            cross(sub(points[1], points[0]), sub(points[2], points[0])),
            normal,
        );
        let mut vertices: [[f32; 5]; 4] = std::array::from_fn(|i| [
            points[i][0],
            points[i][1],
            points[i][2],
            uvs[i][0],
            uvs[i][1],
        ]);
        if facing * winding < 0.0 {
            vertices.reverse();
        }
        (normal, vertices)
    }
    struct Face {
        origin: [f32; 3],
        across: [f32; 3],
        down: [f32; 3],
        start: [f32; 2],
        x: (i64, i64),
        y: (i64, i64),
        normal: [f32; 3],
        winding: f32,
        links: [Option<(usize, usize, bool)>; 4],
    }
    impl Face {
        fn new((normal, v): &Quad, image: &Image) -> Option<Face> {
            let (w, h) = (image.width as f32, image.height as f32);
            let position = |i: usize| [v[i][0], v[i][1], v[i][2]];
            let (p0, p1, p2) = (position(0), position(1), position(2));
            let (ua, ub, va, vb) = (v[1][3] * w, v[0][3] * w, v[1][4] * h, v[2][4] * h);
            if (ub - ua).abs() < 0.5 || (vb - va).abs() < 0.5 {
                return None;
            }
            let n = normalized(*normal);
            let winding = dot(cross(sub(p1, p0), sub(p2, p0)), n).signum();
            if winding == 0.0 {
                return None;
            }
            Some(Face {
                origin: p1,
                across: scale(sub(p0, p1), 1.0 / (ub - ua)),
                down: scale(sub(p2, p1), 1.0 / (vb - va)),
                start: [ua, va],
                x: (ua.min(ub).round() as i64, ua.max(ub).round() as i64),
                y: (va.min(vb).round() as i64, va.max(vb).round() as i64),
                normal: n,
                winding,
                links: [None; 4],
            })
        }
        fn at(&self, tx: f32, ty: f32) -> [f32; 3] {
            add(
                self.origin,
                add(
                    scale(self.across, tx - self.start[0]),
                    scale(self.down, ty - self.start[1]),
                ),
            )
        }
        fn inside(&self, tx: i64, ty: i64) -> bool {
            (self.x.0..self.x.1).contains(&tx) && (self.y.0..self.y.1).contains(&ty)
        }
        fn span(&self, side: usize) -> i64 {
            if side % 2 == 0 { self.x.1 - self.x.0 } else { self.y.1 - self.y.0 }
        }
        fn edge(&self, side: usize) -> ([f32; 3], [f32; 3]) {
            let (x0, x1, y0, y1) = (
                self.x.0 as f32,
                self.x.1 as f32,
                self.y.0 as f32,
                self.y.1 as f32,
            );
            match side {
                0 => (self.at(x0, y0), self.at(x1, y0)),
                1 => (self.at(x1, y0), self.at(x1, y1)),
                2 => (self.at(x0, y1), self.at(x1, y1)),
                _ => (self.at(x0, y0), self.at(x0, y1)),
            }
        }
        fn border(&self, side: usize, index: i64) -> (i64, i64) {
            match side {
                0 => (self.x.0 + index, self.y.0),
                1 => (self.x.1 - 1, self.y.0 + index),
                2 => (self.x.0 + index, self.y.1 - 1),
                _ => (self.x.0, self.y.0 + index),
            }
        }
        fn outward(&self, side: usize) -> [f32; 3] {
            normalized(
                match side {
                    0 => scale(self.down, -1.0),
                    1 => self.across,
                    2 => self.down,
                    _ => scale(self.across, -1.0),
                },
            )
        }
    }
    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        let d = sub(a, b);
        dot(d, d) < 1e-6
    }
    fn link(faces: &mut [Face]) {
        for f in 0..faces.len() {
            for side in 0..4 {
                let (a, b) = faces[f].edge(side);
                let found = (0..faces.len())
                    .filter(|g| *g != f)
                    .find_map(|g| {
                        (0..4)
                            .find_map(|other| {
                                let (c, d) = faces[g].edge(other);
                                let same = faces[g].span(other) == faces[f].span(side);
                                if same && close(a, c) && close(b, d) {
                                    Some((g, other, false))
                                } else if same && close(a, d) && close(b, c) {
                                    Some((g, other, true))
                                } else {
                                    None
                                }
                            })
                    });
                faces[f].links[side] = found;
            }
        }
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Rim {
        Hidden,
        Wall,
        Seal(usize),
    }
    fn rim(
        faces: &[Face],
        image: &Image,
        f: usize,
        side: usize,
        index: i64,
        tx: i64,
        ty: i64,
    ) -> Rim {
        let face = &faces[f];
        if !image.solid(tx, ty) {
            return Rim::Hidden;
        }
        let (nx, ny) = match side {
            0 => (tx, ty - 1),
            1 => (tx + 1, ty),
            2 => (tx, ty + 1),
            _ => (tx - 1, ty),
        };
        if face.inside(nx, ny) {
            return if image.solid(nx, ny) { Rim::Hidden } else { Rim::Wall };
        }
        if let Some((g, other, flip)) = face.links[side] {
            let along = if flip { face.span(side) - 1 - index } else { index };
            let (ox, oy) = faces[g].border(other, along);
            if image.solid(ox, oy) {
                return Rim::Seal(g);
            }
        }
        Rim::Wall
    }
    fn rectangles(face: &Face, image: &Image) -> Vec<(i64, i64, i64, i64)> {
        let (width, height) = (
            (face.x.1 - face.x.0) as usize,
            (face.y.1 - face.y.0) as usize,
        );
        let mut used = vec![false; width * height];
        let index = |x: i64, y: i64| {
            (y - face.y.0) as usize * width + (x - face.x.0) as usize
        };
        let mut out = Vec::new();
        for ty in face.y.0..face.y.1 {
            for tx in face.x.0..face.x.1 {
                let free = |x: i64, y: i64, used: &[bool]| {
                    image.solid(x, y) && !used[index(x, y)]
                };
                if !free(tx, ty, &used) {
                    continue;
                }
                let mut right = tx + 1;
                while right < face.x.1 && free(right, ty, &used) {
                    right += 1;
                }
                let mut bottom = ty + 1;
                while bottom < face.y.1 && (tx..right).all(|x| free(x, bottom, &used)) {
                    bottom += 1;
                }
                for y in ty..bottom {
                    for x in tx..right {
                        used[index(x, y)] = true;
                    }
                }
                out.push((tx, ty, right, bottom));
            }
        }
        out
    }
    fn rims(faces: &[Face], image: &Image, f: usize, out: &mut Vec<Quad>) {
        let face = &faces[f];
        let (w, h) = (image.width as f32, image.height as f32);
        let uv = |tx: f32, ty: f32| [tx / w, ty / h];
        for side in 0..4 {
            let horizontal = side % 2 == 0;
            let (along, rows) = if horizontal {
                (face.x, face.y)
            } else {
                (face.y, face.x)
            };
            for row in rows.0..rows.1 {
                let state = |at: i64| {
                    let (tx, ty) = if horizontal { (at, row) } else { (row, at) };
                    rim(faces, image, f, side, at - along.0, tx, ty)
                };
                let mut at = along.0;
                while at < along.1 {
                    let kind = state(at);
                    let mut end = at + 1;
                    while end < along.1 && state(end) == kind {
                        end += 1;
                    }
                    let (a, b, middle) = (at as f32, end as f32, row as f32 + 0.5);
                    let line = if side == 1 || side == 2 {
                        row as f32 + 1.0
                    } else {
                        row as f32
                    };
                    let (edge_a, edge_b, uvs) = if horizontal {
                        (
                            face.at(a, line),
                            face.at(b, line),
                            [uv(a, middle), uv(b, middle)],
                        )
                    } else {
                        (
                            face.at(line, a),
                            face.at(line, b),
                            [uv(middle, a), uv(middle, b)],
                        )
                    };
                    let (outer_a, outer_b) = (
                        add(edge_a, scale(face.normal, OUTER)),
                        add(edge_b, scale(face.normal, OUTER)),
                    );
                    let spread = [uvs[0], uvs[1], uvs[1], uvs[0]];
                    match kind {
                        Rim::Hidden => {}
                        Rim::Wall => {
                            let (inner_a, inner_b) = (
                                add(edge_a, scale(face.normal, INNER)),
                                add(edge_b, scale(face.normal, INNER)),
                            );
                            out.push(
                                quad(
                                    [outer_a, outer_b, inner_b, inner_a],
                                    spread,
                                    face.outward(side),
                                    face.winding,
                                ),
                            );
                        }
                        Rim::Seal(g) => {
                            let lift = scale(faces[g].normal, OUTER);
                            out.push(
                                quad(
                                    [outer_a, outer_b, add(outer_b, lift), add(outer_a, lift)],
                                    spread,
                                    face.normal,
                                    face.winding,
                                ),
                            );
                        }
                    }
                    at = end;
                }
            }
        }
    }
    pub fn voxels(polygons: &[Quad], image: &Image, limit: usize) -> Option<Vec<Quad>> {
        let mut faces: Vec<Face> = polygons
            .iter()
            .filter_map(|p| Face::new(p, image))
            .collect();
        link(&mut faces);
        let (w, h) = (image.width as f32, image.height as f32);
        let mut out = Vec::new();
        for (f, face) in faces.iter().enumerate() {
            for (x0, y0, x1, y1) in rectangles(face, image) {
                let (a, b, c, d) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
                let cell = [face.at(a, b), face.at(c, b), face.at(c, d), face.at(a, d)];
                let uvs = [
                    [a / w, b / h],
                    [c / w, b / h],
                    [c / w, d / h],
                    [a / w, d / h],
                ];
                out.push(
                    quad(
                        cell.map(|p| add(p, scale(face.normal, OUTER))),
                        uvs,
                        face.normal,
                        face.winding,
                    ),
                );
            }
            rims(&faces, image, f, &mut out);
            if out.len() > limit {
                return None;
            }
        }
        Some(out)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        const FRONT: Quad = (
            [0.0, 0.0, -1.0],
            [
                [4.0, -8.0, -4.0, 0.25, 0.125],
                [-4.0, -8.0, -4.0, 0.125, 0.125],
                [-4.0, 0.0, -4.0, 0.125, 0.25],
                [4.0, 0.0, -4.0, 0.25, 0.25],
            ],
        );
        const RIGHT: Quad = (
            [1.0, 0.0, 0.0],
            [
                [4.0, -8.0, 4.0, 0.375, 0.125],
                [4.0, -8.0, -4.0, 0.25, 0.125],
                [4.0, 0.0, -4.0, 0.25, 0.25],
                [4.0, 0.0, 4.0, 0.375, 0.25],
            ],
        );
        fn skin(opaque: &[(usize, usize)]) -> Vec<u8> {
            let mut rgba = vec![0; 64 * 64 * 4];
            for (x, y) in opaque {
                rgba[(y * 64 + x) * 4..][..4].copy_from_slice(&[200, 100, 50, 255]);
            }
            rgba
        }
        fn faces(opaque: &[(usize, usize)]) -> Vec<Quad> {
            let rgba = skin(opaque);
            voxels(&[FRONT], &Image::new(64, 64, &rgba).unwrap(), MAX_FACES).unwrap()
        }
        fn outer_area(out: &[Quad]) -> f32 {
            let span = |v: &[[f32; 5]; 4], i: usize| {
                v.iter().map(|p| p[i]).fold(f32::MIN, f32::max)
                    - v.iter().map(|p| p[i]).fold(f32::MAX, f32::min)
            };
            out.iter()
                .filter(|(n, _)| *n == [0.0, 0.0, -1.0])
                .map(|(_, v)| span(v, 0) * span(v, 1))
                .sum()
        }
        #[test]
        fn a_lone_pixel_becomes_a_closed_voxel() {
            let out = faces(&[(12, 12)]);
            assert_eq!(out.len(), 5);
            let (normal, outer) = out[0];
            assert_eq!(normal, [0.0, 0.0, - 1.0]);
            assert!(outer.iter().all(| v | (v[2] - (- 4.0 - OUTER)).abs() < 1e-5));
            assert!((outer_area(& out) - 1.0).abs() < 1e-5);
        }
        #[test]
        fn a_full_face_is_one_quad_with_four_walls() {
            let every: Vec<(usize, usize)> = (8..16)
                .flat_map(|x| (8..16).map(move |y| (x, y)))
                .collect();
            let out = faces(&every);
            assert_eq!(out.len(), 5);
            assert!((outer_area(& out) - 64.0).abs() < 1e-3);
            let (_, v) = out[0];
            assert_eq!(v.iter().map(| p | p[3]).fold(f32::MIN, f32::max), 0.25);
            assert_eq!(v.iter().map(| p | p[3]).fold(f32::MAX, f32::min), 0.125);
        }
        #[test]
        fn merged_quads_cover_exactly_the_painted_texels() {
            let shape = [
                (8, 8),
                (9, 8),
                (10, 8),
                (8, 9),
                (10, 9),
                (8, 10),
                (9, 10),
                (10, 10),
                (13, 13),
            ];
            let out = faces(&shape);
            assert!((outer_area(& out) - shape.len() as f32).abs() < 1e-3);
            assert!(faces(& []).is_empty());
            assert!(faces(& [(2, 2)]).is_empty());
        }
        #[test]
        fn a_wall_reads_the_texels_along_its_edge() {
            let out = faces(&[(12, 12), (13, 12)]);
            assert_eq!(out.len(), 5);
            let (_, v) = out.iter().find(|(n, _)| n[1] != 0.0).unwrap();
            let us: Vec<f32> = v.iter().map(|p| p[3]).collect();
            assert!(us.contains(& (12.0 / 64.0)) && us.contains(& (14.0 / 64.0)));
        }
        #[test]
        fn faces_keep_the_source_winding_and_point_out() {
            let source = dot(
                    cross(
                        sub([-4.0, -8.0, -4.0], [4.0, -8.0, -4.0]),
                        sub([-4.0, 0.0, -4.0], [4.0, -8.0, -4.0]),
                    ),
                    [0.0, 0.0, -1.0],
                )
                .signum();
            for (normal, v) in faces(&[(12, 12)]) {
                let p = |i: usize| [v[i][0], v[i][1], v[i][2]];
                assert_eq!(
                    dot(cross(sub(p(1), p(0)), sub(p(2), p(0))), normal).signum(), source
                );
            }
            let sides: Vec<[f32; 3]> = faces(&[(12, 12)])
                .iter()
                .skip(1)
                .map(|f| f.0)
                .collect();
            assert!(
                sides.iter().all(| n | n[2] == 0.0 && (dot(* n, * n) - 1.0).abs() < 1e-5)
            );
        }
        #[test]
        fn a_box_edge_painted_on_both_sides_is_sealed() {
            let mut paint: Vec<(usize, usize)> = (8..16).map(|y| (15, y)).collect();
            paint.extend((8..16).map(|y| (16, y)));
            let rgba = skin(&paint);
            let image = Image::new(64, 64, &rgba).unwrap();
            let out = voxels(&[FRONT, RIGHT], &image, MAX_FACES).unwrap();
            let tip = |p: &[f32; 5]| {
                p[0] > 4.0 + OUTER - 1e-4 && p[2] < -4.0 - OUTER + 1e-4
            };
            assert_eq!(
                out.iter().filter(| (_, v) | v.iter().filter(| p | tip(p)).count() == 2)
                .count(), 2
            );
            let open = |n: [f32; 3]| n != [0.0, 0.0, -1.0] && n != [1.0, 0.0, 0.0];
            assert_eq!(
                out.iter().filter(| (n, v) | open(* n) && v.iter().all(| p | (p[0] - 4.0)
                .abs() < 1e-4)).count(), 0
            );
        }
        #[test]
        fn oversized_or_broken_input_is_refused() {
            let rgba = skin(&[(10, 10), (12, 12), (14, 14)]);
            assert!(
                voxels(& [FRONT], & Image::new(64, 64, & rgba).unwrap(), 4).is_none()
            );
            assert!(Image::new(64, 64, & rgba[..100]).is_none());
            assert!(Image::new(0, 64, & []).is_none());
        }
    }
}
pub mod umbx_skins3d_cache {
    use super::umbx_skins3d_build::{self as build, Image, MAX_FACES};
    use crate::mobs::{MobType, Part, Polygon};
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
    pub const PLAYER: &str = "minecraft:player";
    pub const HAT: u8 = 1;
    pub const JACKET_AND_SLEEVES: u8 = 2 | 4 | 8;
    pub const PANTS: u8 = 16 | 32;
    pub const ALL: u8 = HAT | JACKET_AND_SLEEVES | PANTS;
    const LAYERS: [&str; 6] = [
        "hat",
        "jacket",
        "left_sleeve",
        "right_sleeve",
        "left_pants",
        "right_pants",
    ];
    const MAX_SKINS: usize = 96;
    const MAX_WANTED: usize = 64;
    struct Skin {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
        models: HashMap<(bool, u8), Arc<Part>>,
    }
    struct Cache {
        skins: HashMap<String, Skin>,
        order: VecDeque<String>,
        wanted: HashSet<String>,
    }
    static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache {
        skins: HashMap::new(),
        order: VecDeque::new(),
        wanted: HashSet::new(),
    }));
    fn cache() -> MutexGuard<'static, Cache> {
        CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn wanted() -> Vec<String> {
        cache().wanted.drain().collect()
    }
    pub fn offer(location: &str, width: usize, height: usize, rgba: &[u8]) {
        if Image::new(width, height, rgba).is_none() {
            return;
        }
        let mut cache = cache();
        if cache.skins.contains_key(location) {
            return;
        }
        while cache.order.len() >= MAX_SKINS {
            if let Some(old) = cache.order.pop_front() {
                cache.skins.remove(&old);
            }
        }
        cache.order.push_back(location.to_owned());
        cache
            .skins
            .insert(
                location.to_owned(),
                Skin {
                    width,
                    height,
                    rgba: rgba.to_vec(),
                    models: HashMap::new(),
                },
            );
    }
    fn layered(part: &mut Part, image: &Image, shown: u8, faces: &mut usize) {
        if let Some(bit) = LAYERS.iter().position(|layer| *layer == part.name)
            && shown & (1 << bit) != 0
        {
            let quads: Vec<build::Quad> = part
                .polygons
                .iter()
                .map(|p| (p.normal, p.vertices))
                .collect();
            if let Some(built) = build::voxels(&quads, image, MAX_FACES) {
                *faces += built.len();
                part.polygons = built
                    .into_iter()
                    .map(|(normal, vertices)| Polygon {
                        normal,
                        vertices,
                        cull_back: false,
                    })
                    .collect();
            }
        }
        for child in &mut part.children {
            layered(child, image, shown, faces);
        }
    }
    pub fn model(
        kind: &MobType,
        location: &str,
        slim: bool,
        shown: u8,
    ) -> Option<Arc<Part>> {
        if shown == 0 {
            return None;
        }
        let mut cache = cache();
        let Some(skin) = cache.skins.get_mut(location) else {
            if cache.wanted.len() < MAX_WANTED {
                cache.wanted.insert(location.to_owned());
            }
            return None;
        };
        if let Some(model) = skin.models.get(&(slim, shown)) {
            return Some(model.clone());
        }
        let image = Image::new(skin.width, skin.height, &skin.rgba)?;
        let key = if slim { "player_slim#main" } else { "main" };
        let mut part = kind
            .layer(key)
            .map_or_else(|| (*kind.model).clone(), |model| (**model).clone());
        let mut faces = 0;
        layered(&mut part, &image, shown, &mut faces);
        if shown == ALL {
            log::info!(
                "3D Skins: {location} ({}) has {faces} faces", if slim { "slim" } else {
                "wide" }
            );
        }
        let model = Arc::new(part);
        skin.models.insert((slim, shown), model.clone());
        Some(model)
    }
}
