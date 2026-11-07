//! Settings strings and `.rnqs` files of settings version 428, so settings travel between this app and other
//! randomizers that use the format.
//!
//! A string is the version number (428) followed by Base64 of 69 bytes of options, the ROM name, a CRC-32 and four
//! bytes of padding. A `.rnqs` file holds the version, the length and the string without its version.
//!
//! Deliberate choices:
//! - Options of games that are not ported (the legacy move updates) are written with their defaults and ignored when
//!   read; Custom Player Graphics and custom names are not part of the strings either.
//! - A level modifier, an update generation or a percentage whose checkbox is off is not kept, and neither are
//!   custom starters outside Custom mode.
//! - Strings of older versions are refused; they need to be updated to version 428 first.

use ppr_formats::{base64, crc32};
use ppr_model::{BattleStyle, ExpCurve, MiscTweak, Type};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    AbilityMode, AuraMode, BaseStatMode, BattleStyleMode, BstMode, CompatMode, EvolutionMode, ExpCurveScope,
    FieldItemMode, GenerationLimit, MovesetMode, Settings, ShopItemMode, StarterMode, StarterTypes, StaticMode,
    TotemMode, TradeMode, TrainerMode, TypeEffectivenessMode, TypeMode, WildEvolution, WildMode, WildTypes, WildZones,
};

/// The settings version this module reads and writes.
pub const SETTINGS_VERSION: u32 = 428;
/// Bytes of options before the ROM name.
const DATA_LENGTH: usize = 69;
/// The CRC-32 and the padding after the ROM name.
const TRAILER: usize = 8;
/// Level modifiers are stored shifted into a signed byte.
const MODIFIER_SHIFT: i16 = 28;
/// The default level of Make Evolutions Easier.
const EASIER_EVOLUTIONS_DEFAULT: u8 = 40;
/// Versions up to this one are too old to be updated.
const TOO_OLD: u32 = 172;

/// The type order settings strings use.
const TYPE_ORDER: [Type; 18] = [
    Type::Normal,
    Type::Fighting,
    Type::Flying,
    Type::Grass,
    Type::Water,
    Type::Fire,
    Type::Rock,
    Type::Ground,
    Type::Psychic,
    Type::Bug,
    Type::Dragon,
    Type::Electric,
    Type::Ghost,
    Type::Poison,
    Type::Ice,
    Type::Steel,
    Type::Dark,
    Type::Fairy,
];

/// The misc tweaks by their bit in the misc tweaks field.
const TWEAKS: [MiscTweak; 26] = [
    MiscTweak::BwExpPatch,
    MiscTweak::NerfXAccuracy,
    MiscTweak::FixCritRate,
    MiscTweak::FastestText,
    MiscTweak::RunningShoesIndoors,
    MiscTweak::RandomizePcPotion,
    MiscTweak::AllowPikachuEvolution,
    MiscTweak::NationalDexAtStart,
    MiscTweak::FastEggHatching,
    MiscTweak::ForceChallengeMode,
    MiscTweak::LowerCaseNames,
    MiscTweak::RandomizeCatchingTutorial,
    MiscTweak::BanLuckyEgg,
    MiscTweak::NoFreeLuckyEgg,
    MiscTweak::BanBigManiacItems,
    MiscTweak::SosBattlesForAll,
    MiscTweak::BalanceStaticLevels,
    MiscTweak::RetainAltFormes,
    MiscTweak::RunWithoutRunningShoes,
    MiscTweak::FasterHpAndExpBars,
    MiscTweak::FastDistortionWorld,
    MiscTweak::UpdateRotomFormeTyping,
    MiscTweak::DisableLowHpMusic,
    MiscTweak::ReusableTms,
    MiscTweak::ForgettableHms,
    MiscTweak::NoEvYields,
];

/// The EXP curves by their byte.
const EXP_CURVES: [ExpCurve; 6] = [
    ExpCurve::MediumFast,
    ExpCurve::Erratic,
    ExpCurve::Fluctuating,
    ExpCurve::MediumSlow,
    ExpCurve::Fast,
    ExpCurve::Slow,
];

/// What a settings string holds besides [`Settings`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsExtras {
    /// The game the settings were made for, as settings strings name it (e.g. "Pokemon Emerald (U)").
    pub rom_name: String,
    /// Race Mode.
    pub race_mode: bool,
}

/// Why a settings string or file can't be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsStringError {
    /// Not a settings string, or a damaged one.
    #[error("the settings string is invalid")]
    Malformed,
    /// Made by an older randomizer version, which needs to be updated first.
    #[error("these settings are from an older randomizer (version {0}) and can't be read yet")]
    OldVersion(u32),
    /// Made by a newer randomizer version.
    #[error("these settings are from a newer randomizer (version {0})")]
    NewerVersion(u32),
}

use SettingsStringError::Malformed;

/// A byte of flags: bit `i` is `flags[i]`.
fn bits(flags: &[bool]) -> u8 {
    flags
        .iter()
        .enumerate()
        .fold(0, |byte, (i, &on)| byte | (u8::from(on) << i))
}

/// Whether bit `i` of `byte` is set.
fn bit(byte: u8, i: u8) -> bool {
    byte >> i & 1 == 1
}

/// Which of the bits at `indices` is set; the first when none is, an error when several are.
fn choice<T: Copy>(byte: u8, options: &[(u8, T)]) -> Result<T, SettingsStringError> {
    let mut set = options.iter().filter(|&&(i, _)| bit(byte, i));
    let first = set.next();
    if set.next().is_some() {
        return Err(Malformed);
    }
    Ok(first.unwrap_or(&options[0]).1)
}

/// A level or evolution modifier, -100 to 155, as settings strings store it.
fn modifier(value: i16) -> u8 {
    i8::try_from(value - MODIFIER_SHIFT).unwrap_or(0).to_le_bytes()[0]
}

fn read_modifier(byte: u8) -> i16 {
    i16::from(i8::from_le_bytes([byte])) + MODIFIER_SHIFT
}

/// "Force good damaging moves" and its percentage, in one byte.
fn good_damaging(percent: Option<u8>) -> u8 {
    percent.map_or(0, |p| 0x80 | (p & 0x7F))
}

fn read_good_damaging(byte: u8) -> Option<u8> {
    bit(byte, 7).then_some(byte & 0x7F)
}

fn type_index(t: Type) -> u8 {
    TYPE_ORDER
        .iter()
        .position(|&u| u == t)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

fn exp_curve_byte(curve: ExpCurve) -> u8 {
    EXP_CURVES
        .iter()
        .position(|&c| c == curve)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

/// The generation restrictions as a number; no limit is the default restriction, everything allowed.
fn restrictions(limit: Option<&GenerationLimit>) -> u32 {
    let limit = limit.copied().unwrap_or_default();
    let gens = limit
        .generations
        .iter()
        .enumerate()
        .fold(0u32, |v, (i, &on)| v | u32::from(on) << (i + 1));
    gens | u32::from(limit.allow_evolutionary_relatives)
}

fn read_restrictions(value: u32) -> GenerationLimit {
    let mut generations = [false; 7];
    for (i, g) in generations.iter_mut().enumerate() {
        *g = value >> (i + 1) & 1 == 1;
    }
    GenerationLimit {
        generations,
        allow_evolutionary_relatives: value & 1 == 1,
    }
}

/// The settings string without its version: the options, ROM name and checksum in Base64.
fn encode(s: &Settings, extras: &SettingsExtras) -> String {
    let mut out = options(s, extras).to_vec();
    let name: Vec<u8> = extras
        .rom_name
        .chars()
        .map(|c| u8::try_from(c).ok().filter(u8::is_ascii).unwrap_or(b'?'))
        .take(255)
        .collect();
    out.push(u8::try_from(name.len()).unwrap_or(u8::MAX));
    out.extend_from_slice(&name);
    let checksum = crc32(&out);
    out.extend_from_slice(&checksum.to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    base64::encode(&out)
}

/// The 69 bytes of options, in the format's order.
#[expect(clippy::too_many_lines, reason = "one statement per byte, in the format's order")]
fn options(s: &Settings, extras: &SettingsExtras) -> [u8; DATA_LENGTH] {
    let mut d = [0u8; DATA_LENGTH];
    let ev = &s.evolutions;
    let md = &s.move_data;
    let tr = &s.trainers;
    let wild = &s.wild;
    let st = &s.starters;
    let (tms, tutors) = (&s.tms, &s.tutors);
    let (tm_compat, tutor_compat) = (&s.tm_hm_compat, &s.tutor_compat);
    d[0] = bits(&[
        ev.change_impossible,
        md.update_to_generation.is_some(),
        false,
        tr.random_names,
        tr.random_class_names,
        ev.easier_max_level.is_some(),
        ev.remove_time_based,
        ev.use_estimated_levels,
    ]);
    let bs = &s.base_stats;
    d[1] = bits(&[
        bs.follow_evolutions,
        bs.mode == BaseStatMode::Random,
        bs.mode == BaseStatMode::Shuffle,
        bs.mode == BaseStatMode::Unchanged,
        s.exp_curves.standardize.is_some(),
        bs.update_to_generation.is_some(),
        bs.follow_megas,
        bs.random_evolution_stats,
    ]);
    d[2] = bits(&[
        s.types.mode == TypeMode::FollowEvolutions,
        s.types.mode == TypeMode::Random,
        s.types.mode == TypeMode::Unchanged,
        false,
        false,
        false,
        s.types.follow_megas,
        s.types.force_dual,
    ]);
    let ab = &s.abilities;
    d[3] = bits(&[
        ab.mode == AbilityMode::Unchanged,
        ab.mode == AbilityMode::Random,
        ab.allow_wonder_guard,
        ab.follow_evolutions,
        ab.ban_trapping,
        ab.ban_negative,
        ab.ban_bad,
        ab.follow_megas,
    ]);
    d[4] = bits(&[
        st.mode == StarterMode::Custom,
        st.mode == StarterMode::Random,
        st.mode == StarterMode::Unchanged,
        st.mode == StarterMode::RandomWithTwoEvolutions,
        st.random_held_items,
        st.ban_bad_items,
        st.allow_alt_formes,
        st.mode == StarterMode::RandomBasic,
    ]);
    for i in 0..3 {
        let n = st.custom.get(i).copied().flatten().unwrap_or(0);
        d[5 + 2 * i..7 + 2 * i].copy_from_slice(&n.to_le_bytes());
    }
    let ms = &s.movesets;
    d[11] = bits(&[
        ms.mode == MovesetMode::RandomCompletely,
        ms.mode == MovesetMode::RandomPreferSameType,
        ms.mode == MovesetMode::Unchanged,
        ms.mode == MovesetMode::MetronomeOnly,
        ms.guaranteed_level_one_moves.is_some(),
        ms.reorder_damaging,
    ]) | (ms.guaranteed_level_one_moves.unwrap_or(2).clamp(2, 5) - 2) << 6;
    d[12] = good_damaging(ms.good_damaging_percent);
    d[13] = bits(&[
        tr.mode == TrainerMode::Unchanged,
        tr.mode == TrainerMode::Random,
        tr.mode == TrainerMode::RandomEvenDistribution,
        tr.mode == TrainerMode::RandomEvenDistributionMainGame,
        tr.mode == TrainerMode::TypeThemed,
        tr.mode == TrainerMode::TypeThemedEliteFourGyms,
        tr.mode == TrainerMode::KeepThemed,
        tr.mode == TrainerMode::KeepThemesOrPrimary,
    ]);
    d[14] = modifier(tr.evolve.unwrap_or(0));
    d[15] = bits(&[
        wild.mode == WildMode::Unchanged,
        wild.zones == WildZones::Slot,
        wild.zones == WildZones::Set,
        wild.zones == WildZones::Game,
        wild.keep_relations,
        wild.zones == WildZones::Location,
        wild.zones == WildZones::Map,
        wild.split_by_kind,
    ]);
    d[16] = bits(&[false, wild.similar_strength, wild.catch_em_all]);
    d[17] = bits(&[
        wild.types == WildTypes::None,
        wild.types == WildTypes::KeepPrimary,
        wild.types == WildTypes::ZoneThemes,
        wild.keep_themes,
        wild.evolution == WildEvolution::None,
        wild.evolution == WildEvolution::BasicOnly,
        wild.evolution == WildEvolution::SameStage,
    ]);
    d[18] = bits(&[
        wild.time_of_day,
        wild.min_catch_rate > 0,
        wild.no_legendaries,
        wild.random_held_items,
        wild.ban_bad_held_items,
        wild.balance_low_levels,
    ]);
    let statics = &s.statics;
    d[19] = bits(&[
        statics.mode == StaticMode::Unchanged,
        statics.mode == StaticMode::RandomMatching,
        statics.mode == StaticMode::Random,
        statics.mode == StaticMode::SimilarStrength,
        statics.limit_main_game_legendaries,
        statics.randomize_600_bst,
        statics.allow_alt_formes,
        statics.swap_mega_evos,
    ]);
    d[20] = bits(&[
        tm_compat.mode == CompatMode::RandomCompletely,
        tm_compat.mode == CompatMode::RandomPreferSameType,
        tm_compat.mode == CompatMode::Unchanged,
        tms.random,
        !tms.random,
        tm_compat.level_up_sanity,
        tms.keep_field_moves,
        tm_compat.mode == CompatMode::Full,
    ]);
    d[21] = bits(&[
        tm_compat.full_hm,
        tm_compat.follow_evolutions,
        tutor_compat.follow_evolutions,
    ]);
    d[22] = good_damaging(tms.good_damaging_percent);
    d[23] = bits(&[
        tutor_compat.mode == CompatMode::RandomCompletely,
        tutor_compat.mode == CompatMode::RandomPreferSameType,
        tutor_compat.mode == CompatMode::Unchanged,
        tutors.random,
        !tutors.random,
        tutor_compat.level_up_sanity,
        tutors.keep_field_moves,
        tutor_compat.mode == CompatMode::Full,
    ]);
    d[24] = good_damaging(tutors.good_damaging_percent);
    let trades = &s.trades;
    d[25] = bits(&[
        trades.mode == TradeMode::GivenAndRequested,
        trades.mode == TradeMode::Given,
        trades.random_items,
        trades.random_ivs,
        trades.random_nicknames,
        trades.random_ots,
        trades.mode == TradeMode::Unchanged,
    ]);
    let fi = &s.field_items;
    d[26] = bits(&[
        fi.mode == FieldItemMode::Random,
        fi.mode == FieldItemMode::Shuffle,
        fi.mode == FieldItemMode::Unchanged,
        fi.ban_bad,
        fi.mode == FieldItemMode::RandomEven,
    ]);
    d[27] = bits(&[
        md.power,
        md.accuracy,
        md.pp,
        md.types,
        md.category,
        statics.fix_music,
        md.names,
    ]);
    d[28] = bits(&[
        ev.mode == EvolutionMode::Unchanged,
        ev.mode == EvolutionMode::Random,
        ev.similar_strength,
        ev.same_typing,
        ev.limit_three_stages,
        ev.force_change,
        ev.allow_alt_formes,
        ev.mode == EvolutionMode::RandomEveryLevel,
    ]);
    d[29] = bits(&[
        tr.similar_strength,
        tr.rival_carries_starter,
        tr.weight_types,
        tr.no_legendaries,
        tr.no_early_wonder_guard,
        tr.swap_mega_evos,
        tr.random_shiny,
        tr.avoid_duplicates,
    ]);
    d[30..34].copy_from_slice(&restrictions(s.general.limit_pokemon.as_ref()).to_le_bytes());
    let tweaks = TWEAKS
        .iter()
        .enumerate()
        .filter(|(_, t)| s.misc_tweaks.contains(t))
        .fold(0u32, |v, (i, _)| v | 1 << i);
    d[34..38].copy_from_slice(&tweaks.to_be_bytes());
    d[38] = modifier(tr.level_modifier.unwrap_or(0));
    let sh = &s.shops;
    d[39] = bits(&[
        sh.mode == ShopItemMode::Random,
        sh.mode == ShopItemMode::Shuffle,
        sh.mode == ShopItemMode::Unchanged,
        sh.ban_bad,
        sh.ban_regular,
        sh.ban_overpowered,
        false,
        sh.guarantee_evolution_items,
    ]);
    d[40] = modifier(wild.level_modifier);
    let scope = s.exp_curves.apply_to;
    d[41] = bits(&[
        scope == ExpCurveScope::Legendaries,
        scope == ExpCurveScope::StrongLegendaries,
        scope == ExpCurveScope::All,
        ms.no_game_breaking,
        tms.no_game_breaking,
        tutors.no_game_breaking,
        tr.allow_alt_formes,
        wild.allow_alt_formes,
    ]);
    let [boss, important, regular] = tr.additional_pokemon;
    d[42] = (boss & 0x7) << 1 | (important & 0x7) << 4 | if ab.combine_duplicates { 0x80 } else { 0 };
    let totems = &s.totems;
    d[43] = (regular & 0x7)
        | bits(&[
            totems.aura_mode == AuraMode::Unchanged,
            totems.aura_mode == AuraMode::Random,
            totems.aura_mode == AuraMode::SameStrength,
        ]) << 3
        | if ms.evolution_moves_for_all { 0x40 } else { 0 }
        | if sh.guarantee_x_items { 0x80 } else { 0 };
    d[44] = bits(&[
        totems.mode == TotemMode::Unchanged,
        totems.mode == TotemMode::Random,
        totems.mode == TotemMode::SimilarStrength,
        totems.ally_mode == TotemMode::Unchanged,
        totems.ally_mode == TotemMode::Random,
        totems.ally_mode == TotemMode::SimilarStrength,
        totems.randomize_held_items,
        totems.allow_alt_formes,
    ]);
    d[45] = modifier(totems.level_modifier);
    d[46] = bs.update_to_generation.unwrap_or(0);
    d[47] = md.update_to_generation.unwrap_or(0);
    d[48] = exp_curve_byte(s.exp_curves.standardize.unwrap_or_default());
    d[49] = modifier(statics.level_modifier);
    d[50] = bits(&[
        tr.held_items[0],
        tr.held_items[1],
        tr.held_items[2],
        tr.consumable_items_only,
        tr.sensible_items_only,
        tr.highest_level_gets_item,
        ab.ensure_two,
        tr.local_pokemon,
    ]);
    d[51] = bits(&[
        s.pickup.random,
        !s.pickup.random,
        s.pickup.ban_bad,
        s.general.no_irregular_alt_formes,
    ]);
    let catch_level = if wild.min_catch_rate > 0 {
        wild.min_catch_rate
    } else {
        1
    };
    d[52] = (tr.elite_four_unique.unwrap_or(0) & 0x7) | ((catch_level.clamp(1, 8) - 1) << 3);
    d[53] = bits(&[
        st.types == StarterTypes::None,
        st.types == StarterTypes::FireWaterGrass,
        st.types == StarterTypes::TypeTriangle,
        st.types == StarterTypes::Unique,
        matches!(st.types, StarterTypes::SingleType(_)),
        false,
        st.no_legendaries,
        st.no_dual_types,
    ]);
    d[54] = match st.types {
        StarterTypes::SingleType(Some(t)) => type_index(t) + 1,
        _ => 0,
    };
    let pal = &s.palettes;
    d[55] = bits(&[
        !pal.random,
        pal.random,
        pal.follow_types,
        pal.follow_evolutions,
        pal.shiny_from_normal,
    ]);
    let te = &s.type_effectiveness;
    d[56] = bits(&[
        te.mode == TypeEffectivenessMode::Unchanged,
        te.mode == TypeEffectivenessMode::Random,
        te.mode == TypeEffectivenessMode::RandomBalanced,
        te.mode == TypeEffectivenessMode::KeepIdentities,
        te.mode == TypeEffectivenessMode::Inverse,
        te.random_immunities,
        te.update,
    ]);
    d[57] = bits(&[ev.force_growth, ev.no_convergence, ev.adjust_levels]);
    let [min_high, min_low] = st.bst_min.to_be_bytes();
    let [max_high, max_low] = st.bst_max.to_be_bytes();
    d[58] = (min_high & 0x0F) | (max_high & 0x0F) << 4;
    d[59] = min_low;
    d[60] = max_low;
    d[61] = bits(&[
        tr.diverse_types[0],
        tr.diverse_types[1],
        tr.diverse_types[2],
        tr.better_movesets[0],
        tr.better_movesets[1],
        tr.better_movesets[2],
    ]);
    let style = &s.battle_style;
    d[62] = bits(&[
        style.mode == BattleStyleMode::Unchanged,
        style.mode == BattleStyleMode::Random,
        style.mode == BattleStyleMode::SingleStyle,
    ]) | bits(&[
        style.style == BattleStyle::Single,
        style.style == BattleStyle::Double,
        style.style == BattleStyle::Triple,
        style.style == BattleStyle::Rotation,
    ]) << 3;
    d[63] = bits(&[
        tr.evolve.is_some(),
        s.general.no_premature_evolutions,
        tr.level_modifier.is_some(),
        wild.level_modifier != 0,
        totems.level_modifier != 0,
        statics.level_modifier != 0,
    ]);
    d[64] = bits(&[sh.balance_prices, sh.add_cheap_rare_candies]);
    d[65] = bits(&[
        s.general.random_intro_pokemon,
        extras.race_mode,
        false,
        s.general.limit_pokemon.is_some(),
    ]);
    d[66] = ev.easier_max_level.unwrap_or(EASIER_EVOLUTIONS_DEFAULT);
    let bst = &s.bsts;
    d[67] = bits(&[
        bst.mode == BstMode::Unchanged,
        bst.mode == BstMode::BuffNerf,
        bst.mode == BstMode::Shuffle,
        bst.mode == BstMode::Random,
        bst.follow_evolutions,
        bst.separate_legendaries,
    ]);
    d[68] = bst.max_buff_nerf_percent;
    d
}

/// Reads a settings string without its version.
fn decode(text: &str) -> Result<(Settings, SettingsExtras), SettingsStringError> {
    let data = base64::decode(text.trim()).map_err(|_| Malformed)?;
    if data.len() < DATA_LENGTH + 1 + TRAILER {
        return Err(Malformed);
    }
    let (body, trailer) = data.split_at(data.len() - TRAILER);
    if crc32(body).to_be_bytes() != trailer[..4] {
        return Err(Malformed);
    }
    let name_length = usize::from(body[DATA_LENGTH]);
    let name = body
        .get(DATA_LENGTH + 1..DATA_LENGTH + 1 + name_length)
        .ok_or(Malformed)?;
    let mut d = [0u8; DATA_LENGTH];
    d.copy_from_slice(&body[..DATA_LENGTH]);
    let extras = SettingsExtras {
        rom_name: name.iter().map(|&b| char::from(b)).collect(),
        race_mode: bit(d[65], 1),
    };
    Ok((settings(&d)?, extras))
}

/// The settings in the 69 bytes of options.
#[expect(clippy::too_many_lines, reason = "one statement per option, in the format's order")]
fn settings(d: &[u8; DATA_LENGTH]) -> Result<Settings, SettingsStringError> {
    let mut s = Settings::default();
    let ev = &mut s.evolutions;
    ev.change_impossible = bit(d[0], 0);
    ev.easier_max_level = bit(d[0], 5).then_some(d[66] & 0x7F);
    ev.remove_time_based = bit(d[0], 6);
    ev.use_estimated_levels = bit(d[0], 7);
    ev.mode = choice(
        d[28],
        &[
            (0, EvolutionMode::Unchanged),
            (1, EvolutionMode::Random),
            (7, EvolutionMode::RandomEveryLevel),
        ],
    )?;
    ev.similar_strength = bit(d[28], 2);
    ev.same_typing = bit(d[28], 3);
    ev.limit_three_stages = bit(d[28], 4);
    ev.force_change = bit(d[28], 5);
    ev.allow_alt_formes = bit(d[28], 6);
    ev.force_growth = bit(d[57], 0);
    ev.no_convergence = bit(d[57], 1);
    ev.adjust_levels = bit(d[57], 2);

    let md = &mut s.move_data;
    md.update_to_generation = bit(d[0], 1).then_some(d[47]);
    md.power = bit(d[27], 0);
    md.accuracy = bit(d[27], 1);
    md.pp = bit(d[27], 2);
    md.types = bit(d[27], 3);
    md.category = bit(d[27], 4);
    md.names = bit(d[27], 6);

    let bs = &mut s.base_stats;
    bs.mode = choice(
        d[1],
        &[
            (3, BaseStatMode::Unchanged),
            (2, BaseStatMode::Shuffle),
            (1, BaseStatMode::Random),
        ],
    )?;
    bs.follow_evolutions = bit(d[1], 0);
    bs.follow_megas = bit(d[1], 6);
    bs.update_to_generation = bit(d[1], 5).then_some(d[46]);
    bs.random_evolution_stats = bit(d[1], 7);
    s.exp_curves.standardize = bit(d[1], 4)
        .then(|| EXP_CURVES.get(usize::from(d[48])).copied().ok_or(Malformed))
        .transpose()?;
    s.exp_curves.apply_to = choice(
        d[41],
        &[
            (0, ExpCurveScope::Legendaries),
            (1, ExpCurveScope::StrongLegendaries),
            (2, ExpCurveScope::All),
        ],
    )?;

    s.types.mode = choice(
        d[2],
        &[
            (2, TypeMode::Unchanged),
            (0, TypeMode::FollowEvolutions),
            (1, TypeMode::Random),
        ],
    )?;
    s.types.follow_megas = bit(d[2], 6);
    s.types.force_dual = bit(d[2], 7);

    let ab = &mut s.abilities;
    ab.mode = choice(d[3], &[(0, AbilityMode::Unchanged), (1, AbilityMode::Random)])?;
    ab.allow_wonder_guard = bit(d[3], 2);
    ab.follow_evolutions = bit(d[3], 3);
    ab.ban_trapping = bit(d[3], 4);
    ab.ban_negative = bit(d[3], 5);
    ab.ban_bad = bit(d[3], 6);
    ab.follow_megas = bit(d[3], 7);
    ab.combine_duplicates = bit(d[42], 7);
    ab.ensure_two = bit(d[50], 6);

    let st = &mut s.starters;
    st.mode = choice(
        d[4],
        &[
            (2, StarterMode::Unchanged),
            (0, StarterMode::Custom),
            (1, StarterMode::Random),
            (3, StarterMode::RandomWithTwoEvolutions),
            (7, StarterMode::RandomBasic),
        ],
    )?;
    st.random_held_items = bit(d[4], 4);
    st.ban_bad_items = bit(d[4], 5);
    st.allow_alt_formes = bit(d[4], 6);
    if st.mode == StarterMode::Custom {
        st.custom = (0..3)
            .map(|i| Some(u16::from_le_bytes([d[5 + 2 * i], d[6 + 2 * i]])).filter(|&n| n != 0))
            .collect();
    }
    let single = (d[54] != 0)
        .then(|| {
            usize::from(d[54] & 0x1F)
                .checked_sub(1)
                .and_then(|i| TYPE_ORDER.get(i))
                .copied()
        })
        .flatten();
    st.types = choice(
        d[53],
        &[
            (0, StarterTypes::None),
            (1, StarterTypes::FireWaterGrass),
            (2, StarterTypes::TypeTriangle),
            (3, StarterTypes::Unique),
            (4, StarterTypes::SingleType(single)),
        ],
    )?;
    st.no_legendaries = bit(d[53], 6);
    st.no_dual_types = bit(d[53], 7);
    st.bst_min = u16::from(d[58] & 0x0F) << 8 | u16::from(d[59]);
    st.bst_max = u16::from(d[58] & 0xF0) << 4 | u16::from(d[60]);

    let ms = &mut s.movesets;
    ms.mode = choice(
        d[11],
        &[
            (2, MovesetMode::Unchanged),
            (1, MovesetMode::RandomPreferSameType),
            (0, MovesetMode::RandomCompletely),
            (3, MovesetMode::MetronomeOnly),
        ],
    )?;
    ms.guaranteed_level_one_moves = bit(d[11], 4).then_some((d[11] >> 6) + 2);
    ms.reorder_damaging = bit(d[11], 5);
    ms.good_damaging_percent = read_good_damaging(d[12]);
    ms.no_game_breaking = bit(d[41], 3);
    ms.evolution_moves_for_all = bit(d[43], 6);

    let tr = &mut s.trainers;
    tr.mode = choice(
        d[13],
        &[
            (0, TrainerMode::Unchanged),
            (1, TrainerMode::Random),
            (2, TrainerMode::RandomEvenDistribution),
            (3, TrainerMode::RandomEvenDistributionMainGame),
            (4, TrainerMode::TypeThemed),
            (5, TrainerMode::TypeThemedEliteFourGyms),
            (6, TrainerMode::KeepThemed),
            (7, TrainerMode::KeepThemesOrPrimary),
        ],
    )?;
    tr.random_names = bit(d[0], 3);
    tr.random_class_names = bit(d[0], 4);
    tr.evolve = bit(d[63], 0).then(|| read_modifier(d[14]));
    tr.level_modifier = bit(d[63], 2).then(|| read_modifier(d[38]));
    tr.similar_strength = bit(d[29], 0);
    tr.rival_carries_starter = bit(d[29], 1);
    tr.weight_types = bit(d[29], 2);
    tr.no_legendaries = bit(d[29], 3);
    tr.no_early_wonder_guard = bit(d[29], 4);
    tr.swap_mega_evos = bit(d[29], 5);
    tr.random_shiny = bit(d[29], 6);
    tr.avoid_duplicates = bit(d[29], 7);
    tr.additional_pokemon = [(d[42] & 0x0E) >> 1, (d[42] & 0x70) >> 4, d[43] & 0x07];
    tr.held_items = [bit(d[50], 0), bit(d[50], 1), bit(d[50], 2)];
    tr.consumable_items_only = bit(d[50], 3);
    tr.sensible_items_only = bit(d[50], 4);
    tr.highest_level_gets_item = bit(d[50], 5);
    tr.local_pokemon = bit(d[50], 7);
    tr.allow_alt_formes = bit(d[41], 6);
    tr.elite_four_unique = Some(d[52] & 0x07).filter(|&n| n != 0);
    tr.diverse_types = [bit(d[61], 0), bit(d[61], 1), bit(d[61], 2)];
    tr.better_movesets = [bit(d[61], 3), bit(d[61], 4), bit(d[61], 5)];

    let wild = &mut s.wild;
    wild.mode = if bit(d[15], 0) {
        WildMode::Unchanged
    } else {
        WildMode::Random
    };
    wild.zones = choice(
        d[15],
        &[
            (1, WildZones::Slot),
            (2, WildZones::Set),
            (6, WildZones::Map),
            (5, WildZones::Location),
            (3, WildZones::Game),
        ],
    )?;
    wild.keep_relations = bit(d[15], 4);
    wild.split_by_kind = bit(d[15], 7);
    wild.similar_strength = bit(d[16], 1);
    wild.catch_em_all = bit(d[16], 2);
    wild.types = choice(
        d[17],
        &[
            (0, WildTypes::None),
            (2, WildTypes::ZoneThemes),
            (1, WildTypes::KeepPrimary),
        ],
    )?;
    wild.keep_themes = bit(d[17], 3);
    wild.evolution = choice(
        d[17],
        &[
            (4, WildEvolution::None),
            (5, WildEvolution::BasicOnly),
            (6, WildEvolution::SameStage),
        ],
    )?;
    wild.min_catch_rate = if bit(d[18], 1) { ((d[52] & 0x38) >> 3) + 1 } else { 0 };
    wild.time_of_day = bit(d[18], 0);
    wild.no_legendaries = bit(d[18], 2);
    wild.random_held_items = bit(d[18], 3);
    wild.ban_bad_held_items = bit(d[18], 4);
    wild.balance_low_levels = bit(d[18], 5);
    wild.level_modifier = if bit(d[63], 3) { read_modifier(d[40]) } else { 0 };
    wild.allow_alt_formes = bit(d[41], 7);

    let statics = &mut s.statics;
    statics.mode = choice(
        d[19],
        &[
            (0, StaticMode::Unchanged),
            (1, StaticMode::RandomMatching),
            (2, StaticMode::Random),
            (3, StaticMode::SimilarStrength),
        ],
    )?;
    statics.limit_main_game_legendaries = bit(d[19], 4);
    statics.randomize_600_bst = bit(d[19], 5);
    statics.allow_alt_formes = bit(d[19], 6);
    statics.swap_mega_evos = bit(d[19], 7);
    statics.fix_music = bit(d[27], 5);
    statics.level_modifier = if bit(d[63], 5) { read_modifier(d[49]) } else { 0 };

    let modes = [
        (0, TotemMode::Unchanged),
        (1, TotemMode::Random),
        (2, TotemMode::SimilarStrength),
    ];
    let totems = &mut s.totems;
    totems.mode = choice(d[44], &modes)?;
    totems.ally_mode = choice(d[44] >> 3, &modes)?;
    totems.aura_mode = choice(
        d[43] >> 3,
        &[
            (0, AuraMode::Unchanged),
            (1, AuraMode::Random),
            (2, AuraMode::SameStrength),
        ],
    )?;
    totems.randomize_held_items = bit(d[44], 6);
    totems.allow_alt_formes = bit(d[44], 7);
    totems.level_modifier = if bit(d[63], 4) { read_modifier(d[45]) } else { 0 };

    s.tms.random = choice(d[20], &[(4, false), (3, true)])?;
    s.tms.keep_field_moves = bit(d[20], 6);
    s.tms.no_game_breaking = bit(d[41], 4);
    s.tms.good_damaging_percent = read_good_damaging(d[22]);
    let compat = [
        (2, CompatMode::Unchanged),
        (1, CompatMode::RandomPreferSameType),
        (0, CompatMode::RandomCompletely),
        (7, CompatMode::Full),
    ];
    s.tm_hm_compat.mode = choice(d[20], &compat)?;
    s.tm_hm_compat.level_up_sanity = bit(d[20], 5);
    s.tm_hm_compat.full_hm = bit(d[21], 0);
    s.tm_hm_compat.follow_evolutions = bit(d[21], 1);

    s.tutors.random = choice(d[23], &[(4, false), (3, true)])?;
    s.tutors.keep_field_moves = bit(d[23], 6);
    s.tutors.no_game_breaking = bit(d[41], 5);
    s.tutors.good_damaging_percent = read_good_damaging(d[24]);
    s.tutor_compat.mode = choice(d[23], &compat)?;
    s.tutor_compat.level_up_sanity = bit(d[23], 5);
    s.tutor_compat.follow_evolutions = bit(d[21], 2);

    let trades = &mut s.trades;
    trades.mode = choice(
        d[25],
        &[
            (6, TradeMode::Unchanged),
            (1, TradeMode::Given),
            (0, TradeMode::GivenAndRequested),
        ],
    )?;
    trades.random_items = bit(d[25], 2);
    trades.random_ivs = bit(d[25], 3);
    trades.random_nicknames = bit(d[25], 4);
    trades.random_ots = bit(d[25], 5);

    s.field_items.mode = choice(
        d[26],
        &[
            (2, FieldItemMode::Unchanged),
            (1, FieldItemMode::Shuffle),
            (0, FieldItemMode::Random),
            (4, FieldItemMode::RandomEven),
        ],
    )?;
    s.field_items.ban_bad = bit(d[26], 3);

    let sh = &mut s.shops;
    sh.mode = choice(
        d[39],
        &[
            (2, ShopItemMode::Unchanged),
            (1, ShopItemMode::Shuffle),
            (0, ShopItemMode::Random),
        ],
    )?;
    sh.ban_bad = bit(d[39], 3);
    sh.ban_regular = bit(d[39], 4);
    sh.ban_overpowered = bit(d[39], 5);
    sh.guarantee_evolution_items = bit(d[39], 7);
    sh.guarantee_x_items = bit(d[43], 7);
    sh.balance_prices = bit(d[64], 0);
    sh.add_cheap_rare_candies = bit(d[64], 1);

    s.pickup.random = choice(d[51], &[(1, false), (0, true)])?;
    s.pickup.ban_bad = bit(d[51], 2);
    s.general.no_irregular_alt_formes = bit(d[51], 3);

    s.palettes.random = choice(d[55], &[(0, false), (1, true)])?;
    s.palettes.follow_types = bit(d[55], 2);
    s.palettes.follow_evolutions = bit(d[55], 3);
    s.palettes.shiny_from_normal = bit(d[55], 4);

    let te = &mut s.type_effectiveness;
    te.mode = choice(
        d[56],
        &[
            (0, TypeEffectivenessMode::Unchanged),
            (1, TypeEffectivenessMode::Random),
            (2, TypeEffectivenessMode::RandomBalanced),
            (3, TypeEffectivenessMode::KeepIdentities),
            (4, TypeEffectivenessMode::Inverse),
        ],
    )?;
    te.random_immunities = bit(d[56], 5);
    te.update = bit(d[56], 6);

    s.battle_style.mode = choice(
        d[62],
        &[
            (0, BattleStyleMode::Unchanged),
            (1, BattleStyleMode::Random),
            (2, BattleStyleMode::SingleStyle),
        ],
    )?;
    s.battle_style.style = choice(
        d[62],
        &[
            (3, BattleStyle::Single),
            (4, BattleStyle::Double),
            (5, BattleStyle::Triple),
            (6, BattleStyle::Rotation),
        ],
    )?;

    s.general.no_premature_evolutions = bit(d[63], 1);
    s.general.random_intro_pokemon = bit(d[65], 0);
    let limit = u32::from_le_bytes([d[30], d[31], d[32], d[33]]);
    s.general.limit_pokemon = bit(d[65], 3).then(|| read_restrictions(limit));
    let tweaks = u32::from_be_bytes([d[34], d[35], d[36], d[37]]);
    s.misc_tweaks = TWEAKS
        .iter()
        .enumerate()
        .filter(|&(i, _)| tweaks >> i & 1 == 1)
        .map(|(_, &t)| t)
        .collect();

    let bst = &mut s.bsts;
    bst.mode = choice(
        d[67],
        &[
            (0, BstMode::Unchanged),
            (1, BstMode::BuffNerf),
            (2, BstMode::Shuffle),
            (3, BstMode::Random),
        ],
    )?;
    bst.follow_evolutions = bit(d[67], 4);
    bst.separate_legendaries = bit(d[67], 5);
    bst.max_buff_nerf_percent = d[68];
    Ok(s)
}

/// The settings string: the version, then the settings.
#[must_use]
pub fn to_settings_string(settings: &Settings, extras: &SettingsExtras) -> String {
    format!("{SETTINGS_VERSION}{}", encode(settings, extras))
}

fn check_version(version: u32) -> Result<(), SettingsStringError> {
    match version {
        SETTINGS_VERSION => Ok(()),
        v if v > SETTINGS_VERSION => Err(SettingsStringError::NewerVersion(v)),
        v => Err(SettingsStringError::OldVersion(v)),
    }
}

/// Settings from a string that starts with its three-digit version.
pub fn from_settings_string(text: &str) -> Result<(Settings, SettingsExtras), SettingsStringError> {
    let text = text.trim();
    let version = text.get(..3).and_then(|v| v.parse::<u32>().ok()).ok_or(Malformed)?;
    check_version(version)?;
    decode(&text[3..])
}

/// The bytes of a `.rnqs` file.
#[must_use]
pub fn to_settings_file(settings: &Settings, extras: &SettingsExtras) -> Vec<u8> {
    let text = encode(settings, extras);
    let mut out = Vec::with_capacity(text.len() + 8);
    out.extend_from_slice(&SETTINGS_VERSION.to_be_bytes());
    out.extend_from_slice(&u32::try_from(text.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    out
}

/// Settings from a `.rnqs` file.
pub fn from_settings_file(bytes: &[u8]) -> Result<(Settings, SettingsExtras), SettingsStringError> {
    let word = |at: usize| -> Result<u32, SettingsStringError> {
        let b = bytes.get(at..at + 4).ok_or(Malformed)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let version = word(0)?;
    let major = version >> 24;
    if major > 0 && major <= TOO_OLD {
        return Err(SettingsStringError::OldVersion(version));
    }
    check_version(version)?;
    let length = usize::try_from(word(4)?).map_err(|_| Malformed)?;
    let text = bytes.get(8..8 + length).ok_or(Malformed)?;
    decode(std::str::from_utf8(text).map_err(|_| Malformed)?)
}

#[cfg(test)]
mod tests;
