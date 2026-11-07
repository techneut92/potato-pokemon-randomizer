//! The check value: a value over the randomized species stats and abilities, trainer and wild Pokémon, TM and tutor
//! moves, static and Totem Pokémon, for Race Mode players to compare their ROMs by.

use ppr_model::GameData;

/// The check value of the game as randomized, its Totem Pokémon included, over the wild encounter sets the run
/// used: by time of day when the settings ask for it.
///
/// Deliberate choice: the original also reads the encounters by time of day when the wild levels change without
/// randomizing the Pokémon and time of day is off. The app's options keep time of day on whenever wild Pokémon are
/// not randomized, so that only differs for settings strings or command lines that turn it off.
pub fn check_value(game: &GameData) -> u32 {
    let mut cv = CheckValue(0);
    let mut species: Vec<_> = game.species.iter().collect();
    species.sort_by_key(|sp| sp.number);
    for sp in species {
        let s = &sp.stats;
        cv.add([s.hp, s.attack, s.defense, s.speed, s.sp_attack, s.sp_defense].map(u32::from));
        let ability = |i: usize| u32::from(sp.abilities.get(i).copied().unwrap_or(0));
        cv.add([ability(0), ability(1), ability(2)]);
    }
    for p in game.trainers.iter().flat_map(|t| &t.pokemon) {
        cv.add([u32::from(p.level), u32::from(p.species)]);
    }
    for e in game.encounters.iter().flat_map(|s| &s.encounters) {
        cv.add([u32::from(e.min_level), u32::from(e.species)]);
    }
    cv.add(game.tm_moves.iter().map(|&m| u32::from(m)));
    cv.add(game.tutor_moves.iter().map(|&m| u32::from(m)));
    cv.add(game.statics.iter().map(|s| u32::from(s.species)));
    cv.add(game.totems.iter().map(|t| u32::from(t.species)));
    cv.0
}

/// The running check value: each value rotates the check value left by 3 bits, then is XOR-ed in.
struct CheckValue(u32);

impl CheckValue {
    fn add(&mut self, values: impl IntoIterator<Item = u32>) {
        for v in values {
            self.0 = self.0.rotate_left(3) ^ v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_rotate_then_xor() {
        let mut cv = CheckValue(0);
        cv.add([1, 2]);
        assert_eq!(cv.0, (1 << 3) ^ 2);
        let mut cv = CheckValue(0x8000_0000);
        cv.add([0]);
        assert_eq!(cv.0, 0b100);
    }
}
