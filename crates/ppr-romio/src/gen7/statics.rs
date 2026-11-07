//! Starters, gifts, static encounters and in-game trades (`StaticPokemon`), the Zygarde assembled on Route 16
//! (a script and the executable), and the texts that name the starters and traded Pokémon.
//!
//! File 0 holds the gifts, 0x14 bytes each (species, forme, level, item at 8, egg flag at 10); the first three are
//! the starters. File 1 holds the static encounters, 0x38 bytes each (species, forme, level, item at 4 with 0xFFFF
//! for none, moves at 12). File 4 holds the trades, 0x34 bytes each.
//!
//! Starters, gifts and static encounters of an alternate forme with an entry of its own have that forme's number as
//! their species; the stand-ins 30 and 31, and formes the species doesn't have, are read as forme 0. Gifts and
//! static encounters hold their item.
//!
//! Deliberate choices:
//! - Only what changed is written: a slot keeps its stored forme while its species and forme stay, and its stored
//!   held item (0 or 0xFFFF for none) while that stays.
//! - A static encounter whose species or forme changed gets the default moves of its level, as in the original;
//!   one that keeps them keeps its moves, also when the randomizer happened to choose the same species.
//! - Linked encounters get their base's species and forme, and keep their own level and held item.

use ppr_formats::ctr::Decompression;
use ppr_model::{DexNumber, GameData, InGameTrade, ItemId, StaticEncounter, Type};

use super::amx::Amx;
use super::constants::{self as c, Family};
use super::files::Files;
use super::personal::FormeIndex;
use super::{find, put_u16, put_u32, word};
use crate::RomError;

/// A forme byte as the game data reads it: the stand-ins 30 and 31 and "keep" (-1) are forme 0.
fn internal_forme(b: u8) -> u8 {
    match b {
        30 | 31 | 0xFF => 0,
        f => f,
    }
}

fn item(id: u16) -> Option<ItemId> {
    (id != 0).then_some(id)
}

/// Where each static encounter of the game data is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    Gift(usize),
    Static(usize),
    Zygarde,
}

/// The static encounter layout as loaded.
#[derive(Debug, Clone)]
pub(super) struct Statics {
    /// Where each encounter is, before linked encounters are folded into the ones they follow.
    spots: Vec<Spot>,
    /// Linked encounters: (base, linked) positions in `spots`.
    links: Vec<(usize, usize)>,
    /// The forme byte of each spot as stored.
    formes: Vec<u8>,
    /// Held items as loaded, by spot.
    items: Vec<Option<ItemId>>,
    zygarde_levels: Vec<usize>,
    /// Where the alternate formes are.
    index: FormeIndex,
}

/// What a spot holds: species, forme, level and held item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Holder {
    species: DexNumber,
    forme: u8,
    level: u8,
    held: Option<ItemId>,
}

/// The moves of a static encounter, four of 16 bits; 0 is the default moves of its level.
const MOVES: usize = 12;

/// The linked encounters of a ROM entry: `[111:112, 119:130]`, base and linked positions.
pub(super) fn parse_links(value: &str) -> Vec<(usize, usize)> {
    let inner = value.split("//").next().unwrap_or_default();
    let inner = inner.trim().trim_start_matches('[');
    let inner = inner.split(']').next().unwrap_or_default();
    inner
        .split(',')
        .filter_map(|pair| {
            let (a, b) = pair.split_once(':')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

/// The starters, their held items and their formes.
type Starters = (Vec<DexNumber>, Vec<Option<ItemId>>, Vec<u8>);

/// The starters, their held items and their forme bytes as stored.
pub(super) fn read_starters(gifts: &[u8], index: &FormeIndex) -> Result<Starters, RomError> {
    let mut starters = Vec::new();
    let mut items = Vec::new();
    let mut formes = Vec::new();
    for i in 0..3 {
        let at = i * c::GIFT_SIZE;
        let stored = gifts.get(at + 2).copied().unwrap_or_default();
        starters.push(species_of(index, word(gifts, at)?, stored));
        formes.push(stored);
        items.push(item(word(gifts, at + 8)?));
    }
    Ok((starters, items, formes))
}

/// The species and forme of a Pokémon stored as `base` with forme byte `stored`.
fn holder(index: &FormeIndex, base: DexNumber, stored: u8) -> (DexNumber, u8) {
    let forme = index.holder_forme(base, u16::from(stored));
    (index.species_of(base, forme), forme)
}

/// The game-data species of a Pokémon stored as `base` with forme byte `stored`.
fn species_of(index: &FormeIndex, base: DexNumber, stored: u8) -> DexNumber {
    holder(index, base, stored).0
}

/// The forme byte to store with a Pokémon of `species` and `forme`: an alternate forme's own number, else `forme`.
fn forme_byte(index: &FormeIndex, species: DexNumber, forme: u8) -> u8 {
    match index.forme_number(species) {
        0 => forme,
        f => f,
    }
}

/// An encounter of the stored `base` and forme byte `stored` at `level`.
fn encounter(index: &FormeIndex, base: DexNumber, stored: u8, level: u8) -> StaticEncounter {
    let (species, forme) = holder(index, base, stored);
    let mut se = StaticEncounter::new(species, level);
    se.forme = forme;
    se
}

#[expect(
    clippy::too_many_arguments,
    reason = "the parts of the game the encounters are spread over"
)]
pub(super) fn read(
    gifts: &[u8],
    statics: &[u8],
    script: &[u8],
    code: &[u8],
    family: Family,
    skip: &[usize],
    links: Vec<(usize, usize)>,
    level_offsets: Vec<usize>,
    index: FormeIndex,
) -> Result<(Statics, Vec<StaticEncounter>), RomError> {
    let mut spots = Vec::new();
    let mut encounters = Vec::new();
    let mut formes = Vec::new();
    let mut items = Vec::new();
    for i in 3..gifts.len() / c::GIFT_SIZE {
        let at = i * c::GIFT_SIZE;
        let mut se = encounter(&index, word(gifts, at)?, gifts[at + 2], gifts[at + 3]);
        se.egg = gifts[at + 10] == 1;
        se.held_item = item(word(gifts, at + 8)?);
        items.push(se.held_item);
        encounters.push(se);
        formes.push(gifts[at + 2]);
        spots.push(Spot::Gift(i));
    }
    for i in 0..statics.len() / c::STATIC_SIZE {
        if skip.contains(&i) {
            continue;
        }
        let at = i * c::STATIC_SIZE;
        let mut se = encounter(&index, word(statics, at)?, statics[at + 2], statics[at + 3]);
        let held = word(statics, at + 4)?;
        se.held_item = item(if held == 0xFFFF { 0 } else { held });
        items.push(se.held_item);
        encounters.push(se);
        formes.push(statics[at + 2]);
        spots.push(Spot::Static(i));
    }
    let amx = Amx::parse(script)?;
    let levels: Vec<u8> = level_offsets
        .iter()
        .map(|&o| amx.data.get(o).copied().unwrap_or_default())
        .collect();
    if let (Some(species_at), Some(forme_at)) = (
        find(code, c::ZYGARDE_ASSEMBLY_SPECIES_PREFIX)?,
        find(code, c::ZYGARDE_ASSEMBLY_FORME_PREFIX)?,
    ) {
        let species = word(code, species_at)?;
        let untouched = code
            .get(forme_at..forme_at + 4)
            .is_some_and(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) == c::ZYGARDE_FORME_LOAD);
        let zygarde_forme = if untouched {
            // Zygarde 10% in Sun and Moon, met early; 50% in the post-game of Ultra Sun and Ultra Moon.
            u8::from(family == Family::SunMoon)
        } else {
            code[forme_at]
        };
        let mut se = encounter(
            &index,
            species,
            zygarde_forme,
            levels.first().copied().unwrap_or_default(),
        );
        se.linked_levels = levels.iter().skip(1).copied().collect();
        encounters.push(se);
        formes.push(zygarde_forme);
        items.push(None);
        spots.push(Spot::Zygarde);
    }
    // Linked encounters follow their base; they leave the list.
    let mut linked: Vec<usize> = Vec::new();
    for &(base, link) in &links {
        let level = encounters.get(link).map(|e| e.level).unwrap_or_default();
        if let Some(b) = encounters.get_mut(base) {
            b.linked_levels.push(level);
        }
        linked.push(link);
    }
    let kept = encounters
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !linked.contains(i))
        .map(|(_, e)| e)
        .collect();
    Ok((
        Statics {
            spots,
            links,
            formes,
            items,
            zygarde_levels: level_offsets,
            index,
        },
        kept,
    ))
}

/// The assembled Zygarde to write: its encounter, the species to store and the forme byte.
type Zygarde<'a> = (&'a StaticEncounter, DexNumber, u8);

impl Statics {
    /// Writes the encounters that changed: the linked ones get their base's species and forme, and their own level.
    /// A spot keeps its forme byte while its species and forme stay, and its held item while that stays; a static
    /// encounter with a new species or forme gets the default moves of its level.
    pub(super) fn write(
        &self,
        files: &mut Files,
        paths: [&str; 2],
        loaded: &[StaticEncounter],
        new: &[StaticEncounter],
    ) -> Result<(), RomError> {
        let [static_path, script_path] = paths;
        let garc = files.garc_mut(static_path, &Decompression::None)?;
        let mut gifts = garc.file(c::GIFTS_FILE)?;
        let mut statics = garc.file(c::STATICS_FILE)?;
        let zygarde = self.write_tables(&mut gifts, &mut statics, loaded, new);
        garc.set_file(c::GIFTS_FILE, gifts)?;
        garc.set_file(c::STATICS_FILE, statics)?;
        if let Some((se, species, forme)) = zygarde {
            let levels: Vec<u8> = std::iter::once(se.level)
                .chain(se.linked_levels.iter().copied())
                .collect();
            let garc = files.garc_mut(script_path, &Decompression::None)?;
            let mut amx = Amx::parse(&garc.file(c::ZYGARDE_ASSEMBLY_SCRIPT_FILE)?)?;
            for (&o, &level) in self.zygarde_levels.iter().zip(&levels) {
                if let Some(b) = amx.data.get_mut(o) {
                    *b = level;
                }
            }
            garc.set_file(c::ZYGARDE_ASSEMBLY_SCRIPT_FILE, amx.to_bytes())?;
            let code = &mut files.code;
            if let (Some(species_at), Some(forme_at)) = (
                find(code, c::ZYGARDE_ASSEMBLY_SPECIES_PREFIX)?,
                find(code, c::ZYGARDE_ASSEMBLY_FORME_PREFIX)?,
            ) {
                put_u16(code, species_at, species);
                // "mov r0, #forme" where the game loaded the forme.
                code[forme_at..forme_at + 4].copy_from_slice(&[forme, 0x00, 0xA0, 0xE3]);
            }
        }
        Ok(())
    }

    /// Writes the gifts and static encounters that changed into their files ([`Statics::write`]); the assembled
    /// Zygarde, when it changed, is left to the caller.
    fn write_tables<'a>(
        &self,
        gifts: &mut [u8],
        statics: &mut [u8],
        loaded: &[StaticEncounter],
        new: &'a [StaticEncounter],
    ) -> Option<Zygarde<'a>> {
        let old = self.unfold(loaded);
        let full = self.unfold(new);
        let mut zygarde = None;
        for (k, (&spot, h)) in self.spots.iter().zip(&full).enumerate() {
            let o = old.get(k);
            let unchanged =
                o == Some(h) && (spot != Spot::Zygarde || self.kept(k).is_some_and(|i| loaded.get(i) == new.get(i)));
            if unchanged {
                continue;
            }
            let same = o.is_some_and(|o| o.species == h.species && o.forme == h.forme);
            let forme = if same {
                self.formes[k]
            } else {
                forme_byte(&self.index, h.species, h.forme)
            };
            let species = self.index.base(h.species);
            let held_changed = o.map(|o| o.held) != Some(h.held);
            match spot {
                Spot::Gift(i) => {
                    let at = i * c::GIFT_SIZE;
                    put_u16(gifts, at, species);
                    gifts[at + 2] = forme;
                    gifts[at + 3] = h.level;
                    if held_changed {
                        put_u16(gifts, at + 8, h.held.unwrap_or(0));
                    }
                }
                Spot::Static(i) => {
                    write_static(statics, i, species, forme, h.level);
                    if held_changed {
                        put_u16(statics, i * c::STATIC_SIZE + 4, h.held.unwrap_or(0xFFFF));
                    }
                    if !same {
                        reset_moves(statics, i);
                    }
                }
                Spot::Zygarde => zygarde = self.kept(k).and_then(|i| new.get(i)).map(|se| (se, species, forme)),
            }
        }
        zygarde
    }

    /// The position in the game data's list of spot `k`; `None` for a linked encounter, which left the list.
    fn kept(&self, k: usize) -> Option<usize> {
        if self.links.iter().any(|&(_, l)| l == k) {
            return None;
        }
        Some(k - self.links.iter().filter(|&&(_, l)| l < k).count())
    }

    /// What every spot holds: a linked encounter has its base's species and forme, and its own level (kept with
    /// the base) and held item (as loaded).
    fn unfold(&self, list: &[StaticEncounter]) -> Vec<Holder> {
        let none = Holder {
            species: 0,
            forme: 0,
            level: 0,
            held: None,
        };
        let mut full: Vec<Holder> = (0..self.spots.len())
            .map(|k| {
                self.kept(k).and_then(|i| list.get(i)).map_or(none, |e| Holder {
                    species: e.species,
                    forme: e.forme,
                    level: e.level,
                    held: e.held_item,
                })
            })
            .collect();
        for (r, &(base, link)) in self.links.iter().enumerate() {
            let Some(b) = self.kept(base).and_then(|i| list.get(i)) else {
                continue;
            };
            let level = b.linked_levels.get(self.link_rank(r)).copied().unwrap_or(b.level);
            if let Some(slot) = full.get_mut(link) {
                *slot = Holder {
                    species: b.species,
                    forme: b.forme,
                    level,
                    held: self.items.get(link).copied().flatten(),
                };
            }
        }
        full
    }

    /// Which of its base's linked levels link `k` is: the links of the same base before it.
    fn link_rank(&self, k: usize) -> usize {
        let base = self.links[k].0;
        self.links[..k].iter().filter(|&&(b, _)| b == base).count()
    }
}

/// Writes the species, forme and level of static encounter `i`.
fn write_static(statics: &mut [u8], i: usize, species: DexNumber, forme: u8, level: u8) {
    let at = i * c::STATIC_SIZE;
    put_u16(statics, at, species);
    statics[at + 2] = forme;
    statics[at + 3] = level;
}

/// Gives static encounter `i` the default moves of its level.
fn reset_moves(statics: &mut [u8], i: usize) {
    let at = i * c::STATIC_SIZE + MOVES;
    for m in 0..4 {
        put_u16(statics, at + 2 * m, 0);
    }
}

/// The first Route 1 encounter (a level 3 Pikipek) is also a static encounter; it follows that slot.
pub(super) fn write_route_1(
    files: &mut Files,
    path: &str,
    data: &GameData,
    personal: &super::personal::Personal,
) -> Result<(), RomError> {
    let Some(e) = data
        .encounters
        .get(c::ROUTE_1_SET)
        .and_then(|s| s.encounters.get(c::ROUTE_1_PIKIPEK_SLOT))
    else {
        return Ok(());
    };
    let garc = files.garc_mut(path, &Decompression::None)?;
    let mut statics = garc.file(c::STATICS_FILE)?;
    let at = c::ROUTE_1_PIKIPEK_STATIC * c::STATIC_SIZE;
    let old_species = word(&statics, at)?;
    let species = personal.base(e.species);
    let forme = if old_species == species && personal.holder_forme(species, u16::from(statics[at + 2])) == e.forme {
        statics[at + 2]
    } else {
        match personal.forme_number(e.species) {
            0 => e.forme,
            f => f,
        }
    };
    write_static(&mut statics, c::ROUTE_1_PIKIPEK_STATIC, species, forme, e.max_level);
    garc.set_file(c::STATICS_FILE, statics)?;
    Ok(())
}

/// Writes the starters and their held items. A starter keeps its forme byte while its species stays; a new one is
/// stored as its base species with its forme's number, as in the original, which gives a new base species forme 0.
pub(super) fn write_starters(
    gifts: &mut [u8],
    loaded: &[DexNumber],
    formes: &[u8],
    starters: &[DexNumber],
    items: &[Option<ItemId>],
    index: &FormeIndex,
) {
    for (i, &forme) in formes.iter().enumerate().take(3) {
        let at = i * c::GIFT_SIZE;
        if let Some(&s) = starters.get(i) {
            put_u16(gifts, at, index.base(s));
            gifts[at + 2] = if loaded.get(i) == Some(&s) {
                forme
            } else {
                index.forme_number(s)
            };
        }
        if let Some(&held) = items.get(i) {
            put_u16(gifts, at + 8, held.unwrap_or(0));
        }
    }
}

fn type_name(t: Type) -> String {
    format!("{t:?}")
}

/// Drops the first `n` characters.
fn after(s: &str, n: usize) -> String {
    s.chars().skip(n).collect()
}

/// The starter texts with the new starters' names and types.
pub(super) fn starter_text(family: Family, lines: &mut [String], data: &GameData) {
    let sp = |i: usize| data.starters.get(i).and_then(|&n| data.species(n));
    let (Some(a), Some(b), Some(c3)) = (sp(0), sp(1), sp(2)) else {
        return;
    };
    let new = [a, b, c3];
    let set = |lines: &mut [String], i: usize, s: String| {
        if let Some(l) = lines.get_mut(i) {
            *l = s;
        }
    };
    let get = |lines: &[String], i: usize| lines.get(i).cloned().unwrap_or_default();
    match family {
        Family::UltraSunMoon => {
            // "Rowlet", "Litten" and "Popplio" lead these lines.
            let first = [
                after(&get(lines, 1), 6),
                after(&get(lines, 2), 6),
                after(&get(lines, 3), 7),
            ];
            for (i, rest) in first.into_iter().enumerate() {
                set(lines, i + 1, format!("{}{rest}", new[i].name));
            }
            for (i, s) in new.iter().enumerate() {
                set(
                    lines,
                    i + 7,
                    format!(
                        "So, you wanna go with the {}-type Pokémon\\n{}?[VAR 0114(0005)]",
                        type_name(s.primary_type),
                        s.name
                    ),
                );
                set(lines, i + 14, s.name.clone());
            }
        }
        Family::SunMoon => {
            let first = [
                after(&get(lines, 11), 6),
                after(&get(lines, 12), 6),
                after(&get(lines, 13), 7),
            ];
            for (i, rest) in first.into_iter().enumerate() {
                set(lines, i + 11, format!("{}{rest}", new[i].name));
            }
            for (i, s) in new.iter().enumerate() {
                let t = type_name(s.primary_type);
                set(lines, i + 1, format!("The {t}-type {}", s.name));
                set(
                    lines,
                    i + 4,
                    format!("Will you choose the {t}-type Pokémon\\n{}?[VAR 0114(0008)]", s.name),
                );
                let flavor = get(lines, i + 35);
                let tail = flavor.find("\\n").map_or("", |p| &flavor[p..]);
                set(lines, i + 35, format!("The {t}-type {}{tail}", s.name));
            }
        }
    }
}

/// The trades as loaded, with what the game data leaves out.
#[derive(Debug, Clone)]
pub(super) struct Trades {
    formes: Vec<u8>,
}

/// The in-game trades; their nicknames and trainer names come from the trade texts.
pub(super) fn read_trades(file: &[u8], texts: &[String]) -> Result<(Trades, Vec<InGameTrade>), RomError> {
    let mut trades = Vec::new();
    let mut formes = Vec::new();
    for i in 0..file.len() / c::TRADE_SIZE {
        let at = i * c::TRADE_SIZE;
        let ot_id = u32::from_le_bytes(
            file[at + 0x10..at + 0x14]
                .try_into()
                .map_err(|_| RomError::Malformed("trade cut off".into()))?,
        );
        let text = |o: usize| -> Result<String, RomError> {
            Ok(texts.get(usize::from(word(file, at + o)?)).cloned().unwrap_or_default())
        };
        trades.push(InGameTrade {
            requested: Some(word(file, at + 0x2C)?),
            given: word(file, at)?,
            nickname: text(2)?,
            ot_name: text(0x18)?,
            ot_id,
            ivs: file[at + 6..at + 12].to_vec(),
            held_item: item(word(file, at + 0x14)?),
        });
        formes.push(internal_forme(file[at + 4]));
    }
    Ok((Trades { formes }, trades))
}

impl Trades {
    /// Writes the trades that changed, their texts included; the texts that name the traded species get the new
    /// names. A trade keeps its forme while its given species stays.
    pub(super) fn write(
        &self,
        family: Family,
        file: &mut [u8],
        texts: &mut [String],
        loaded: &GameData,
        data: &GameData,
    ) -> Result<(), RomError> {
        let name = |d: &GameData, n: Option<DexNumber>| {
            n.and_then(|n| d.species(n)).map(|s| s.name.clone()).unwrap_or_default()
        };
        for (i, (old, t)) in loaded.trades.iter().zip(&data.trades).enumerate() {
            if old == t {
                continue;
            }
            let at = i * c::TRADE_SIZE;
            put_u16(file, at, t.given);
            file[at + 4] = if old.given == t.given { self.formes[i] } else { 0 };
            put_u16(file, at + 0x2C, t.requested.unwrap_or(0));
            let nick = usize::from(word(file, at + 2)?);
            let ot = usize::from(word(file, at + 0x18)?);
            if let Some(l) = texts.get_mut(nick) {
                l.clone_from(&t.nickname);
            }
            if let Some(l) = texts.get_mut(ot) {
                l.clone_from(&t.ot_name);
            }
            put_u32(file, at + 0x10, t.ot_id);
            for (k, &iv) in t.ivs.iter().take(6).enumerate() {
                file[at + 6 + k] = iv;
            }
            put_u16(file, at + 0x14, t.held_item.unwrap_or(0));
            for &line in c::hardcoded_trade_texts(family, i) {
                if let Some(l) = texts.get_mut(line) {
                    *l = l
                        .replace(&name(loaded, old.requested), &name(data, t.requested))
                        .replace(&name(loaded, Some(old.given)), &name(data, Some(t.given)));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Species 20 has an alternate forme 2 with entry 900; species 19 has two cosmetic formes.
    fn index() -> FormeIndex {
        FormeIndex::of(&[(900, 20, 2)], &[(19, 2)])
    }

    /// Four gifts: the starters 20 (forme 2), 19 (forme 1) and 21 (forme 30, a stand-in), and a gift 21 holding
    /// item 7.
    fn gifts() -> Vec<u8> {
        let mut g = vec![0u8; 4 * c::GIFT_SIZE];
        for (i, (species, forme, item)) in [(20, 2, 0), (19, 1, 0), (21, 30, 0), (21, 0, 7)]
            .into_iter()
            .enumerate()
        {
            let at = i * c::GIFT_SIZE;
            put_u16(&mut g, at, species);
            g[at + 2] = forme;
            g[at + 3] = 5;
            put_u16(&mut g, at + 8, item);
        }
        g
    }

    /// Three static encounters: 20 in forme 2 holding nothing (0xFFFF), 19 in forme 1 holding nothing (0) and 21
    /// in forme 30 holding item 155, linked to the first. Every one has move 33 first.
    fn statics() -> Vec<u8> {
        let mut f = vec![0u8; 3 * c::STATIC_SIZE];
        for (i, (species, forme, level, held)) in [(20, 2, 40, 0xFFFF), (19, 1, 30, 0), (21, 30, 50, 155)]
            .into_iter()
            .enumerate()
        {
            let at = i * c::STATIC_SIZE;
            put_u16(&mut f, at, species);
            f[at + 2] = forme;
            f[at + 3] = level;
            put_u16(&mut f, at + 4, held);
            put_u16(&mut f, at + MOVES, 33);
        }
        f
    }

    /// An AMX script without cells.
    fn script() -> Vec<u8> {
        let start = 0x1C_u32;
        let mut enc = Vec::new();
        for v in [start, 0x0A0A_F1E0, 0, start, 0, start, 0] {
            enc.extend_from_slice(&u32::to_le_bytes(v));
        }
        enc
    }

    fn load() -> (Statics, Vec<StaticEncounter>) {
        // Spot 0 is the gift, spots 1 to 3 the static encounters; the last follows the first.
        read(
            &gifts(),
            &statics(),
            &script(),
            &[],
            Family::UltraSunMoon,
            &[],
            vec![(1, 3)],
            Vec::new(),
            index(),
        )
        .expect("reads")
    }

    fn encounter_of(species: DexNumber, forme: u8, level: u8, held_item: Option<ItemId>) -> StaticEncounter {
        let mut se = StaticEncounter::new(species, level);
        se.forme = forme;
        se.held_item = held_item;
        se
    }

    #[test]
    fn links_are_read_without_the_comment() {
        assert_eq!(
            parse_links("[126:127, 134:145, 138:144] // more could follow"),
            [(126, 127), (134, 145), (138, 144)]
        );
        assert_eq!(parse_links(""), []);
    }

    #[test]
    fn starters_are_their_formes_species() {
        let (starters, items, formes) = read_starters(&gifts(), &index()).expect("reads");
        assert_eq!(starters, [900, 19, 21]);
        assert_eq!(items, [None; 3]);
        assert_eq!(formes, [2, 1, 30]);

        let mut g = gifts();
        write_starters(&mut g, &starters, &formes, &starters, &items, &index());
        assert_eq!(g, gifts(), "unchanged");
        // A new alternate forme is its base species with its forme number; a new base species gets forme 0.
        write_starters(&mut g, &starters, &formes, &[19, 900, 21], &items, &index());
        assert_eq!((word(&g, 0).expect("species"), g[2]), (19, 0));
        let second = c::GIFT_SIZE;
        assert_eq!((word(&g, second).expect("species"), g[second + 2]), (20, 2));
        assert_eq!(g[2 * c::GIFT_SIZE + 2], 30, "an unchanged starter keeps its forme byte");
        assert_eq!(read_starters(&g, &index()).expect("reads").0, [19, 900, 21]);
    }

    #[test]
    fn statics_are_their_formes_species_with_their_held_items() {
        let (_, encounters) = load();
        let mut first = encounter_of(900, 2, 40, None);
        first.linked_levels = vec![50];
        assert_eq!(
            encounters,
            [encounter_of(21, 0, 5, Some(7)), first, encounter_of(19, 1, 30, None)]
        );
    }

    #[test]
    fn only_what_changed_is_written() {
        let (layout, loaded) = load();
        let (mut g, mut f) = (gifts(), statics());
        assert!(layout.write_tables(&mut g, &mut f, &loaded, &loaded).is_none());
        assert_eq!((g.clone(), f.clone()), (gifts(), statics()), "unchanged");

        let mut new = loaded.clone();
        // The gift becomes the alternate forme, the first static the base species in a cosmetic forme (and its
        // linked encounter with it); the second static only gets an item.
        new[0].species = 900;
        new[0].forme = 2;
        new[1].species = 19;
        new[1].forme = 1;
        new[2].held_item = Some(5);
        layout.write_tables(&mut g, &mut f, &loaded, &new);
        assert_eq!(
            (word(&g, 3 * c::GIFT_SIZE).expect("species"), g[3 * c::GIFT_SIZE + 2]),
            (20, 2)
        );
        assert_eq!(
            word(&g, 3 * c::GIFT_SIZE + 8).expect("item"),
            7,
            "the gift keeps its item"
        );
        for i in [0, 2] {
            let at = i * c::STATIC_SIZE;
            assert_eq!((word(&f, at).expect("species"), f[at + 2]), (19, 1), "static {i}");
            assert_eq!(
                word(&f, at + MOVES).expect("moves"),
                0,
                "static {i} gets the default moves"
            );
        }
        assert_eq!(word(&f, 4).expect("item"), 0xFFFF, "none stays as stored");
        assert_eq!(
            word(&f, 2 * c::STATIC_SIZE + 4).expect("item"),
            155,
            "the linked encounter keeps its item"
        );
        let second = c::STATIC_SIZE;
        assert_eq!(word(&f, second + 4).expect("item"), 5);
        assert_eq!((f[second + 2], word(&f, second + MOVES).expect("moves")), (1, 33));
        let (_, back) = read(
            &g,
            &f,
            &script(),
            &[],
            Family::UltraSunMoon,
            &[],
            vec![(1, 3)],
            Vec::new(),
            index(),
        )
        .expect("reads");
        assert_eq!(back, new);

        // A held item taken away: 0 for a gift, 0xFFFF for a static encounter.
        let mut none = new.clone();
        none[0].held_item = None;
        none[2].held_item = None;
        layout.write_tables(&mut g, &mut f, &new, &none);
        assert_eq!(word(&g, 3 * c::GIFT_SIZE + 8).expect("item"), 0);
        assert_eq!(word(&f, second + 4).expect("item"), 0xFFFF);
    }
}
