//! Move synergy: which moves go well (synergy) or badly (anti-synergy) with an ability, base stats or another
//! move, for trainer movesets. Also the move lists the trainer moveset randomizer uses.
//!
//! Every function returns the moves of `moves` that match, in the order of `moves`, each move once; moves are told
//! apart by number.
//!
//! Ability numbers are the canonical ones of Gen 4 onwards, the same in every later generation. Gen 3 numbers
//! Air Lock 77, which is Tangled Feet from Gen 4 on; callers pass [`synergy_ability`] of the game's number.
//!
//! Deliberate choices:
//! - Gen 3's Air Lock (77) counts as Air Lock, not as Tangled Feet, so Rayquaza gets Air Lock's anti-synergy with
//!   weather moves rather than Tangled Feet's synergy with Petal Dance, Thrash and Outrage. See [`synergy_ability`].
//! - Lock-On and Mind Reader go with moves of accuracy 1–50 only, not with accuracy 0, which in Gen 3 is every move
//!   that never misses, status moves included. Compound Eyes and No Guard already leave those out.
//! - The base stat total is [`BaseStats::total`], also for Shedinja and Gen 1 (one Special stat); the only Gen 1
//!   special case would concern Power Split and Guard Split, which Gen 1 does not have.

use ppr_model::{
    BaseStats, CriticalChance, Effectiveness, Move, MoveCategory, MoveId, StatChangeMoveType, StatChangeType,
    StatusMoveType, StatusType, Type, TypeChart,
};

/// Huge Power.
pub(super) const HUGE_POWER: u16 = 37;
/// Pure Power.
pub(super) const PURE_POWER: u16 = 74;
/// Hustle.
pub(super) const HUSTLE: u16 = 55;
/// Gorilla Tactics.
pub(super) const GORILLA_TACTICS: u16 = 255;
/// Moxie.
pub(super) const MOXIE: u16 = 153;
/// Soul Heart.
pub(super) const SOUL_HEART: u16 = 220;
/// Air Lock.
const AIR_LOCK: u16 = 76;
/// Air Lock's number in Gen 3.
const GEN3_AIR_LOCK: u16 = 77;

/// Splash, Celebrate, Hold Hands, Teleport, and Reflect Type, which the AI cannot
/// use well.
pub(super) const USELESS_MOVES: [MoveId; 5] = [m::SPLASH, m::CELEBRATE, m::HOLD_HANDS, m::TELEPORT, m::REFLECT_TYPE];

/// Moves that only help in double battles.
pub(super) const DOUBLE_BATTLE_MOVES: [MoveId; 16] = [
    m::FOLLOW_ME,
    m::HELPING_HAND,
    m::RAGE_POWDER,
    m::AFTER_YOU,
    m::ALLY_SWITCH,
    m::HEAL_PULSE,
    m::QUASH,
    m::ION_DELUGE,
    m::MAT_BLOCK,
    m::AROMATIC_MIST,
    m::ELECTRIFY,
    m::INSTRUCT,
    m::SPOTLIGHT,
    m::DECORATE,
    m::LIFE_DEW,
    m::COACHING,
];

/// Moves useless without another (see [`requires_other_move`]).
pub(super) const REQUIRES_OTHER_MOVE: [MoveId; 4] = [m::SPIT_UP, m::SWALLOW, m::DREAM_EATER, m::NIGHTMARE];

/// Moves too situational to push others out of a moveset.
pub(super) const CANNOT_OBSOLETE_MOVES: [MoveId; 7] = [
    m::GEAR_UP,
    m::MAGNETIC_FLUX,
    m::FOCUS_PUNCH,
    m::EXPLOSION,
    m::SELF_DESTRUCT,
    m::GEOMANCY,
    m::VENOM_DRENCH,
];

/// Moves whose power doesn't compare, so stronger moves don't replace
/// them.
pub(super) const CANNOT_BE_OBSOLETED_MOVES: [MoveId; 13] = [
    m::RETURN,
    m::FRUSTRATION,
    m::ENDEAVOR,
    m::FLAIL,
    m::REVERSAL,
    m::HIDDEN_POWER,
    m::STORED_POWER,
    m::SMELLING_SALTS,
    m::FLING,
    m::POWER_TRIP,
    m::COUNTER,
    m::MIRROR_COAT,
    m::SUPER_FANG,
];

/// The canonical ability number of a game's ability number: Gen 3's Air Lock (77) becomes [`AIR_LOCK`], and Gen 3's
/// Cacophony (76), which has no canonical number, becomes 0. Other numbers are already canonical.
pub(super) const fn synergy_ability(ability: u16, generation: u8) -> u16 {
    match (generation, ability) {
        (3, GEN3_AIR_LOCK) => AIR_LOCK,
        (3, AIR_LOCK) => 0,
        _ => ability,
    }
}

/// The move numbers a synergy collects, possibly repeated, over the moves it picks from.
struct Picks<'m, 'a> {
    moves: &'m [&'a Move],
    ids: Vec<MoveId>,
}

impl<'m, 'a> Picks<'m, 'a> {
    /// Nothing picked yet from `moves`.
    fn new(moves: &'m [&'a Move]) -> Self {
        Self { moves, ids: Vec::new() }
    }

    /// Picks these move numbers.
    fn add(&mut self, ids: &[MoveId]) {
        self.ids.extend_from_slice(ids);
    }

    /// Picks the moves that match `pred`.
    fn add_where(&mut self, pred: impl Fn(&Move) -> bool) {
        self.ids
            .extend(self.moves.iter().filter(|mv| pred(mv)).map(|mv| mv.number));
    }

    /// The moves picked, in the order of the move list, each once.
    fn finish(self) -> Vec<&'a Move> {
        let mut out: Vec<&'a Move> = Vec::new();
        for &mv in self.moves {
            if self.ids.contains(&mv.number) && !out.iter().any(|o| o.number == mv.number) {
                out.push(mv);
            }
        }
        out
    }
}

/// Whether either type is `t`.
fn has_type(type1: Type, type2: Option<Type>, t: Type) -> bool {
    type1 == t || type2 == Some(t)
}

/// A damaging move of type `t`.
fn damaging_of(mv: &Move, t: Type) -> bool {
    mv.move_type == Some(t) && mv.category != MoveCategory::Status
}

/// The stages of the first stat change; 0 without stat changes.
fn first_stages(mv: &Move) -> i8 {
    mv.stat_changes.first().map_or(0, |s| s.stages)
}

/// Two or more stat changes.
fn has_second_stat_change(mv: &Move) -> bool {
    mv.stat_changes.len() >= 2
}

/// A change of `stat` up (`positive`, which includes 0 stages) or down.
fn has_stat_change(mv: &Move, stat: StatChangeType, positive: bool) -> bool {
    mv.stat_changes
        .iter()
        .any(|s| s.stat == stat && positive != (s.stages < 0))
}

/// Changes the user's own stats, with or without damage.
fn targets_user(mv: &Move) -> bool {
    matches!(
        mv.stat_change_move_type,
        StatChangeMoveType::DamageUser | StatChangeMoveType::NoDamageUser
    )
}

/// A status move that causes `status`.
fn status_move_causing(mv: &Move, status: StatusType) -> bool {
    mv.category == MoveCategory::Status && mv.status == Some(status)
}

/// Poisons or badly poisons.
fn poisons(mv: &Move) -> bool {
    matches!(mv.status, Some(StatusType::Poison | StatusType::ToxicPoison))
}

/// Moves an ability makes somewhat better.
pub(super) fn soft_ability_synergy<'a>(
    ability: u16,
    moves: &[&'a Move],
    type1: Type,
    type2: Option<Type>,
) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    let boosted = match ability {
        a::DRIZZLE | a::PRIMORDIAL_SEA => Some(Type::Water),
        a::DROUGHT | a::DESOLATE_LAND => Some(Type::Fire),
        a::REFRIGERATE if has_type(type1, type2, Type::Ice) => Some(Type::Normal),
        a::PIXILATE if has_type(type1, type2, Type::Fairy) => Some(Type::Normal),
        a::AERILATE if has_type(type1, type2, Type::Flying) => Some(Type::Normal),
        a::GALVANIZE if has_type(type1, type2, Type::Electric) => Some(Type::Normal),
        a::DARK_AURA => Some(Type::Dark),
        a::FAIRY_AURA => Some(Type::Fairy),
        a::STEELWORKER | a::STEELY_SPIRIT => Some(Type::Steel),
        a::ELECTRIC_SURGE | a::TRANSISTOR => Some(Type::Electric),
        a::PSYCHIC_SURGE => Some(Type::Psychic),
        a::GRASSY_SURGE => Some(Type::Grass),
        a::DRAGONS_MAW => Some(Type::Dragon),
        _ => None,
    };
    if let Some(t) = boosted {
        picks.add_where(|mv| damaging_of(mv, t));
    }
    if ability == a::GALE_WINGS {
        // Status moves too: Gale Wings gives them priority as well.
        picks.add_where(|mv| mv.move_type == Some(Type::Flying));
    }
    picks.finish()
}

/// Moves an ability makes somewhat worse.
pub(super) fn soft_ability_anti_synergy<'a>(ability: u16, moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    let weakened = match ability {
        a::DRIZZLE => Some(Type::Fire),
        a::DROUGHT => Some(Type::Water),
        a::MISTY_SURGE => Some(Type::Dragon),
        _ => None,
    };
    if let Some(t) = weakened {
        picks.add_where(|mv| damaging_of(mv, t));
    }
    picks.finish()
}

/// Moves an ability makes much better. `perfect_accuracy` is the game's "never misses" accuracy.
pub(super) fn hard_ability_synergy<'a>(
    ability: u16,
    type1: Type,
    type2: Option<Type>,
    moves: &[&'a Move],
    generation: u8,
    perfect_accuracy: u8,
) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    hard_ability_fixed(&mut picks, ability, type1, type2, generation);
    hard_ability_matching(&mut picks, ability, type1, type2, perfect_accuracy);
    picks.finish()
}

/// The fixed moves of [`hard_ability_synergy`].
fn hard_ability_fixed(picks: &mut Picks<'_, '_>, ability: u16, type1: Type, type2: Option<Type>, generation: u8) {
    match ability {
        a::DRIZZLE | a::PRIMORDIAL_SEA => {
            picks.add(&[m::THUNDER, m::HURRICANE]);
            if has_type(type1, type2, Type::Water) {
                picks.add(&[m::WEATHER_BALL]);
            }
        }
        a::SPEED_BOOST => picks.add(&[m::BATON_PASS, m::STORED_POWER, m::POWER_TRIP]),
        a::STURDY if generation >= 5 => {
            picks.add(&[m::ENDEAVOR, m::COUNTER, m::MIRROR_COAT, m::FLAIL, m::REVERSAL]);
        }
        a::SAND_VEIL | a::SAND_RUSH => picks.add(&[m::SANDSTORM]),
        a::STATIC => picks.add(&[m::SMELLING_SALTS, m::HEX]),
        a::OWN_TEMPO | a::TANGLED_FEET => picks.add(&[m::PETAL_DANCE, m::THRASH, m::OUTRAGE]),
        a::SHADOW_TAG | a::ARENA_TRAP | a::SOUNDPROOF => picks.add(&[m::PERISH_SONG]),
        a::POISON_POINT => picks.add(&[m::VENOSHOCK, m::HEX]),
        a::EFFECT_SPORE | a::FLAME_BODY => picks.add(&[m::HEX]),
        a::SWIFT_SWIM | a::RAIN_DISH | a::DRY_SKIN | a::HYDRATION => picks.add(&[m::RAIN_DANCE]),
        a::CHLOROPHYLL | a::HARVEST | a::LEAF_GUARD => picks.add(&[m::SUNNY_DAY]),
        a::SAND_STREAM if has_type(type1, type2, Type::Rock) => picks.add(&[m::WEATHER_BALL]),
        a::EARLY_BIRD | a::SHED_SKIN => picks.add(&[m::REST]),
        a::TRUANT => picks.add(&[m::TRANSFORM]),
        a::GUTS => picks.add(&[m::FACADE]),
        a::DROUGHT | a::DESOLATE_LAND => {
            picks.add(&[
                m::SOLAR_BEAM,
                m::SOLAR_BLADE,
                m::MORNING_SUN,
                m::SYNTHESIS,
                m::MOONLIGHT,
            ]);
            if generation >= 5 {
                picks.add(&[m::GROWTH]);
            }
            if has_type(type1, type2, Type::Fire) {
                picks.add(&[m::WEATHER_BALL]);
            }
        }
        a::SNOW_CLOAK | a::ICE_BODY | a::SLUSH_RUSH => picks.add(&[m::HAIL]),
        a::UNBURDEN => picks.add(&[m::FLING, m::ACROBATICS]),
        a::SIMPLE => picks.add(&[m::ACUPRESSURE]),
        a::MAGIC_GUARD => picks.add(&[m::MIND_BLOWN]),
        a::STALL => picks.add(&[m::METAL_BURST, m::PAYBACK]),
        a::SLOW_START => picks.add(&[
            m::TRANSFORM,
            m::PROTECT,
            m::DETECT,
            m::KINGS_SHIELD,
            m::BANEFUL_BUNKER,
            m::FLY,
            m::DIG,
            m::BOUNCE,
            m::DIVE,
        ]),
        a::SNOW_WARNING => {
            picks.add(&[m::AURORA_VEIL, m::BLIZZARD]);
            if has_type(type1, type2, Type::Ice) {
                picks.add(&[m::WEATHER_BALL]);
            }
        }
        a::RECKLESS => picks.add(&[m::JUMP_KICK, m::HIGH_JUMP_KICK]),
        a::HEAVY_METAL => picks.add(&[m::HEAT_CRASH, m::HEAVY_SLAM]),
        a::MOODY => picks.add(&[m::STORED_POWER, m::POWER_TRIP]),
        a::REGENERATOR => picks.add(&[m::U_TURN, m::VOLT_SWITCH, m::PARTING_SHOT]),
        a::PRANKSTER => picks.add(&[
            m::DESTINY_BOND,
            m::ENCORE,
            m::REFLECT,
            m::LIGHT_SCREEN,
            m::GRUDGE,
            m::PAIN_SPLIT,
            m::SUBSTITUTE,
        ]),
        a::STRONG_JAW => picks.add(&[
            m::BITE,
            m::CRUNCH,
            m::FIRE_FANG,
            m::FISHIOUS_REND,
            m::HYPER_FANG,
            m::ICE_FANG,
            m::JAW_LOCK,
            m::POISON_FANG,
            m::PSYCHIC_FANGS,
            m::THUNDER_FANG,
        ]),
        a::MEGA_LAUNCHER => picks.add(&[
            m::AURA_SPHERE,
            m::DARK_PULSE,
            m::DRAGON_PULSE,
            m::ORIGIN_PULSE,
            m::TERRAIN_PULSE,
            m::WATER_PULSE,
        ]),
        a::WIMP_OUT | a::EMERGENCY_EXIT => picks.add(&[m::FAKE_OUT, m::FIRST_IMPRESSION]),
        a::MERCILESS => picks.add(&[m::BANEFUL_BUNKER]),
        a::SURGE_SURFER => picks.add(&[m::ELECTRIC_TERRAIN]),
        _ => {}
    }
}

/// The moves of [`hard_ability_synergy`] picked by their data.
fn hard_ability_matching(
    picks: &mut Picks<'_, '_>,
    ability: u16,
    type1: Type,
    type2: Option<Type>,
    perfect_accuracy: u8,
) {
    match ability {
        a::COMPOUND_EYES => picks.add_where(|mv| mv.accuracy > 0 && mv.accuracy <= 80),
        a::SERENE_GRACE => picks.add_where(|mv| {
            (matches!(
                mv.stat_change_move_type,
                StatChangeMoveType::DamageTarget | StatChangeMoveType::DamageUser
            ) && mv.stat_changes.first().map_or(0, |s| s.percent_chance) < 100)
                || (mv.status_move_type == StatusMoveType::Damage && mv.status_percent_chance < 100)
                || mv.flinch_percent_chance > 0
        }),
        HUSTLE => {
            picks.add_where(|mv| targets_user(mv) && has_stat_change(mv, StatChangeType::Accuracy, true));
            picks.add_where(|mv| mv.category == MoveCategory::Physical && mv.accuracy == perfect_accuracy);
        }
        a::ROCK_HEAD | a::MAGIC_GUARD | a::RECKLESS => picks.add_where(|mv| mv.recoil_percent > 0),
        a::IRON_FIST => picks.add_where(|mv| mv.is_punch),
        a::SIMPLE => picks.add_where(|mv| targets_user(mv) && first_stages(mv) > 0),
        a::ADAPTABILITY => picks.add_where(|mv| {
            mv.category != MoveCategory::Status && (mv.move_type == Some(type1) || mv.move_type == type2)
        }),
        a::SKILL_LINK => picks.add_where(|mv| mv.hit_count_hundredths >= 300),
        a::SNIPER => picks.add_where(|mv| {
            matches!(
                mv.critical_chance,
                CriticalChance::Increased | CriticalChance::Guaranteed
            )
        }),
        a::SUPER_LUCK => picks.add_where(|mv| mv.critical_chance == CriticalChance::Increased),
        a::ANALYTIC => picks.add_where(|mv| mv.power > 0 && mv.priority < 0),
        a::NO_GUARD => picks.add_where(|mv| mv.accuracy > 0 && mv.accuracy <= 70),
        a::TECHNICIAN => {
            picks.add_where(|mv| (40..=60).contains(&mv.power) || mv.hit_count_hundredths > 100);
        }
        a::BAD_DREAMS => picks.add_where(|mv| mv.status == Some(StatusType::Sleep)),
        a::SHEER_FORCE => picks.add_where(|mv| {
            mv.stat_change_move_type == StatChangeMoveType::DamageTarget
                || (mv.stat_change_move_type == StatChangeMoveType::DamageUser && first_stages(mv) > 0)
                || mv.status_move_type == StatusMoveType::Damage
                || mv.flinch_percent_chance > 0
        }),
        a::CONTRARY => {
            picks.add_where(|mv| mv.stat_change_move_type == StatChangeMoveType::DamageUser && first_stages(mv) < 0);
        }
        a::POISON_TOUCH | a::TOUGH_CLAWS => picks.add_where(|mv| mv.makes_contact),
        a::PRANKSTER => picks.add_where(|mv| mv.status_move_type == StatusMoveType::NoDamage),
        a::MERCILESS => picks.add_where(poisons),
        a::LIQUID_VOICE if has_type(type1, type2, Type::Water) => picks.add_where(|mv| mv.is_sound && mv.power > 0),
        a::TRIAGE => picks.add_where(|mv| mv.absorb_percent > 0),
        a::CORROSION => picks.add_where(|mv| mv.category == MoveCategory::Status && poisons(mv)),
        _ => {}
    }
}

/// Moves an ability makes much worse or useless.
pub(super) fn hard_ability_anti_synergy<'a>(ability: u16, moves: &[&'a Move]) -> Vec<&'a Move> {
    /// Weather moves and those weather spoils; what a weather-setting ability (other than sun) makes worse.
    const NON_SUN_WEATHER: [MoveId; 9] = [
        m::SOLAR_BEAM,
        m::SOLAR_BLADE,
        m::MORNING_SUN,
        m::SYNTHESIS,
        m::MOONLIGHT,
        m::RAIN_DANCE,
        m::SUNNY_DAY,
        m::HAIL,
        m::SANDSTORM,
    ];
    /// What a sun-setting ability makes worse.
    const SUN: [MoveId; 6] = [
        m::THUNDER,
        m::HURRICANE,
        m::RAIN_DANCE,
        m::SUNNY_DAY,
        m::HAIL,
        m::SANDSTORM,
    ];

    let mut picks = Picks::new(moves);
    match ability {
        a::PRIMORDIAL_SEA => {
            picks.add_where(|mv| damaging_of(mv, Type::Fire));
            picks.add(&NON_SUN_WEATHER);
        }
        a::DRIZZLE | a::SAND_STREAM | a::SNOW_WARNING => picks.add(&NON_SUN_WEATHER),
        a::SPEED_BOOST => {
            picks.add_where(|mv| {
                mv.stat_change_move_type == StatChangeMoveType::NoDamageUser
                    && mv
                        .stat_changes
                        .first()
                        .is_some_and(|s| s.stat == StatChangeType::Speed && s.stages > 0)
            });
            picks.add(&[m::PSYCH_UP, m::HAZE]);
        }
        a::DESOLATE_LAND => {
            picks.add_where(|mv| damaging_of(mv, Type::Water));
            picks.add(&SUN);
        }
        a::DROUGHT => picks.add(&SUN),
        a::NO_GUARD => {
            picks.add(&[m::LOCK_ON, m::MIND_READER]);
            picks.add_where(|mv| {
                has_stat_change(mv, StatChangeType::Accuracy, true)
                    || has_stat_change(mv, StatChangeType::Evasion, true)
                    || has_stat_change(mv, StatChangeType::Evasion, false)
            });
        }
        a::DAMP => picks.add(&[m::SELF_DESTRUCT, m::EXPLOSION, m::MIND_BLOWN, m::MISTY_EXPLOSION]),
        a::INSOMNIA | a::VITAL_SPIRIT | a::COMATOSE | a::SWEET_VEIL => picks.add(&[m::REST]),
        AIR_LOCK | a::CLOUD_NINE | a::DELTA_STREAM => {
            picks.add(&[m::RAIN_DANCE, m::SUNNY_DAY, m::SANDSTORM, m::HAIL]);
        }
        a::SIMPLE => picks.add_where(|mv| targets_user(mv) && first_stages(mv) < 0),
        a::CONTRARY => {
            picks.add_where(|mv| targets_user(mv) && first_stages(mv) > 0);
            picks.add(&[m::SHELL_SMASH]);
        }
        a::LIGHT_METAL => picks.add(&[m::HEAT_CRASH, m::HEAVY_SLAM]),
        a::ELECTRIC_SURGE => {
            picks.add_where(|mv| status_move_causing(mv, StatusType::Sleep));
            picks.add(&[m::REST]);
        }
        a::PSYCHIC_SURGE => picks.add_where(|mv| mv.priority > 0),
        a::MISTY_SURGE => {
            picks.add_where(|mv| {
                mv.category == MoveCategory::Status && mv.status.is_some_and(|s| s != StatusType::Confusion)
            });
            picks.add(&[m::REST]);
        }
        a::GRASSY_SURGE => picks.add(&[m::EARTHQUAKE, m::MAGNITUDE, m::BULLDOZE]),
        _ => {}
    }
    picks.finish()
}

/// `part` over the base stat total.
fn ratio(part: u16, stats: BaseStats) -> f64 {
    f64::from(part) / f64::from(stats.total())
}

/// Moves the base stats make better.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "the trainer moveset randomizer's interface"
)]
pub(super) fn stat_synergy<'a>(stats: &BaseStats, moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    let hp = ratio(stats.hp.into(), *stats);
    let speed = ratio(stats.speed.into(), *stats);
    if hp < 1.0 / 8.0 {
        picks.add(&[m::PAIN_SPLIT, m::ENDEAVOR]);
    }
    if hp >= 1.0 / 4.0 {
        picks.add(&[m::WATER_SPOUT, m::ERUPTION, m::COUNTER, m::MIRROR_COAT]);
    }
    if u16::from(stats.attack) * 2 < u16::from(stats.defense) {
        picks.add(&[m::POWER_TRICK]);
    }
    if speed < 1.0 / 8.0 {
        picks.add(&[m::GYRO_BALL]);
    }
    if speed >= 1.0 / 4.0 {
        picks.add(&[m::ELECTRO_BALL]);
    }
    if ratio(u16::from(stats.attack) + u16::from(stats.sp_attack), *stats) < 1.0 / 4.0 {
        picks.add(&[m::POWER_SPLIT]);
    }
    if ratio(u16::from(stats.defense) + u16::from(stats.sp_defense), *stats) < 1.0 / 4.0 {
        picks.add(&[m::GUARD_SPLIT]);
    }
    picks.finish()
}

/// Moves the base stats make worse.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "the trainer moveset randomizer's interface"
)]
pub(super) fn stat_anti_synergy<'a>(stats: &BaseStats, moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    let speed = ratio(stats.speed.into(), *stats);
    if ratio(stats.hp.into(), *stats) >= 1.0 / 4.0 {
        picks.add(&[m::PAIN_SPLIT, m::ENDEAVOR]);
    }
    if u16::from(stats.defense) * 2 < u16::from(stats.attack) {
        picks.add(&[m::POWER_TRICK]);
    }
    if speed >= 1.0 / 4.0 {
        picks.add(&[m::GYRO_BALL]);
    }
    if speed < 1.0 / 8.0 {
        picks.add(&[m::ELECTRO_BALL]);
    }
    if ratio(u16::from(stats.attack) + u16::from(stats.sp_attack), *stats) >= 1.0 / 3.0 {
        picks.add(&[m::POWER_SPLIT]);
    }
    if ratio(u16::from(stats.defense) + u16::from(stats.sp_defense), *stats) >= 1.0 / 3.0 {
        picks.add(&[m::GUARD_SPLIT]);
    }
    picks.finish()
}

/// Lowers the target's Speed while hitting, or raises the user's.
fn controls_speed(mv: &Move) -> bool {
    (mv.stat_change_move_type == StatChangeMoveType::DamageTarget && has_stat_change(mv, StatChangeType::Speed, false))
        || (targets_user(mv) && has_stat_change(mv, StatChangeType::Speed, true))
}

/// A move that may flinch, without priority.
fn flinches(mv: &Move) -> bool {
    mv.flinch_percent_chance > 0 && mv.priority == 0
}

/// Moves that go well with `mv`. The generation is not used.
pub(super) fn move_synergy<'a>(mv: &Move, moves: &[&'a Move], _generation: u8) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    if controls_speed(mv) {
        picks.add_where(flinches);
    }
    if flinches(mv) {
        picks.add_where(controls_speed);
    }
    if first_stages(mv) >= 2 || has_second_stat_change(mv) {
        picks.add(&[m::BATON_PASS, m::STORED_POWER, m::POWER_TRIP]);
    }
    if mv.status == Some(StatusType::Sleep) {
        picks.add(&[m::DREAM_EATER, m::NIGHTMARE, m::HEX]);
    }
    move_synergy_by_number(&mut picks, mv.number);
    picks.finish()
}

/// The `switch (mv1.number)` of [`move_synergy`].
fn move_synergy_by_number(picks: &mut Picks<'_, '_>, number: MoveId) {
    match number {
        m::TOXIC => {
            picks.add(&[
                m::PROTECT,
                m::DETECT,
                m::KINGS_SHIELD,
                m::DIG,
                m::FLY,
                m::BOUNCE,
                m::DIVE,
            ]);
            picks.add_where(|mv| mv.is_trap);
            picks.add(&[m::VENOSHOCK, m::HEX]);
        }
        m::POISON_POWDER | m::POISON_GAS | m::BANEFUL_BUNKER | m::TOXIC_THREAD => {
            picks.add(&[m::VENOSHOCK, m::HEX]);
        }
        m::VENOSHOCK => picks.add_where(|mv| mv.category == MoveCategory::Status && poisons(mv)),
        m::PROTECT | m::DETECT | m::KINGS_SHIELD => picks.add(&[m::TOXIC, m::LEECH_SEED, m::WILL_O_WISP]),
        m::BATON_PASS => {
            picks.add_where(|mv| first_stages(mv) >= 2 || has_second_stat_change(mv));
            picks.add(&[m::SHELL_SMASH]);
        }
        m::WILL_O_WISP => picks.add(&[m::HEX]),
        // Deliberate: accuracy 0 is "never misses" (see the module doc).
        m::LOCK_ON | m::MIND_READER => picks.add_where(|mv| mv.accuracy > 0 && mv.accuracy <= 50),
        m::SUNNY_DAY => picks.add(&[m::SOLAR_BLADE, m::SOLAR_BEAM]),
        m::RAIN_DANCE => picks.add(&[m::THUNDER, m::HURRICANE]),
        m::POWER_SWAP => picks.add_where(|mv| {
            mv.stat_change_move_type == StatChangeMoveType::DamageUser
                && (has_stat_change(mv, StatChangeType::Attack, false)
                    || has_stat_change(mv, StatChangeType::SpecialAttack, false))
        }),
        m::ENDURE => picks.add(&[m::REVERSAL, m::FLAIL, m::ENDEAVOR]),
        m::ENDEAVOR => picks.add_where(|mv| mv.category != MoveCategory::Status && mv.priority > 0),
        m::THUNDER_WAVE | m::GLARE | m::STUN_SPORE => {
            picks.add_where(|mv| mv.flinch_percent_chance > 0);
            picks.add_where(|mv| status_move_causing(mv, StatusType::Confusion));
            picks.add(&[m::HEX]);
        }
        m::HAIL => picks.add(&[m::BLIZZARD, m::AURORA_VEIL]),
        m::STOCKPILE => picks.add(&[m::SPIT_UP, m::SWALLOW]),
        m::SPIT_UP | m::SWALLOW => picks.add(&[m::STOCKPILE]),
        m::LEECH_SEED | m::PERISH_SONG => picks.add_where(|mv| mv.is_trap),
        m::SPIKES | m::STEALTH_ROCK | m::TOXIC_SPIKES => {
            picks.add(&[m::ROAR, m::WHIRLWIND, m::DRAGON_TAIL, m::CIRCLE_THROW]);
        }
        m::REST => picks.add(&[m::SLEEP_TALK]),
        m::FOCUS_ENERGY => picks.add_where(|mv| mv.critical_chance == CriticalChance::Increased),
        m::FOCUS_PUNCH | m::DREAM_EATER | m::NIGHTMARE => picks
            .add_where(|mv| mv.status_move_type == StatusMoveType::NoDamage && mv.status == Some(StatusType::Sleep)),
        m::TORMENT => picks.add(&[m::ENCORE]),
        m::ENCORE => picks.add(&[m::TORMENT]),
        m::HEX => {
            // "Not confusion" also holds for no status at all.
            picks.add_where(|mv| {
                mv.status_move_type == StatusMoveType::NoDamage && mv.status != Some(StatusType::Confusion)
            });
            picks.add(&[m::BANEFUL_BUNKER]);
        }
        m::STORED_POWER => {
            picks.add_where(|mv| first_stages(mv) > 1 || has_second_stat_change(mv));
            picks.add(&[m::ACUPRESSURE, m::SHELL_SMASH]);
        }
        m::SWAGGER => picks.add(&[m::PUNISHMENT]),
        m::PUNISHMENT => picks.add(&[m::SWAGGER]),
        m::SHELL_SMASH => picks.add(&[m::STORED_POWER]),
        _ => {}
    }
}

/// Moves that cover what `mv` is not very effective against, and attacks of the category a setup move boosts.
pub(super) fn soft_move_synergy<'a>(mv: &Move, moves: &[&'a Move], chart: &TypeChart) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    if mv.category != MoveCategory::Status
        && let Some(attacker) = mv.move_type
    {
        let mut resisting = chart.when_attacking(attacker, Effectiveness::Half);
        resisting.extend(chart.when_attacking(attacker, Effectiveness::Zero));
        for defender in resisting {
            let covering = chart.when_defending(defender, Effectiveness::Double);
            picks.add_where(|o| {
                o.category != MoveCategory::Status && o.move_type.is_some_and(|t| covering.contains(&t))
            });
        }
    }
    match mv.number {
        m::SWORDS_DANCE => picks.add_where(|o| o.category == MoveCategory::Physical),
        m::NASTY_PLOT | m::TAIL_GLOW => picks.add_where(|o| o.category == MoveCategory::Special),
        _ => {}
    }
    picks.finish()
}

/// Moves that clash with `mv` (another status for the same target, a second Protect, Return with Frustration,
/// phazing after Leech Seed or Perish Song, the Sport that weakens it).
pub(super) fn hard_move_anti_synergy<'a>(mv: &Move, moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    if mv.category == MoveCategory::Status
        && let Some(status) = mv.status
    {
        picks.add_where(|o| {
            o.category == MoveCategory::Status
                && o.status
                    .is_some_and(|s| s == status || (status != StatusType::Confusion && s != StatusType::Confusion))
        });
    }
    match mv.number {
        m::PROTECT => picks.add(&[m::DETECT, m::BANEFUL_BUNKER, m::KINGS_SHIELD]),
        m::DETECT => picks.add(&[m::PROTECT, m::BANEFUL_BUNKER, m::KINGS_SHIELD]),
        m::KINGS_SHIELD => picks.add(&[m::PROTECT, m::DETECT, m::BANEFUL_BUNKER]),
        m::BANEFUL_BUNKER => picks.add(&[m::PROTECT, m::DETECT, m::KINGS_SHIELD]),
        m::RETURN => picks.add(&[m::FRUSTRATION]),
        m::FRUSTRATION => picks.add(&[m::RETURN]),
        m::LEECH_SEED | m::PERISH_SONG => picks.add(&[m::WHIRLWIND, m::ROAR, m::CIRCLE_THROW, m::DRAGON_TAIL]),
        _ => {}
    }
    if mv.category != MoveCategory::Status {
        match mv.move_type {
            Some(Type::Fire) => picks.add(&[m::WATER_SPORT]),
            Some(Type::Electric) => picks.add(&[m::MUD_SPORT]),
            _ => {}
        }
    }
    picks.finish()
}

/// Attacks of the same type as `mv`, and those Water Sport or Mud Sport weaken.
pub(super) fn soft_move_anti_synergy<'a>(mv: &Move, moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    if mv.category != MoveCategory::Status {
        picks.add_where(|o| o.category != MoveCategory::Status && o.move_type == mv.move_type);
    }
    match mv.number {
        m::WATER_SPORT => picks.add_where(|o| damaging_of(o, Type::Fire)),
        m::MUD_SPORT => picks.add_where(|o| damaging_of(o, Type::Electric)),
        _ => {}
    }
    picks.finish()
}

/// The moves among `moves` that make `mv` work (Stockpile for Spit Up and Swallow, a sleep move for Dream Eater
/// and Nightmare, Rest for Snore and Sleep Talk). Empty when `mv` needs none or none is there.
pub(super) fn requires_other_move<'a>(mv: &Move, moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut picks = Picks::new(moves);
    match mv.number {
        m::SPIT_UP | m::SWALLOW => picks.add(&[m::STOCKPILE]),
        m::DREAM_EATER | m::NIGHTMARE => picks.add_where(|o| status_move_causing(o, StatusType::Sleep)),
        m::SNORE | m::SLEEP_TALK => picks.add(&[m::REST]),
        _ => {}
    }
    picks.finish()
}

/// Move numbers (national move index) used here.
mod m {
    use ppr_model::MoveId;

    pub(super) const ACROBATICS: MoveId = 512;
    pub(super) const ACUPRESSURE: MoveId = 367;
    pub(super) const AFTER_YOU: MoveId = 495;
    pub(super) const ALLY_SWITCH: MoveId = 502;
    pub(super) const AROMATIC_MIST: MoveId = 597;
    pub(super) const AURA_SPHERE: MoveId = 396;
    pub(super) const AURORA_VEIL: MoveId = 694;
    pub(super) const BANEFUL_BUNKER: MoveId = 661;
    pub(super) const BATON_PASS: MoveId = 226;
    pub(super) const BITE: MoveId = 44;
    pub(super) const BLIZZARD: MoveId = 59;
    pub(super) const BOUNCE: MoveId = 340;
    pub(super) const BULLDOZE: MoveId = 523;
    pub(super) const CELEBRATE: MoveId = 606;
    pub(super) const CIRCLE_THROW: MoveId = 509;
    pub(super) const COACHING: MoveId = 811;
    pub(super) const COUNTER: MoveId = 68;
    pub(super) const CRUNCH: MoveId = 242;
    pub(super) const DARK_PULSE: MoveId = 399;
    pub(super) const DECORATE: MoveId = 777;
    pub(super) const DESTINY_BOND: MoveId = 194;
    pub(super) const DETECT: MoveId = 197;
    pub(super) const DIG: MoveId = 91;
    pub(super) const DIVE: MoveId = 291;
    pub(super) const DRAGON_PULSE: MoveId = 406;
    pub(super) const DRAGON_TAIL: MoveId = 525;
    pub(super) const DREAM_EATER: MoveId = 138;
    pub(super) const EARTHQUAKE: MoveId = 89;
    pub(super) const ELECTRIC_TERRAIN: MoveId = 604;
    pub(super) const ELECTRIFY: MoveId = 582;
    pub(super) const ELECTRO_BALL: MoveId = 486;
    pub(super) const ENCORE: MoveId = 227;
    pub(super) const ENDEAVOR: MoveId = 283;
    pub(super) const ENDURE: MoveId = 203;
    pub(super) const ERUPTION: MoveId = 284;
    pub(super) const EXPLOSION: MoveId = 153;
    pub(super) const FACADE: MoveId = 263;
    pub(super) const FAKE_OUT: MoveId = 252;
    pub(super) const FIRE_FANG: MoveId = 424;
    pub(super) const FIRST_IMPRESSION: MoveId = 660;
    pub(super) const FISHIOUS_REND: MoveId = 755;
    pub(super) const FLAIL: MoveId = 175;
    pub(super) const FLING: MoveId = 374;
    pub(super) const FLY: MoveId = 19;
    pub(super) const FOCUS_ENERGY: MoveId = 116;
    pub(super) const FOCUS_PUNCH: MoveId = 264;
    pub(super) const FOLLOW_ME: MoveId = 266;
    pub(super) const FRUSTRATION: MoveId = 218;
    pub(super) const GEAR_UP: MoveId = 674;
    pub(super) const GEOMANCY: MoveId = 601;
    pub(super) const GLARE: MoveId = 137;
    pub(super) const GROWTH: MoveId = 74;
    pub(super) const GRUDGE: MoveId = 288;
    pub(super) const GUARD_SPLIT: MoveId = 470;
    pub(super) const GYRO_BALL: MoveId = 360;
    pub(super) const HAIL: MoveId = 258;
    pub(super) const HAZE: MoveId = 114;
    pub(super) const HEAL_PULSE: MoveId = 505;
    pub(super) const HEAT_CRASH: MoveId = 535;
    pub(super) const HEAVY_SLAM: MoveId = 484;
    pub(super) const HELPING_HAND: MoveId = 270;
    pub(super) const HEX: MoveId = 506;
    pub(super) const HIDDEN_POWER: MoveId = 237;
    pub(super) const HIGH_JUMP_KICK: MoveId = 136;
    pub(super) const HOLD_HANDS: MoveId = 607;
    pub(super) const HURRICANE: MoveId = 542;
    pub(super) const HYPER_FANG: MoveId = 158;
    pub(super) const ICE_FANG: MoveId = 423;
    pub(super) const INSTRUCT: MoveId = 689;
    pub(super) const ION_DELUGE: MoveId = 569;
    pub(super) const JAW_LOCK: MoveId = 746;
    pub(super) const JUMP_KICK: MoveId = 26;
    pub(super) const KINGS_SHIELD: MoveId = 588;
    pub(super) const LEECH_SEED: MoveId = 73;
    pub(super) const LIFE_DEW: MoveId = 791;
    pub(super) const LIGHT_SCREEN: MoveId = 113;
    pub(super) const LOCK_ON: MoveId = 199;
    pub(super) const MAGNETIC_FLUX: MoveId = 602;
    pub(super) const MAGNITUDE: MoveId = 222;
    pub(super) const MAT_BLOCK: MoveId = 561;
    pub(super) const METAL_BURST: MoveId = 368;
    pub(super) const MIND_BLOWN: MoveId = 720;
    pub(super) const MIND_READER: MoveId = 170;
    pub(super) const MIRROR_COAT: MoveId = 243;
    pub(super) const MISTY_EXPLOSION: MoveId = 802;
    pub(super) const MOONLIGHT: MoveId = 236;
    pub(super) const MORNING_SUN: MoveId = 234;
    pub(super) const MUD_SPORT: MoveId = 300;
    pub(super) const NASTY_PLOT: MoveId = 417;
    pub(super) const NIGHTMARE: MoveId = 171;
    pub(super) const ORIGIN_PULSE: MoveId = 618;
    pub(super) const OUTRAGE: MoveId = 200;
    pub(super) const PAIN_SPLIT: MoveId = 220;
    pub(super) const PARTING_SHOT: MoveId = 575;
    pub(super) const PAYBACK: MoveId = 371;
    pub(super) const PERISH_SONG: MoveId = 195;
    pub(super) const PETAL_DANCE: MoveId = 80;
    pub(super) const POISON_FANG: MoveId = 305;
    pub(super) const POISON_GAS: MoveId = 139;
    pub(super) const POISON_POWDER: MoveId = 77;
    pub(super) const POWER_SPLIT: MoveId = 471;
    pub(super) const POWER_SWAP: MoveId = 384;
    pub(super) const POWER_TRICK: MoveId = 379;
    pub(super) const POWER_TRIP: MoveId = 681;
    pub(super) const PROTECT: MoveId = 182;
    pub(super) const PSYCHIC_FANGS: MoveId = 706;
    pub(super) const PSYCH_UP: MoveId = 244;
    pub(super) const PUNISHMENT: MoveId = 386;
    pub(super) const QUASH: MoveId = 511;
    pub(super) const RAGE_POWDER: MoveId = 476;
    pub(super) const RAIN_DANCE: MoveId = 240;
    pub(super) const REFLECT: MoveId = 115;
    pub(super) const REFLECT_TYPE: MoveId = 513;
    pub(super) const REST: MoveId = 156;
    pub(super) const RETURN: MoveId = 216;
    pub(super) const REVERSAL: MoveId = 179;
    pub(super) const ROAR: MoveId = 46;
    pub(super) const SANDSTORM: MoveId = 201;
    pub(super) const SELF_DESTRUCT: MoveId = 120;
    pub(super) const SHELL_SMASH: MoveId = 504;
    pub(super) const SLEEP_TALK: MoveId = 214;
    pub(super) const SMELLING_SALTS: MoveId = 265;
    pub(super) const SNORE: MoveId = 173;
    pub(super) const SOLAR_BEAM: MoveId = 76;
    pub(super) const SOLAR_BLADE: MoveId = 669;
    pub(super) const SPIKES: MoveId = 191;
    pub(super) const SPIT_UP: MoveId = 255;
    pub(super) const SPLASH: MoveId = 150;
    pub(super) const SPOTLIGHT: MoveId = 671;
    pub(super) const STEALTH_ROCK: MoveId = 446;
    pub(super) const STOCKPILE: MoveId = 254;
    pub(super) const STORED_POWER: MoveId = 500;
    pub(super) const STUN_SPORE: MoveId = 78;
    pub(super) const SUBSTITUTE: MoveId = 164;
    pub(super) const SUNNY_DAY: MoveId = 241;
    pub(super) const SUPER_FANG: MoveId = 162;
    pub(super) const SWAGGER: MoveId = 207;
    pub(super) const SWALLOW: MoveId = 256;
    pub(super) const SWORDS_DANCE: MoveId = 14;
    pub(super) const SYNTHESIS: MoveId = 235;
    pub(super) const TAIL_GLOW: MoveId = 294;
    pub(super) const TELEPORT: MoveId = 100;
    pub(super) const TERRAIN_PULSE: MoveId = 805;
    pub(super) const THRASH: MoveId = 37;
    pub(super) const THUNDER: MoveId = 87;
    pub(super) const THUNDER_FANG: MoveId = 422;
    pub(super) const THUNDER_WAVE: MoveId = 86;
    pub(super) const TORMENT: MoveId = 259;
    pub(super) const TOXIC: MoveId = 92;
    pub(super) const TOXIC_SPIKES: MoveId = 390;
    pub(super) const TOXIC_THREAD: MoveId = 672;
    pub(super) const TRANSFORM: MoveId = 144;
    pub(super) const U_TURN: MoveId = 369;
    pub(super) const VENOM_DRENCH: MoveId = 599;
    pub(super) const VENOSHOCK: MoveId = 474;
    pub(super) const VOLT_SWITCH: MoveId = 521;
    pub(super) const WATER_PULSE: MoveId = 352;
    pub(super) const WATER_SPORT: MoveId = 346;
    pub(super) const WATER_SPOUT: MoveId = 323;
    pub(super) const WEATHER_BALL: MoveId = 311;
    pub(super) const WHIRLWIND: MoveId = 18;
    pub(super) const WILL_O_WISP: MoveId = 261;
}

/// Canonical ability numbers used here.
mod a {
    pub(super) const ADAPTABILITY: u16 = 91;
    pub(super) const AERILATE: u16 = 184;
    pub(super) const ANALYTIC: u16 = 148;
    pub(super) const ARENA_TRAP: u16 = 71;
    pub(super) const BAD_DREAMS: u16 = 123;
    pub(super) const CHLOROPHYLL: u16 = 34;
    pub(super) const CLOUD_NINE: u16 = 13;
    pub(super) const COMATOSE: u16 = 213;
    pub(super) const COMPOUND_EYES: u16 = 14;
    pub(super) const CONTRARY: u16 = 126;
    pub(super) const CORROSION: u16 = 212;
    pub(super) const DAMP: u16 = 6;
    pub(super) const DARK_AURA: u16 = 186;
    pub(super) const DELTA_STREAM: u16 = 191;
    pub(super) const DESOLATE_LAND: u16 = 190;
    pub(super) const DRAGONS_MAW: u16 = 263;
    pub(super) const DRIZZLE: u16 = 2;
    pub(super) const DROUGHT: u16 = 70;
    pub(super) const DRY_SKIN: u16 = 87;
    pub(super) const EARLY_BIRD: u16 = 48;
    pub(super) const EFFECT_SPORE: u16 = 27;
    pub(super) const ELECTRIC_SURGE: u16 = 226;
    pub(super) const EMERGENCY_EXIT: u16 = 194;
    pub(super) const FAIRY_AURA: u16 = 187;
    pub(super) const FLAME_BODY: u16 = 49;
    pub(super) const GALE_WINGS: u16 = 177;
    pub(super) const GALVANIZE: u16 = 206;
    pub(super) const GRASSY_SURGE: u16 = 229;
    pub(super) const GUTS: u16 = 62;
    pub(super) const HARVEST: u16 = 139;
    pub(super) const HEAVY_METAL: u16 = 134;
    pub(super) const HYDRATION: u16 = 93;
    pub(super) const ICE_BODY: u16 = 115;
    pub(super) const INSOMNIA: u16 = 15;
    pub(super) const IRON_FIST: u16 = 89;
    pub(super) const LEAF_GUARD: u16 = 102;
    pub(super) const LIGHT_METAL: u16 = 135;
    pub(super) const LIQUID_VOICE: u16 = 204;
    pub(super) const MAGIC_GUARD: u16 = 98;
    pub(super) const MEGA_LAUNCHER: u16 = 178;
    pub(super) const MERCILESS: u16 = 196;
    pub(super) const MISTY_SURGE: u16 = 228;
    pub(super) const MOODY: u16 = 141;
    pub(super) const NO_GUARD: u16 = 99;
    pub(super) const OWN_TEMPO: u16 = 20;
    pub(super) const PIXILATE: u16 = 182;
    pub(super) const POISON_POINT: u16 = 38;
    pub(super) const POISON_TOUCH: u16 = 143;
    pub(super) const PRANKSTER: u16 = 158;
    pub(super) const PRIMORDIAL_SEA: u16 = 189;
    pub(super) const PSYCHIC_SURGE: u16 = 227;
    pub(super) const RAIN_DISH: u16 = 44;
    pub(super) const RECKLESS: u16 = 120;
    pub(super) const REFRIGERATE: u16 = 174;
    pub(super) const REGENERATOR: u16 = 144;
    pub(super) const ROCK_HEAD: u16 = 69;
    pub(super) const SAND_RUSH: u16 = 146;
    pub(super) const SAND_STREAM: u16 = 45;
    pub(super) const SAND_VEIL: u16 = 8;
    pub(super) const SERENE_GRACE: u16 = 32;
    pub(super) const SHADOW_TAG: u16 = 23;
    pub(super) const SHED_SKIN: u16 = 61;
    pub(super) const SHEER_FORCE: u16 = 125;
    pub(super) const SIMPLE: u16 = 86;
    pub(super) const SKILL_LINK: u16 = 92;
    pub(super) const SLOW_START: u16 = 112;
    pub(super) const SLUSH_RUSH: u16 = 202;
    pub(super) const SNIPER: u16 = 97;
    pub(super) const SNOW_CLOAK: u16 = 81;
    pub(super) const SNOW_WARNING: u16 = 117;
    pub(super) const SOUNDPROOF: u16 = 43;
    pub(super) const SPEED_BOOST: u16 = 3;
    pub(super) const STALL: u16 = 100;
    pub(super) const STATIC: u16 = 9;
    pub(super) const STEELWORKER: u16 = 200;
    pub(super) const STEELY_SPIRIT: u16 = 252;
    pub(super) const STRONG_JAW: u16 = 173;
    pub(super) const STURDY: u16 = 5;
    pub(super) const SUPER_LUCK: u16 = 105;
    pub(super) const SURGE_SURFER: u16 = 207;
    pub(super) const SWEET_VEIL: u16 = 175;
    pub(super) const SWIFT_SWIM: u16 = 33;
    pub(super) const TANGLED_FEET: u16 = 77;
    pub(super) const TECHNICIAN: u16 = 101;
    pub(super) const TOUGH_CLAWS: u16 = 181;
    pub(super) const TRANSISTOR: u16 = 262;
    pub(super) const TRIAGE: u16 = 205;
    pub(super) const TRUANT: u16 = 54;
    pub(super) const UNBURDEN: u16 = 84;
    pub(super) const VITAL_SPIRIT: u16 = 72;
    pub(super) const WIMP_OUT: u16 = 193;
}

#[cfg(test)]
mod tests;
