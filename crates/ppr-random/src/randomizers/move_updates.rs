//! Move updates: the power, PP, accuracy, type and category later generations gave moves.
//!
//! Only these numbers change, not how the moves work: Low Kick gets Gen 3's accuracy, not its
//! weight-based power.

use ppr_model::{GameData, MoveCategory, MoveId, SWIFT, Type};

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Move Updates";

/// What an update changes.
#[derive(Debug, Clone, Copy)]
enum Change {
    Power(u16),
    Pp(u8),
    Accuracy(u8),
    /// The game's "never misses" accuracy.
    PerfectAccuracy,
    Type(Type),
    Category(MoveCategory),
}

/// In which games an update applies, besides being for a later generation.
#[derive(Debug, Clone, Copy)]
enum When {
    Always,
    /// Games from this generation on.
    Since(u8),
    /// Games of this generation.
    Exactly(u8),
}

use When::{Always, Exactly, Since};

/// A change that generation `to` made to a move.
struct Update {
    to: u8,
    when: When,
    number: MoveId,
    change: Change,
}

const fn u(to: u8, when: When, number: MoveId, change: Change) -> Update {
    Update {
        to,
        when,
        number,
        change,
    }
}

/// The changes of Gen 2 to Gen 9, by generation, each with its move's name.
const UPDATES: &[Update] = &[
    u(2, Always, 2, Change::Type(Type::Fighting)),  // Karate Chop
    u(2, Always, 16, Change::Type(Type::Flying)),   // Gust
    u(2, Always, 17, Change::Power(60)),            // Wing Attack
    u(2, Always, 18, Change::Accuracy(100)),        // Whirlwind
    u(2, Always, 28, Change::Type(Type::Ground)),   // Sand Attack
    u(2, Always, 38, Change::Power(120)),           // Double-Edge
    u(2, Always, 59, Change::Accuracy(70)),         // Blizzard
    u(2, Always, 88, Change::Accuracy(90)),         // Rock Throw
    u(2, Always, 95, Change::Accuracy(60)),         // Hypnosis
    u(2, Always, 120, Change::Power(200)),          // Self-Destruct
    u(2, Always, 153, Change::Power(250)),          // Explosion
    u(2, Always, 91, Change::Power(60)),            // Dig
    u(3, Always, 13, Change::Accuracy(100)),        // Razor Wind
    u(3, Always, 67, Change::Accuracy(100)),        // Low Kick
    u(4, Always, 19, Change::Power(90)),            // Fly
    u(4, Always, 22, Change::Pp(15)),               // Vine Whip
    u(4, Always, 71, Change::Pp(25)),               // Absorb
    u(4, Always, 72, Change::Pp(15)),               // Mega Drain
    u(4, Always, 91, Change::Power(80)),            // Dig
    u(4, Always, 105, Change::Pp(10)),              // Recover
    u(4, Always, 148, Change::Accuracy(100)),       // Flash
    u(4, Always, 80, Change::Power(90)),            // Petal Dance
    u(4, Always, 50, Change::Accuracy(80)),         // Disable
    u(4, Always, 26, Change::Power(85)),            // Jump Kick
    u(4, Always, 136, Change::Power(100)),          // High Jump Kick
    u(4, Since(2), 192, Change::Power(120)),        // Zap Cannon
    u(4, Since(2), 200, Change::Power(120)),        // Outrage
    u(4, Since(2), 200, Change::Pp(10)),            // Outrage
    u(4, Since(2), 202, Change::Pp(10)),            // Giga Drain
    u(4, Since(2), 249, Change::Power(40)),         // Rock Smash
    u(4, Exactly(3), 254, Change::Pp(20)),          // Stockpile
    u(4, Exactly(3), 291, Change::Power(80)),       // Dive
    u(4, Exactly(3), 348, Change::Power(90)),       // Leaf Blade
    u(5, Always, 20, Change::Accuracy(85)),         // Bind
    u(5, Always, 26, Change::Pp(10)),               // Jump Kick
    u(5, Always, 26, Change::Power(100)),           // Jump Kick
    u(5, Always, 33, Change::Power(50)),            // Tackle
    u(5, Always, 33, Change::Accuracy(100)),        // Tackle
    u(5, Always, 35, Change::Accuracy(90)),         // Wrap
    u(5, Always, 37, Change::Pp(10)),               // Thrash
    u(5, Always, 37, Change::Power(120)),           // Thrash
    u(5, Always, 50, Change::Accuracy(100)),        // Disable
    u(5, Always, 80, Change::Pp(10)),               // Petal Dance
    u(5, Always, 80, Change::Power(120)),           // Petal Dance
    u(5, Always, 83, Change::Accuracy(85)),         // Fire Spin
    u(5, Always, 83, Change::Power(35)),            // Fire Spin
    u(5, Always, 92, Change::Accuracy(90)),         // Toxic
    u(5, Always, 128, Change::Accuracy(85)),        // Clamp
    u(5, Always, 128, Change::Pp(15)),              // Clamp
    u(5, Always, 136, Change::Pp(10)),              // High Jump Kick
    u(5, Always, 136, Change::Power(130)),          // High Jump Kick
    u(5, Always, 137, Change::Accuracy(90)),        // Glare
    u(5, Always, 139, Change::Accuracy(80)),        // Poison Gas
    u(5, Always, 152, Change::Accuracy(90)),        // Crabhammer
    u(5, Since(2), 174, Change::Type(Type::Ghost)), // Curse
    u(5, Since(2), 178, Change::Accuracy(100)),     // Cotton Spore
    u(5, Since(2), 184, Change::Accuracy(100)),     // Scary Face
    u(5, Since(2), 198, Change::Accuracy(90)),      // Bone Rush
    u(5, Since(2), 202, Change::Power(75)),         // Giga Drain
    u(5, Since(2), 210, Change::Power(20)),         // Fury Cutter
    u(5, Since(2), 248, Change::Pp(10)),            // Future Sight
    u(5, Since(2), 248, Change::Power(100)),        // Future Sight
    u(5, Since(2), 248, Change::Accuracy(100)),     // Future Sight
    u(5, Since(2), 250, Change::Power(35)),         // Whirlpool
    u(5, Since(2), 250, Change::Accuracy(85)),      // Whirlpool
    u(5, Since(3), 253, Change::Power(90)),         // Uproar
    u(5, Since(3), 328, Change::Power(35)),         // Sand Tomb
    u(5, Since(3), 328, Change::Accuracy(85)),      // Sand Tomb
    u(5, Since(3), 331, Change::Power(25)),         // Bullet Seed
    u(5, Since(3), 333, Change::Power(25)),         // Icicle Spear
    u(5, Since(3), 343, Change::Power(60)),         // Covet
    u(5, Since(3), 350, Change::Accuracy(90)),      // Rock Blast
    u(5, Since(3), 353, Change::Power(140)),        // Doom Desire
    u(5, Since(3), 353, Change::Accuracy(100)),     // Doom Desire
    u(5, Exactly(4), 364, Change::Power(30)),       // Feint
    u(5, Exactly(4), 387, Change::Power(140)),      // Last Resort
    u(5, Exactly(4), 409, Change::Pp(10)),          // Drain Punch
    u(5, Exactly(4), 409, Change::Power(75)),       // Drain Punch
    u(5, Exactly(4), 463, Change::Accuracy(75)),    // Magma Storm
    u(6, Always, 14, Change::Pp(20)),               // Swords Dance
    u(6, Always, 18, Change::PerfectAccuracy),      // Whirlwind
    u(6, Always, 22, Change::Pp(25)),               // Vine Whip
    u(6, Always, 22, Change::Power(45)),            // Vine Whip
    u(6, Always, 42, Change::Power(25)),            // Pin Missile
    u(6, Always, 42, Change::Accuracy(95)),         // Pin Missile
    u(6, Always, 53, Change::Power(90)),            // Flamethrower
    u(6, Always, 56, Change::Power(110)),           // Hydro Pump
    u(6, Always, 57, Change::Power(90)),            // Surf
    u(6, Always, 58, Change::Power(90)),            // Ice Beam
    u(6, Always, 59, Change::Power(110)),           // Blizzard
    u(6, Always, 74, Change::Pp(20)),               // Growth
    u(6, Always, 85, Change::Power(90)),            // Thunderbolt
    u(6, Always, 87, Change::Power(110)),           // Thunder
    u(6, Always, 107, Change::Pp(10)),              // Minimize
    u(6, Always, 112, Change::Pp(20)),              // Barrier
    u(6, Always, 122, Change::Power(30)),           // Lick
    u(6, Always, 123, Change::Power(30)),           // Smog
    u(6, Always, 126, Change::Power(110)),          // Fire Blast
    u(6, Always, 130, Change::Pp(10)),              // Skull Bash
    u(6, Always, 130, Change::Power(130)),          // Skull Bash
    u(6, Always, 137, Change::Accuracy(100)),       // Glare
    u(6, Always, 139, Change::Accuracy(90)),        // Poison Gas
    u(6, Always, 145, Change::Power(40)),           // Bubble
    u(6, Always, 149, Change::Accuracy(100)),       // Psywave
    u(6, Always, 151, Change::Pp(20)),              // Acid Armor
    u(6, Always, 152, Change::Power(100)),          // Crabhammer
    u(6, Since(2), 168, Change::Pp(25)),            // Thief
    u(6, Since(2), 168, Change::Power(60)),         // Thief
    u(6, Since(2), 173, Change::Power(50)),         // Snore
    u(6, Since(2), 210, Change::Power(40)),         // Fury Cutter
    u(6, Since(2), 248, Change::Power(120)),        // Future Sight
    u(6, Since(3), 257, Change::Power(95)),         // Heat Wave
    u(6, Since(3), 261, Change::Accuracy(85)),      // Will-O-Wisp
    u(6, Since(3), 265, Change::Power(70)),         // Smelling Salts
    u(6, Since(3), 282, Change::Power(65)),         // Knock Off
    u(6, Since(3), 309, Change::Power(90)),         // Meteor Mash
    u(6, Since(3), 309, Change::Accuracy(90)),      // Meteor Mash
    u(6, Since(3), 314, Change::Power(60)),         // Air Cutter
    u(6, Since(3), 315, Change::Power(130)),        // Overheat
    u(6, Since(3), 317, Change::Pp(15)),            // Rock Tomb
    u(6, Since(3), 317, Change::Power(60)),         // Rock Tomb
    u(6, Since(3), 317, Change::Accuracy(95)),      // Rock Tomb
    u(6, Since(3), 326, Change::Pp(20)),            // Extrasensory
    u(6, Since(3), 330, Change::Power(90)),         // Muddy Water
    u(6, Since(3), 343, Change::Pp(25)),            // Covet
    u(6, Since(4), 358, Change::Power(70)),         // Wake-Up Slap
    u(6, Since(4), 366, Change::Pp(15)),            // Tailwind
    u(6, Since(4), 372, Change::Power(60)),         // Assurance
    u(6, Since(4), 375, Change::Accuracy(100)),     // Psycho Shift
    u(6, Since(4), 396, Change::Power(80)),         // Aura Sphere
    u(6, Since(4), 403, Change::Pp(15)),            // Air Slash
    u(6, Since(4), 406, Change::Power(85)),         // Dragon Pulse
    u(6, Since(4), 408, Change::Power(80)),         // Power Gem
    u(6, Since(4), 412, Change::Power(90)),         // Energy Ball
    u(6, Since(4), 434, Change::Power(130)),        // Draco Meteor
    u(6, Since(4), 437, Change::Power(130)),        // Leaf Storm
    u(6, Since(4), 441, Change::Accuracy(80)),      // Gunk Shot
    u(6, Since(4), 448, Change::Power(65)),         // Chatter
    u(6, Since(4), 463, Change::Power(100)),        // Magma Storm
    u(6, Exactly(5), 485, Change::Power(120)),      // Synchronoise
    u(6, Exactly(5), 490, Change::Power(65)),       // Low Sweep
    u(6, Exactly(5), 506, Change::Power(65)),       // Hex
    u(6, Exactly(5), 510, Change::Power(60)),       // Incinerate
    u(6, Exactly(5), 518, Change::Power(80)),       // Water Pledge
    u(6, Exactly(5), 519, Change::Power(80)),       // Fire Pledge
    u(6, Exactly(5), 520, Change::Power(80)),       // Grass Pledge
    u(6, Exactly(5), 522, Change::Power(50)),       // Struggle Bug
    u(6, Exactly(5), 524, Change::Power(45)),       // Frost Breath
    u(6, Exactly(5), 480, Change::Power(45)),       // Storm Throw
    u(6, Exactly(5), 533, Change::Pp(15)),          // Sacred Sword
    u(6, Exactly(5), 542, Change::Power(110)),      // Hurricane
    u(6, Exactly(5), 546, Change::Power(120)),      // Techno Blast
    u(7, Always, 141, Change::Power(80)),           // Leech Life
    u(7, Always, 141, Change::Pp(10)),              // Leech Life
    u(7, Always, 66, Change::Pp(20)),               // Submission
    u(7, Always, 33, Change::Power(40)),            // Tackle
    u(7, Always, 86, Change::Accuracy(90)),         // Thunder Wave
    u(7, Since(2), 207, Change::Accuracy(85)),      // Swagger
    u(7, Since(3), 282, Change::Pp(20)),            // Knock Off
    u(7, Since(4), 464, Change::Accuracy(50)),      // Dark Void
    u(7, Since(4), 389, Change::Power(70)),         // Sucker Punch
    u(7, Exactly(6), 597, Change::PerfectAccuracy), // Aromatic Mist
    u(7, Exactly(6), 565, Change::Power(50)),       // Fell Stinger
    u(7, Exactly(6), 560, Change::Power(100)),      // Flying Press
    u(7, Exactly(6), 561, Change::Pp(10)),          // Mat Block
    u(7, Exactly(6), 595, Change::Power(75)),       // Mystical Fire
    u(7, Exactly(6), 570, Change::Power(65)),       // Parabolic Charge
    u(7, Exactly(6), 576, Change::PerfectAccuracy), // Topsy-Turvy
    u(7, Exactly(6), 594, Change::Category(MoveCategory::Special)), // Water Shuriken
    u(8, Since(2), 229, Change::Power(50)),         // Rapid Spin
    u(8, Exactly(7), 718, Change::Power(120)),      // Multi-Attack
    u(9, Always, 105, Change::Pp(5)),               // Recover
    u(9, Always, 135, Change::Pp(5)),               // Soft-Boiled
    u(9, Always, 156, Change::Pp(5)),               // Rest
    u(9, Since(2), 208, Change::Pp(5)),             // Milk Drink
    u(9, Since(3), 295, Change::Power(95)),         // Luster Purge
    u(9, Since(3), 296, Change::Power(95)),         // Mist Ball
    u(9, Since(3), 303, Change::Pp(5)),             // Slack Off
    u(9, Since(4), 355, Change::Pp(5)),             // Roost
    u(9, Since(7), 659, Change::Pp(5)),             // Shore Up
    u(9, Since(8), 803, Change::Power(55)),         // Grassy Glide
    u(9, Since(8), 817, Change::Power(75)),         // Wicked Blow
    u(9, Since(8), 824, Change::Power(120)),        // Glacial Lance
];

/// Updates moves to a later generation according to [`ppr_settings::MoveDataSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveUpdater;

impl Step<RunData> for MoveUpdater {
    fn id(&self) -> u64 {
        step_ids::MOVE_UPDATES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.move_data.update_to_generation.is_some() && !data.game.moves.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        if let Some(to) = ctx.data.settings.move_data.update_to_generation {
            update(&mut ctx.data.game, to);
        }
        Ok(())
    }
}

/// Every change of the generations after the game's, up to `to`.
fn update(game: &mut GameData, to: u8) {
    let generation = game.identity.generation.number();
    let perfect = game.moves.iter().find(|m| m.number == SWIFT).map_or(0, |m| m.accuracy);
    for up in UPDATES.iter().filter(|up| up.to > generation && up.to <= to) {
        let applies = match up.when {
            Always => true,
            Since(g) => generation >= g,
            Exactly(g) => generation == g,
        };
        let Some(mv) = game.moves.iter_mut().find(|m| m.number == up.number) else {
            continue;
        };
        if !applies {
            continue;
        }
        match up.change {
            Change::Power(p) => mv.power = p,
            Change::Pp(p) => mv.pp = p,
            Change::Accuracy(a) => mv.accuracy = a,
            Change::PerfectAccuracy => mv.accuracy = perfect,
            Change::Type(t) => mv.move_type = Some(t),
            Change::Category(c) => mv.category = c,
        }
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::Move;

    use super::*;
    use crate::randomizers::testing::emerald;

    fn game() -> GameData {
        let mut g = emerald(Vec::new());
        g.moves = (1..=354)
            .map(|n| Move::basic(n, &format!("M{n}"), 40, Type::Normal, MoveCategory::Physical))
            .collect();
        g.moves[usize::from(SWIFT) - 1].accuracy = 0;
        g
    }

    fn of(g: &GameData, n: MoveId) -> &Move {
        &g.moves[usize::from(n) - 1]
    }

    #[test]
    fn updates_up_to_the_chosen_generation() {
        let mut g = game();
        update(&mut g, 5);
        assert_eq!(of(&g, 19).power, 90, "Fly, Gen 4");
        assert_eq!(of(&g, 33).power, 50, "Tackle, Gen 5");
        assert_eq!(of(&g, 291).power, 80, "Dive, Gen 4 for Gen 3 games");
        assert_eq!(of(&g, 57).power, 40, "Surf changed in Gen 6");
        assert_eq!(
            of(&g, 2).move_type,
            Some(Type::Normal),
            "Karate Chop changed before Gen 3"
        );
        update(&mut g, 9);
        assert_eq!(
            (of(&g, 57).power, of(&g, 33).power),
            (90, 40),
            "Surf; Tackle again in Gen 7"
        );
        assert_eq!(of(&g, 18).accuracy, 0, "Whirlwind never misses from Gen 6");
        assert_eq!(of(&g, 105).pp, 5, "Recover, Gen 9");
    }

    #[test]
    fn gen1_games_get_gen2s_changes_and_only_their_moves() {
        let mut g = crate::randomizers::testing::gen1(Vec::new(), false);
        g.moves = game().moves.into_iter().take(165).collect();
        update(&mut g, 9);
        assert_eq!(of(&g, 2).move_type, Some(Type::Fighting), "Karate Chop, Gen 2");
        assert_eq!(of(&g, 91).power, 80, "Dig, Gen 4");
        assert_eq!(of(&g, 18).accuracy, 0, "Whirlwind never misses, like Swift");
        assert_eq!(g.moves.len(), 165);
    }

    #[test]
    fn earlier_generations_change_nothing() {
        let mut g = game();
        update(&mut g, 3);
        assert_eq!(g, game());
    }
}
