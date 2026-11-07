//! Static encounters: the Pokémon in scripts (with the Game Corner prizes), the Pokémon traded in scripts of
//! `HeartGold` and `SoulSilver`, the Mystery Egg, the fossils and the roamers, in that order; and the music of the
//! special ones.
//!
//! Static encounters keep their species, forme and levels in scripts; a Game Corner prize's name is also in a
//! text. The roamers keep their species and levels in the ARM9 code, which first needs a patch: in Diamond and
//! Pearl one that loads Cresselia's number from a constant, in Platinum, `HeartGold` and `SoulSilver` a new
//! roamer routine in the ARM9's added room. The Mystery Egg command of `HeartGold` and `SoulSilver` stores its
//! species in a byte; it is rewritten to hold a species and an extra move in words.
//!
//! Deliberate choices:
//! - Roamers are read as the patched code would hold them, but the patch is only written when a roamer changes,
//!   so an unchanged game saves byte for byte.
//! - Only the encounters that changed are written.
//! - The fossils share one level; when fossil levels change, the last changed one is written.
//! - The Mystery Egg reads as level 1, as eggs hatch at level 1.
//! - Platinum's Distortion World fix for Pokémon without an intro of their own is written only when static
//!   encounters change.
//! - The Pokémon traded in scripts name their new species in the old one's place, by the old species' name as
//!   loaded.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, GameData, Move, MoveCategory, MoveId, Species, StaticEncounter};

use super::constants::{self as c, Family};
use super::entry::StaticKind;
use super::files::{byte, find_after, long, put, put_long, put_word, word};
use super::{Rom, ips_patch};
use crate::RomError;

/// The Mystery Egg command: its length as rewritten, where the rewritten one differs from the original, and the
/// byte there.
const MYSTERY_EGG_COMMAND_LENGTH: usize = 216;
const MYSTERY_EGG_IDENTIFIER_OFFSET: usize = 30;
const MYSTERY_EGG_IDENTIFIER: u8 = 0x57;
/// The jump table of Platinum's field overlay that picks the encounter intro graphic, after this prefix; ground
/// type 23 (the Distortion World) takes the grass branch.
const DISTORTION_WORLD_GROUND_CHECK_PREFIX: &str = "23D849187944C988090409148F44";
const DISTORTION_WORLD_GROUND_TYPE: usize = 23;
const GRASS_BRANCH: u8 = 0x30;
const FOSSIL_COUNT: usize = 7;
const CRESSELIA: DexNumber = 488;
const SPEAROW: DexNumber = 21;
/// The gender ratio of female-only species.
const FEMALE_ONLY: u8 = 0xFE;
const STAB: f64 = 1.5;

/// A Pokémon traded in a script: its trade, and the script and offset of its level.
#[derive(Debug, Clone, Copy)]
struct ScriptTrade {
    trade: usize,
    script: usize,
    level_at: usize,
}

/// Which static encounters the game has, in the order of [`GameData::statics`].
#[derive(Debug, Clone, Default)]
pub(super) struct Statics {
    supported: bool,
    plain: usize,
    trades: Vec<ScriptTrade>,
    mystery_egg: bool,
    fossils: bool,
    roamers: usize,
}

impl Statics {
    /// The indices of the fossils in [`GameData::statics`].
    pub(super) fn fossils(&self) -> Vec<usize> {
        if !self.fossils {
            return Vec::new();
        }
        let start = self.plain + self.trades.len() + usize::from(self.mystery_egg);
        (start..start + FOSSIL_COUNT).collect()
    }
}

/// Whether the roamers can be changed: Diamond and Pearl with roamers in the entry, the others with the new roamer
/// routine.
fn roamers_enabled(rom: &Rom) -> bool {
    match rom.family() {
        Family::DiamondPearl => !rom.entry.roamers.is_empty(),
        _ => rom.entry.has_tweak("NewRoamerSubroutineTweak"),
    }
}

/// Reads the static encounters.
#[expect(clippy::too_many_lines, reason = "one pass over each kind of static the entry lists")]
pub(super) fn read(rom: &mut Rom) -> Result<(Statics, Vec<StaticEncounter>), RomError> {
    if !rom.entry.static_support {
        return Ok((Statics::default(), Vec::new()));
    }
    let scripts = rom.entry.file("Scripts")?;
    let eggs = rom.entry.array("StaticEggPokemonOffsets");
    let mut out = Vec::new();
    let slots = rom.entry.statics.clone();
    for (i, slot) in slots.iter().enumerate() {
        let first = slot
            .species
            .first()
            .ok_or_else(|| RomError::Malformed("a static encounter without species".into()))?;
        let species = word(rom.files.narc_file(&scripts, first.file)?, first.offset)?;
        let forme = match &slot.kind {
            StaticKind::Plain { forme } => match forme.first() {
                Some(at) => byte(rom.files.narc_file(&scripts, at.file)?, at.offset)?,
                None => 0,
            },
            StaticKind::GameCorner { .. } => 0,
        };
        let mut levels = Vec::new();
        for l in &slot.levels {
            levels.push(byte(rom.files.narc_file(&scripts, l.file)?, l.offset)?);
        }
        let family = rom.family();
        let mut se = StaticEncounter::new(
            c::forme_species(family, species, forme),
            levels.first().copied().unwrap_or(1),
        );
        se.forme = forme;
        se.egg = eggs.contains(&i);
        se.linked_levels = levels.into_iter().skip(1).collect();
        out.push(se);
    }

    let trade_numbers = rom.entry.array("StaticPokemonTrades");
    let trade_scripts = rom.entry.array("StaticPokemonTradeScripts");
    let trade_levels = rom.entry.array("StaticPokemonTradeLevelOffsets");
    let mut trades = Vec::new();
    if !trade_numbers.is_empty() {
        let path = rom.entry.file("InGameTrades")?;
        for (i, &trade) in trade_numbers.iter().enumerate() {
            let (Some(&script), Some(&level_at)) = (trade_scripts.get(i), trade_levels.get(i)) else {
                return Err(RomError::Malformed(
                    "a traded static encounter without its script".into(),
                ));
            };
            let level = byte(rom.files.narc_file(&scripts, script)?, level_at)?;
            let species = long(rom.files.narc_file(&path, trade)?, 0)?;
            out.push(StaticEncounter::new(dex(species)?, level));
            trades.push(ScriptTrade {
                trade,
                script,
                level_at,
            });
        }
    }

    let mystery_egg = rom.entry.int("MysteryEggOffset") > 0;
    if mystery_egg {
        let command = rom.entry.need("MysteryEggCommandOffset")?;
        let egg_at = rom.entry.need("MysteryEggOffset")?;
        let overlay = rom.files.overlay(rom.entry.need("FieldOvlNumber")?)?;
        let species = if improved_mystery_egg(overlay, command) {
            dex(long(overlay, command + MYSTERY_EGG_COMMAND_LENGTH)?)?
        } else {
            DexNumber::from(byte(overlay, egg_at)?)
        };
        let mut se = StaticEncounter::new(species, 1);
        se.egg = true;
        out.push(se);
    }

    let fossils = rom.entry.int("FossilTableOffset") > 0;
    if fossils {
        let level = fossil_level_byte(rom).map(|(file, at)| byte(file, at))??;
        let table = rom.entry.need("FossilTableOffset")?;
        let data = fossil_table(rom)?;
        for f in 0..FOSSIL_COUNT {
            out.push(StaticEncounter::new(word(data, table + 2 + f * 4)?, level));
        }
    }

    let roamers = if roamers_enabled(rom) {
        let mut arm9 = rom.files.arm9_ref().to_vec();
        patch_roamer_code(rom, &mut arm9)?;
        for r in &rom.entry.roamers {
            let at = *r
                .species_code
                .first()
                .ok_or_else(|| RomError::Malformed("a roamer without species".into()))?;
            let level = match r.level_code.first() {
                Some(&l) => byte(&arm9, l)?,
                None => 1,
            };
            out.push(StaticEncounter::new(word(&arm9, at)?, level));
        }
        rom.entry.roamers.len()
    } else {
        0
    };

    let statics = Statics {
        supported: true,
        plain: slots.len(),
        trades,
        mystery_egg,
        fossils,
        roamers,
    };
    Ok((statics, out))
}

fn dex(value: u32) -> Result<DexNumber, RomError> {
    DexNumber::try_from(value).map_err(|_| RomError::Malformed(format!("species {value}")))
}

/// The script file holding the fossils' level, and where in it.
fn fossil_level_byte(rom: &mut Rom) -> Result<(&mut Vec<u8>, usize), RomError> {
    let scripts = rom.entry.file("Scripts")?;
    let number = rom.entry.need("FossilLevelScriptNumber")?;
    let at = rom.entry.need("FossilLevelOffset")?;
    Ok((rom.files.narc_file(&scripts, number)?, at))
}

/// The data holding the fossil table: an overlay in `HeartGold` and `SoulSilver`, the ARM9 otherwise.
fn fossil_table(rom: &mut Rom) -> Result<&mut Vec<u8>, RomError> {
    if rom.family() == Family::HeartGoldSoulSilver {
        let number = rom.entry.need("FossilTableOvlNumber")?;
        rom.files.overlay(number)
    } else {
        Ok(rom.files.arm9())
    }
}

fn improved_mystery_egg(overlay: &[u8], command: usize) -> bool {
    overlay.get(command + MYSTERY_EGG_IDENTIFIER_OFFSET) == Some(&MYSTERY_EGG_IDENTIFIER)
}

/// Patches the roamer code of `arm9` when it isn't yet: Diamond and Pearl load Cresselia's number from the
/// constant of an unused Darkrai entry; the others get the new roamer routine.
fn patch_roamer_code(rom: &Rom, arm9: &mut [u8]) -> Result<(), RomError> {
    if rom.family() == Family::DiamondPearl {
        let start = rom.entry.need("RoamingPokemonFunctionStartOffset")?;
        // The original code shifts Cresselia's number into r7 here; the patched code has a nop.
        if word(arm9, start + 44)? != 0 {
            put_long(arm9, start + 244, u32::from(CRESSELIA))?;
            // ldr r7, [pc, #0xC8]; nop
            put(arm9, start + 42, &[0x32, 0x4F, 0x00, 0x00])?;
        }
    } else if let Some(&first) = rom.entry.roamers.first().and_then(|r| r.species_code.first())
        && (arm9.len() < first + 2 || word(arm9, first)? == 0)
    {
        ips_patch(&rom.entry, arm9, "NewRoamerSubroutineTweak")?;
    }
    Ok(())
}

/// Writes the static encounters that changed.
pub(super) fn write(rom: &mut Rom, statics: &Statics, loaded: &GameData, data: &GameData) -> Result<(), RomError> {
    if !statics.supported || data.statics == loaded.statics {
        return Ok(());
    }
    let expected = statics.plain
        + statics.trades.len()
        + usize::from(statics.mystery_egg)
        + if statics.fossils { FOSSIL_COUNT } else { 0 }
        + statics.roamers;
    if data.statics.len() != expected {
        return Err(RomError::Malformed(format!(
            "this game has {expected} static encounters, got {}",
            data.statics.len()
        )));
    }
    let pairs: Vec<(&StaticEncounter, &StaticEncounter)> = loaded.statics.iter().zip(&data.statics).collect();
    let (plain, rest) = pairs.split_at(statics.plain);
    let (trades, rest) = rest.split_at(statics.trades.len());
    let (egg, rest) = rest.split_at(usize::from(statics.mystery_egg));
    let (fossils, roamers) = rest.split_at(if statics.fossils { FOSSIL_COUNT } else { 0 });

    write_plain(rom, plain, data)?;
    write_script_trades(rom, &statics.trades, trades, loaded, data)?;
    if let [(old, new)] = egg
        && old != new
    {
        write_mystery_egg(rom, new.species, data)?;
    }
    write_fossils(rom, fossils)?;
    write_roamers(rom, roamers)?;
    if rom.family() == Family::Platinum {
        patch_distortion_world(rom)?;
    }
    Ok(())
}

fn species_of(data: &GameData, number: DexNumber) -> Result<&Species, RomError> {
    data.species(number)
        .ok_or_else(|| RomError::Malformed(format!("no species {number}")))
}

fn write_plain(rom: &mut Rom, pairs: &[(&StaticEncounter, &StaticEncounter)], data: &GameData) -> Result<(), RomError> {
    let scripts = rom.entry.file("Scripts")?;
    let slots = rom.entry.statics.clone();
    for (slot, (old, new)) in slots.iter().zip(pairs) {
        if old == new {
            continue;
        }
        if (old.species, old.forme) != (new.species, new.forme) {
            // The species is stored as its base species and forme.
            for at in &slot.species {
                put_word(
                    rom.files.narc_file(&scripts, at.file)?,
                    at.offset,
                    c::base_species(new.species),
                )?;
            }
            match &slot.kind {
                StaticKind::Plain { forme } => {
                    for at in forme {
                        put(rom.files.narc_file(&scripts, at.file)?, at.offset, &[new.forme])?;
                    }
                }
                StaticKind::GameCorner { text } => {
                    let name = species_of(data, new.species)?.name.to_uppercase();
                    for t in text {
                        let mut strings = rom.files.strings(t.file)?;
                        if let Some(s) = strings.get_mut(t.offset)
                            && let Some(end) = s.find('\\')
                        {
                            *s = format!("{name}{}", &s[end..]);
                        }
                        rom.files.set_strings(t.file, strings)?;
                    }
                }
            }
        }
        let levels = std::iter::once(new.level).chain(new.linked_levels.iter().copied());
        for (at, level) in slot.levels.iter().zip(levels) {
            put(rom.files.narc_file(&scripts, at.file)?, at.offset, &[level])?;
        }
    }
    Ok(())
}

/// The Pokémon traded in scripts: species and first ability in the trade, level in the script, and the second
/// one (Kenya) by name in its text.
fn write_script_trades(
    rom: &mut Rom,
    trades: &[ScriptTrade],
    pairs: &[(&StaticEncounter, &StaticEncounter)],
    loaded: &GameData,
    data: &GameData,
) -> Result<(), RomError> {
    let path = rom.entry.file("InGameTrades")?;
    let scripts = rom.entry.file("Scripts")?;
    for (i, (t, (old, new))) in trades.iter().zip(pairs).enumerate() {
        if old == new {
            continue;
        }
        let species = species_of(data, new.species)?;
        let file = rom.files.narc_file(&path, t.trade)?;
        put_long(file, 0, u32::from(new.species))?;
        put_long(file, 0x1C, u32::from(species.abilities.first().copied().unwrap_or(0)))?;
        put(rom.files.narc_file(&scripts, t.script)?, t.level_at, &[new.level])?;
        if i == 1 && old.species != new.species {
            let from = species_of(loaded, SPEAROW)?.name.to_uppercase();
            let replacements = BTreeMap::from([(from, species.name.clone())]);
            let text = rom.entry.need("KenyaTextOffset")?;
            let line = rom.chars_per_line();
            rom.files.replace_names(text, &replacements, line)?;
        }
    }
    Ok(())
}

/// The Mystery Egg: the command rewritten to hold words if it isn't yet, then the species and an extra move.
fn write_mystery_egg(rom: &mut Rom, species: DexNumber, data: &GameData) -> Result<(), RomError> {
    let command = rom.entry.need("MysteryEggCommandOffset")?;
    let extra = mystery_egg_move(data, species).unwrap_or(0);
    let overlay = rom.files.overlay(rom.entry.need("FieldOvlNumber")?)?;
    if !improved_mystery_egg(overlay, command) {
        let original = overlay
            .get(command..command + MYSTERY_EGG_COMMAND_LENGTH)
            .ok_or_else(|| RomError::Malformed("the Mystery Egg command is cut short".into()))?;
        let improved = improve_mystery_egg_command(original)?;
        put(overlay, command, &improved)?;
    }
    put_long(overlay, command + MYSTERY_EGG_COMMAND_LENGTH, u32::from(species))?;
    put_long(overlay, command + MYSTERY_EGG_COMMAND_LENGTH + 4, u32::from(extra))
}

/// The Mystery Egg command rewritten to load the species and the extra move as words from after it.
fn improve_mystery_egg_command(original: &[u8]) -> Result<Vec<u8>, RomError> {
    let mut code = thumb::Code::new(original)?;
    code.insert(
        216,
        &[0x04, thumb::ADD_SP_IMM7, 0xF8, thumb::POP_PC, 0x00, thumb::LSL_IMM_0],
    )?;
    code.set(108, 0x1D, thumb::LDR_PC_R1)?;
    code.set(96, 0x01, thumb::ADD_I8_R4)?;
    code.set(66, 0x26, thumb::LDR_PC_R1)?;
    code.remove(30, 4)?;
    code.insert(30, &[0x57, thumb::BGE])?;
    Ok(code.bytes())
}

/// The extra move of the Mystery Egg's Pokémon: the strongest of its egg moves (its own and its earlier stages'),
/// else of its tutor moves, else of its TM moves; none without any.
fn mystery_egg_move(data: &GameData, number: DexNumber) -> Option<MoveId> {
    let species = data.species(number)?;
    let mut family = BTreeSet::from([number]);
    let mut stack = vec![number];
    while let Some(n) = stack.pop() {
        for e in data.evolutions.iter().filter(|e| e.to == n) {
            if family.insert(e.from) {
                stack.push(e.from);
            }
        }
    }
    let mut moves: BTreeSet<MoveId> = family
        .iter()
        .filter_map(|n| data.egg_moves.get(n))
        .flatten()
        .copied()
        .collect();
    let compatible = |list: &[MoveId], compat: Option<&Vec<bool>>| -> BTreeSet<MoveId> {
        list.iter()
            .enumerate()
            .filter(|&(i, _)| compat.and_then(|c| c.get(i)).copied().unwrap_or(false))
            .map(|(_, &m)| m)
            .collect()
    };
    if moves.is_empty() {
        moves = compatible(&data.tutor_moves, data.tutor_compat.get(&number));
    }
    if moves.is_empty() {
        moves = compatible(&data.tm_moves, data.tm_hm_compat.get(&number));
    }
    let mut candidates: Vec<&Move> = moves
        .iter()
        .filter_map(|&m| data.moves.iter().find(|mv| mv.number == m))
        .collect();
    candidates.sort_by(|a, b| strength(a, species).total_cmp(&strength(b, species)));
    candidates.last().map(|m| m.number)
}

/// A move's expected damage when used by `species`: power, hits, accuracy, the attacking stat and STAB; 0 for
/// status moves.
fn strength(m: &Move, species: &Species) -> f64 {
    if m.category == MoveCategory::Status {
        return 0.0;
    }
    let stat = if m.category == MoveCategory::Physical {
        species.stats.attack
    } else {
        species.stats.sp_attack
    };
    let mut s =
        f64::from(m.power) * f64::from(m.hit_count_hundredths) / 100.0 * f64::from(m.accuracy) * f64::from(stat);
    if m.move_type.is_some_and(|t| species.has_type(t)) {
        s *= STAB;
    }
    s
}

fn write_fossils(rom: &mut Rom, pairs: &[(&StaticEncounter, &StaticEncounter)]) -> Result<(), RomError> {
    if pairs.iter().all(|(o, n)| o == n) {
        return Ok(());
    }
    let table = rom.entry.need("FossilTableOffset")?;
    let data = fossil_table(rom)?;
    for (f, (old, new)) in pairs.iter().enumerate() {
        if old.species != new.species {
            put_word(data, table + 2 + f * 4, new.species)?;
        }
    }
    if let Some((_, new)) = pairs.iter().rev().find(|(o, n)| o.level != n.level) {
        let level = new.level;
        let (file, at) = fossil_level_byte(rom)?;
        put(file, at, &[level])?;
    }
    Ok(())
}

/// The roamers: species in the code and scripts, gender in scripts (female for female-only species) and levels in
/// the code, after the roamer patch.
fn write_roamers(rom: &mut Rom, pairs: &[(&StaticEncounter, &StaticEncounter)]) -> Result<(), RomError> {
    if pairs.iter().all(|(o, n)| o == n) {
        return Ok(());
    }
    let mut arm9 = std::mem::take(rom.files.arm9());
    let patched = patch_roamer_code(rom, &mut arm9);
    *rom.files.arm9() = arm9;
    patched?;
    let scripts = rom.entry.file("Scripts")?;
    let roamers = rom.entry.roamers.clone();
    for (r, (old, new)) in roamers.iter().zip(pairs) {
        if old == new {
            continue;
        }
        let gender = u16::from(byte(rom.stats(new.species)?, c::BS_GENDER_RATIO)? == FEMALE_ONLY);
        for &at in &r.species_code {
            put_word(rom.files.arm9(), at, new.species)?;
        }
        for at in &r.script {
            put_word(rom.files.narc_file(&scripts, at.file)?, at.offset, new.species)?;
        }
        for at in &r.gender {
            put_word(rom.files.narc_file(&scripts, at.file)?, at.offset, gender)?;
        }
        for &at in &r.level_code {
            put(rom.files.arm9(), at, &[new.level])?;
        }
    }
    Ok(())
}

/// Ground type 23 never sets the encounter intro graphic, which Giratina's own intro hides; a replacement
/// without one would crash, so the ground type takes the grass branch.
fn patch_distortion_world(rom: &mut Rom) -> Result<(), RomError> {
    let overlay = rom.files.overlay(rom.entry.need("FieldOvlNumber")?)?;
    if let Some(at) = find_after(overlay, DISTORTION_WORLD_GROUND_CHECK_PREFIX) {
        put(overlay, at + 2 * DISTORTION_WORLD_GROUND_TYPE, &[GRASS_BRANCH])?;
    }
    Ok(())
}

/// Whether the game's special music can follow its replacements.
pub(super) fn has_music_fix(rom: &Rom) -> bool {
    rom.entry.has_tweak("NewIndexToMusicTweak") || rom.family() == Family::HeartGoldSoulSilver
}

/// The species with special battle music, when it can follow their replacements.
pub(super) fn special_music(rom: &Rom) -> Vec<DexNumber> {
    if !has_music_fix(rom) {
        return Vec::new();
    }
    rom.entry
        .array("SpecialMusicStatics")
        .into_iter()
        .filter_map(|n| DexNumber::try_from(n).ok())
        .collect()
}

/// The special music of each replaced species now plays for its replacement: in `HeartGold` and `SoulSilver` the
/// species of the music table (the low ten bits of each word) change; the others first get a new table.
pub(super) fn correct_music(
    rom: &mut Rom,
    special: &[DexNumber],
    old: &[StaticEncounter],
    new: &[StaticEncounter],
) -> Result<(), RomError> {
    // One replacement per species, the last one wins.
    let changes: BTreeMap<DexNumber, DexNumber> = old
        .iter()
        .zip(new)
        .filter(|(o, _)| special.contains(&o.species))
        .map(|(o, n)| (o.species, n.species))
        .collect();
    if changes.is_empty() || !has_music_fix(rom) {
        return Ok(());
    }
    let hgss = rom.family() == Family::HeartGoldSoulSilver;
    let (prefix, step, mask) = if hgss {
        (rom.entry.string("IndexToMusicPrefix").to_owned(), 2, 0x3FF)
    } else {
        let mut arm9 = std::mem::take(rom.files.arm9());
        let patched = ips_patch(&rom.entry, &mut arm9, "NewIndexToMusicTweak");
        *rom.files.arm9() = arm9;
        patched?;
        (rom.entry.string("NewIndexToMusicPrefix").to_owned(), 4, 0xFFFF)
    };
    let arm9 = rom.files.arm9();
    let Some(start) = find_after(arm9, &prefix) else {
        return Ok(());
    };
    let mut replaced = Vec::new();
    for (old, new) in changes {
        let found = (start..arm9.len().saturating_sub(1))
            .step_by(step)
            .find(|&at| !replaced.contains(&at) && word(arm9, at).is_ok_and(|w| w & mask == old));
        let Some(at) = found else {
            continue;
        };
        let w = word(arm9, at)?;
        put_word(arm9, at, new | (w & !mask))?;
        replaced.push(at);
    }
    Ok(())
}

/// Thumb code as a list of instructions, which can be inserted and removed while the branches with links that
/// pass over the change keep their destinations.
mod thumb {
    use crate::RomError;

    pub(super) const LSL_IMM_0: u8 = 0x00;
    pub(super) const LDR_PC_R1: u8 = 0x49;
    pub(super) const ADD_I8_R4: u8 = 0x34;
    pub(super) const ADD_SP_IMM7: u8 = 0xB0;
    pub(super) const POP_PC: u8 = 0xBD;
    pub(super) const BGE: u8 = 0xDA;

    /// One instruction: low and high byte.
    #[derive(Debug, Clone, Copy)]
    struct Instruction {
        low: u8,
        high: u8,
    }

    impl Instruction {
        fn is_bl_setup(self) -> bool {
            (0xF0..=0xF7).contains(&self.high)
        }

        fn is_bl_offset(self) -> bool {
            self.high >= 0xF8
        }
    }

    #[derive(Debug, Clone)]
    pub(super) struct Code {
        instructions: Vec<Instruction>,
    }

    fn instructions(bytes: &[u8]) -> Vec<Instruction> {
        let (pairs, _) = bytes.as_chunks::<2>();
        pairs.iter().map(|&[low, high]| Instruction { low, high }).collect()
    }

    fn bad(why: &str) -> RomError {
        RomError::Malformed(format!("Thumb code: {why}"))
    }

    impl Code {
        pub(super) fn new(bytes: &[u8]) -> Result<Self, RomError> {
            if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
                return Err(bad("an even, non-zero number of bytes is needed"));
            }
            Ok(Self {
                instructions: instructions(bytes),
            })
        }

        pub(super) fn bytes(&self) -> Vec<u8> {
            self.instructions.iter().flat_map(|i| [i.low, i.high]).collect()
        }

        /// Inserts instructions (pairs of low and high byte) at byte offset `at`.
        pub(super) fn insert(&mut self, at: usize, bytes: &[u8]) -> Result<(), RomError> {
            if !at.is_multiple_of(2) || at > self.instructions.len() * 2 || !bytes.len().is_multiple_of(2) {
                return Err(bad("bad insertion"));
            }
            let new = instructions(bytes);
            let count = new.len();
            self.instructions.splice(at / 2..at / 2, new);
            self.shift_branches(at + bytes.len(), i32::try_from(count).map_err(|_| bad("too long"))?)
        }

        /// Removes `count` instructions at byte offset `at`.
        pub(super) fn remove(&mut self, at: usize, count: usize) -> Result<(), RomError> {
            if !at.is_multiple_of(2) || self.instructions.len() < at / 2 + count {
                return Err(bad("bad removal"));
            }
            self.instructions.drain(at / 2..at / 2 + count);
            self.shift_branches(at, -i32::try_from(count).map_err(|_| bad("too long"))?)
        }

        /// Replaces the instruction at byte offset `at`.
        pub(super) fn set(&mut self, at: usize, low: u8, high: u8) -> Result<(), RomError> {
            let i = self
                .instructions
                .get_mut(at / 2)
                .filter(|_| at.is_multiple_of(2))
                .ok_or_else(|| bad("bad offset"))?;
            *i = Instruction { low, high };
            Ok(())
        }

        /// Corrects the branches with link from byte offset `start` on, which moved by `shift` instructions,
        /// unless their destination moved with them.
        fn shift_branches(&mut self, start: usize, shift: i32) -> Result<(), RomError> {
            let len = self.instructions.len();
            let mut offset = i32::try_from(start).map_err(|_| bad("too long"))?;
            let start_offset = offset;
            for i in start / 2..len {
                offset += 2;
                let setup = self.instructions[i];
                if !setup.is_bl_setup() {
                    continue;
                }
                let second = *self
                    .instructions
                    .get(i + 1)
                    .filter(|s| s.is_bl_offset())
                    .ok_or_else(|| bad("a branch with link cut in half"))?;
                let displacement = displacement(setup, second) + shift;
                let destination = offset + displacement;
                let moved_along = destination >= start_offset
                    && destination / 2 <= i32::try_from(len).map_err(|_| bad("too long"))?;
                if !moved_along {
                    let (a, b) = encode(displacement)?;
                    self.instructions[i] = a;
                    self.instructions[i + 1] = b;
                }
            }
            Ok(())
        }
    }

    /// The displacement of a branch with link, the sign bit taken as a sign.
    fn displacement(setup: Instruction, second: Instruction) -> i32 {
        let d = (i32::from(setup.high & 0b11) << 19)
            + (i32::from(setup.low) << 11)
            + (i32::from(second.high & 0b111) << 8)
            + i32::from(second.low);
        if setup.high & 0b100 != 0 { -d } else { d }
    }

    fn encode(displacement: i32) -> Result<(Instruction, Instruction), RomError> {
        let sign = u8::from(displacement <= 0);
        let d = displacement.unsigned_abs();
        if d >= 2 << 22 {
            return Err(bad("a branch too far"));
        }
        let byte = |v: u32| u8::try_from(v & 0xFF).unwrap_or_default();
        Ok((
            Instruction {
                low: byte(d >> 11),
                high: 0xF0 + (sign << 2) + byte((d >> 19) & 0b11),
            },
            Instruction {
                low: byte(d),
                high: 0xF8 + byte((d >> 8) & 0b111),
            },
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn branches_over_a_removal_keep_their_destination() {
            // A branch with link at 16 back over an instruction that is removed: its displacement changes by the
            // instruction removed, in its sign-and-magnitude form; the code keeps its length after an insertion.
            let mut bytes = vec![0u8; 24];
            let (a, b) = encode(-100).expect("fits");
            bytes[16..20].copy_from_slice(&[a.low, a.high, b.low, b.high]);
            let mut code = Code::new(&bytes).expect("code");
            code.remove(2, 1).expect("removes");
            let out = code.bytes();
            let setup = Instruction {
                low: out[14],
                high: out[15],
            };
            let second = Instruction {
                low: out[16],
                high: out[17],
            };
            assert_eq!(displacement(setup, second), -101);
            code.insert(2, &[1, 2]).expect("inserts");
            assert_eq!(code.bytes().len(), 24);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::GameHandler;
    use crate::gen4::tests::{load, real_rom, save};

    /// With `PPR_TEST_ROMS`: static encounters, fossils and roamers written to Platinum read back, with the special
    /// music following; an unchanged game saves byte for byte.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_platinum_statics_read_back() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes.clone());
        let mut data = h.game_data().clone();
        assert_eq!(data.statics.len(), 32);
        assert_eq!(data.fossils, (20..27).collect::<Vec<_>>());
        assert_eq!(data.main_game_legendaries, [487]);
        assert!(data.special_music_statics.contains(&487));
        // Giratina, a Game Corner-free static with special music; a fossil; Mesprit, the first roamer.
        data.statics[0].species = 25;
        data.statics[0].level = 33;
        data.statics[20].species = 1;
        data.statics[20].level = 40;
        data.statics[27].species = 4;
        data.statics[27].level = 45;
        data.correct_static_music = true;
        h.apply(&data).expect("applies");
        let again = load(save(&h));
        let read = &again.game_data().statics;
        assert_eq!(read[0].species, 25);
        assert_eq!(read[0].level, 33);
        assert_eq!((read[20].species, read[20].level), (1, 40));
        assert!(read[21..27].iter().all(|s| s.level == 40));
        assert_eq!((read[27].species, read[27].level), (4, 45));
        assert_eq!(read[28..], data.statics[28..]);
        assert_eq!(read[1..20], data.statics[1..20]);

        let mut unchanged = load(bytes.clone());
        let same = unchanged.game_data().clone();
        unchanged.apply(&same).expect("applies");
        assert!(save(&unchanged) == bytes, "an unchanged game saves byte for byte");
    }
}
