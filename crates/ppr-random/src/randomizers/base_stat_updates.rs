//! Base stat updates: the base stats later generations changed.
//!
//! In Gen 1, whose one Special stat is kept as Sp. Atk, the Sp. Atk changes set the Special and the Sp. Def changes
//! are left out.
//!
//! Alternate formes are changed too, found by their base species: Mega Alakazam's Sp. Def in Gen 6 games (Gen 7 games
//! have it already) and Aegislash's Blade Forme in Gen 6 and 7 games.

use ppr_model::{BaseStats, DexNumber, GameData};

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Base Stat Updates";

/// One of the six stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stat {
    Hp,
    Attack,
    Defense,
    SpAttack,
    SpDefense,
    Speed,
}

use Stat::{Attack as ATK, Defense as DEF, Hp as HP, SpAttack as SPA, SpDefense as SPD, Speed as SPE};

/// Which species an update changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// A base species.
    Species(DexNumber),
    /// The Mega Evolution of a base species that has one.
    Mega(DexNumber),
    /// An alternate forme of a base species, by forme number.
    Forme(DexNumber, u8),
}

/// A stat that generation `to` changed, in games from generation `since` on.
struct Update {
    to: u8,
    since: u8,
    target: Target,
    stat: Stat,
    value: u8,
}

const fn u(to: u8, since: u8, species: DexNumber, stat: Stat, value: u8) -> Update {
    at(to, since, Target::Species(species), stat, value)
}

const fn at(to: u8, since: u8, target: Target, stat: Stat, value: u8) -> Update {
    Update {
        to,
        since,
        target,
        stat,
        value,
    }
}

/// The changes of Gen 6 to Gen 9, by generation. Gens 2–5 changed no existing species' stats.
const UPDATES: &[Update] = &[
    // Gen 6
    u(6, 1, 12, SPA, 90),  // Butterfree
    u(6, 1, 15, ATK, 90),  // Beedrill
    u(6, 1, 18, SPE, 101), // Pidgeot
    u(6, 1, 25, DEF, 40),  // Pikachu
    u(6, 1, 25, SPD, 50),
    u(6, 1, 26, SPE, 110),  // Raichu
    u(6, 1, 31, ATK, 92),   // Nidoqueen
    u(6, 1, 34, ATK, 102),  // Nidoking
    u(6, 1, 36, SPA, 95),   // Clefable
    u(6, 1, 40, SPA, 85),   // Wigglytuff
    u(6, 1, 45, SPA, 110),  // Vileplume
    u(6, 1, 62, ATK, 95),   // Poliwrath
    u(6, 1, 65, SPD, 95),   // Alakazam
    u(6, 1, 71, SPD, 70),   // Victreebel
    u(6, 1, 76, ATK, 120),  // Golem
    u(6, 2, 181, DEF, 85),  // Ampharos
    u(6, 2, 182, DEF, 95),  // Bellossom
    u(6, 2, 184, SPA, 60),  // Azumarill
    u(6, 2, 189, SPD, 95),  // Jumpluff
    u(6, 3, 267, SPA, 100), // Beautifly
    u(6, 3, 295, SPD, 73),  // Exploud
    u(6, 4, 398, SPD, 60),  // Staraptor
    u(6, 4, 407, DEF, 65),  // Roserade
    u(6, 5, 508, ATK, 110), // Stoutland
    u(6, 5, 521, ATK, 115), // Unfezant
    u(6, 5, 526, SPD, 80),  // Gigalith
    u(6, 5, 537, ATK, 95),  // Seismitoad
    u(6, 5, 542, SPD, 80),  // Leavanny
    u(6, 5, 545, ATK, 100), // Scolipede
    u(6, 5, 553, DEF, 80),  // Krookodile
    // Gen 7
    u(7, 1, 24, ATK, 95),   // Arbok
    u(7, 1, 51, ATK, 100),  // Dugtrio
    u(7, 1, 83, ATK, 90),   // Farfetch'd
    u(7, 1, 85, SPE, 110),  // Dodrio
    u(7, 1, 101, SPE, 150), // Electrode
    u(7, 1, 103, SPD, 75),  // Exeggutor
    u(7, 2, 164, SPA, 86),  // Noctowl
    u(7, 2, 168, SPD, 70),  // Ariados
    u(7, 2, 211, DEF, 85),  // Qwilfish
    u(7, 2, 219, HP, 60),   // Magcargo
    u(7, 2, 219, SPA, 90),
    u(7, 2, 222, HP, 65), // Corsola
    u(7, 2, 222, DEF, 95),
    u(7, 2, 222, SPD, 95),
    u(7, 2, 226, HP, 85),   // Mantine
    u(7, 3, 277, SPA, 75),  // Swellow
    u(7, 3, 279, SPA, 95),  // Pelipper
    u(7, 3, 284, SPA, 100), // Masquerain
    u(7, 3, 284, SPE, 80),
    u(7, 3, 301, SPE, 90), // Delcatty
    u(7, 3, 313, DEF, 75), // Volbeat
    u(7, 3, 313, SPD, 85),
    u(7, 3, 314, DEF, 75), // Illumise
    u(7, 3, 314, SPD, 85),
    u(7, 3, 337, HP, 90), // Lunatone
    u(7, 3, 338, HP, 90), // Solrock
    u(7, 3, 358, HP, 75), // Chimecho
    u(7, 3, 358, DEF, 80),
    u(7, 3, 358, SPD, 90),
    u(7, 5, 527, HP, 65),   // Woobat
    u(7, 5, 558, ATK, 105), // Crustle
    u(7, 5, 614, ATK, 130), // Beartic
    u(7, 5, 615, HP, 80),   // Cryogonal
    u(7, 5, 615, DEF, 50),
    at(7, 6, Target::Mega(65), SPD, 105), // Mega Alakazam
    // Gen 8
    u(8, 6, 681, DEF, 140), // Aegislash
    u(8, 6, 681, SPD, 140),
    at(8, 6, Target::Forme(681, 1), ATK, 140), // Aegislash's Blade Forme
    at(8, 6, Target::Forme(681, 1), SPA, 140),
    // Gen 9
    u(9, 4, 488, DEF, 110), // Cresselia
    u(9, 4, 488, SPD, 120),
];

fn stat_mut(stats: &mut BaseStats, stat: Stat) -> &mut u8 {
    match stat {
        HP => &mut stats.hp,
        ATK => &mut stats.attack,
        DEF => &mut stats.defense,
        SPA => &mut stats.sp_attack,
        SPD => &mut stats.sp_defense,
        SPE => &mut stats.speed,
    }
}

/// Updates base stats to a later generation according to [`ppr_settings::BaseStatSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BaseStatUpdater;

impl Step<RunData> for BaseStatUpdater {
    fn id(&self) -> u64 {
        step_ids::BASE_STAT_UPDATES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.base_stats.update_to_generation.is_some()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        if let Some(to) = ctx.data.settings.base_stats.update_to_generation {
            update(&mut ctx.data.game, to);
        }
        Ok(())
    }
}

/// Every change of the generations after the game's, up to `to`.
fn update(game: &mut GameData, to: u8) {
    let generation = game.identity.generation.number();
    for up in UPDATES
        .iter()
        .filter(|up| up.to > generation && up.to <= to && generation >= up.since)
    {
        // Gen 1's Special takes the Sp. Atk changes (Butterfree, Clefable, Wigglytuff, Vileplume).
        if generation == 1 && up.stat == SPD {
            continue;
        }
        if let Some(n) = target_species(game, up.target)
            && let Ok(i) = game.species.binary_search_by_key(&n, |s| s.number)
        {
            *stat_mut(&mut game.species[i].stats, up.stat) = up.value;
        }
    }
}

/// The species `target` is in `game`, if the game has it.
fn target_species(game: &GameData, target: Target) -> Option<DexNumber> {
    match target {
        Target::Species(n) => Some(n),
        Target::Mega(base) => game.formes_of(base).find(|sp| sp.is_mega()).map(|sp| sp.number),
        Target::Forme(base, number) => game
            .formes_of(base)
            .find(|sp| sp.forme.number == number)
            .map(|sp| sp.number),
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::Type;

    use super::*;
    use crate::randomizers::testing::{emerald, species};

    fn game() -> GameData {
        emerald(
            [12, 25, 267, 337, 386]
                .map(|n| species(n, Type::Normal, None, 50))
                .to_vec(),
        )
    }

    #[test]
    fn updates_up_to_the_chosen_generation() {
        let mut g = game();
        update(&mut g, 6);
        let stats = |g: &GameData, n| g.species(n).expect("there").stats;
        assert_eq!(stats(&g, 12).sp_attack, 90);
        assert_eq!((stats(&g, 25).defense, stats(&g, 25).sp_defense), (40, 50));
        assert_eq!(stats(&g, 267).sp_attack, 100);
        assert_eq!(stats(&g, 337).hp, 50, "Lunatone changed in Gen 7");
        update(&mut g, 9);
        assert_eq!(stats(&g, 337).hp, 90);
        assert_eq!(stats(&g, 386), game().species(386).expect("there").stats);
    }

    #[test]
    fn earlier_generations_change_nothing() {
        let mut g = game();
        update(&mut g, 3);
        assert_eq!(g, game());
    }

    #[test]
    fn gen1_updates_the_special_and_not_sp_def() {
        let mut g =
            crate::randomizers::testing::gen1([12, 25, 65].map(|n| species(n, Type::Normal, None, 50)).to_vec(), false);
        update(&mut g, 9);
        let stats = |n| g.species(n).expect("there").stats;
        assert_eq!(stats(12).sp_attack, 90, "Butterfree's Special");
        assert_eq!((stats(25).defense, stats(25).sp_defense), (40, 0), "Pikachu");
        assert_eq!(stats(65), game_stats_of(65), "Alakazam changed only its Sp. Def");
    }

    fn game_stats_of(n: DexNumber) -> BaseStats {
        let mut stats = species(n, Type::Normal, None, 50).stats;
        stats.sp_defense = 0;
        stats
    }

    /// A Gen 6 or 7 game with Alakazam (65), Aegislash (681) and their formes.
    fn forme_game(generation: u8) -> GameData {
        let mut g = game();
        g.identity.generation = ppr_model::Generation::new(generation).expect("valid");
        g.species = [65, 681].map(|n| species(n, Type::Normal, None, 50)).to_vec();
        for (n, base, mega) in [(800, 65, true), (801, 681, false)] {
            let mut sp = species(n, Type::Normal, None, 150);
            sp.forme = ppr_model::Forme {
                base: Some(base),
                number: 1,
                mega,
                ..ppr_model::Forme::default()
            };
            g.species.push(sp);
        }
        g
    }

    #[test]
    fn alternate_formes_are_updated_by_their_base_species() {
        let stats = |g: &GameData, n| g.species(n).expect("there").stats;
        let mut g = forme_game(6);
        update(&mut g, 7);
        assert_eq!(stats(&g, 800).sp_defense, 105, "Mega Alakazam");
        assert_eq!(stats(&g, 801), forme_game(6).species(801).expect("there").stats);
        update(&mut g, 8);
        let blade = stats(&g, 801);
        assert_eq!(
            (blade.attack, blade.sp_attack, blade.defense),
            (140, 140, 150),
            "Blade Forme"
        );
        assert_eq!((stats(&g, 681).defense, stats(&g, 681).sp_defense), (140, 140));

        let mut g = forme_game(7);
        update(&mut g, 9);
        assert_eq!(
            stats(&g, 800).sp_defense,
            150,
            "a Gen 7 game has Mega Alakazam's already"
        );
        assert_eq!((stats(&g, 801).attack, stats(&g, 801).sp_attack), (140, 140));
    }

    #[test]
    fn missing_formes_change_nothing() {
        let mut g = forme_game(6);
        g.species.retain(ppr_model::Species::is_base_forme);
        let old = g.clone();
        update(&mut g, 7);
        assert_eq!(g, old, "not Alakazam itself");
        update(&mut g, 8);
        assert_eq!(g.species(681).expect("there").stats.attack, 50, "not Aegislash itself");
    }

    #[test]
    fn the_table_is_ordered_by_generation() {
        assert!(UPDATES.windows(2).all(|w| w[0].to <= w[1].to));
        assert!(UPDATES.iter().all(|u| (6..=9).contains(&u.to) && u.since < u.to));
    }
}
