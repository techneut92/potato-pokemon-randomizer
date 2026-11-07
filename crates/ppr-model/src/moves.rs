//! Moves, with what their effects say about them (stat changes, statuses, critical hit chance …).

use serde::{Deserialize, Serialize};

use crate::Type;

/// Move number, the same in every generation; 0 is no move.
pub type MoveId = u16;

/// Struggle, which randomizers leave alone.
pub const STRUGGLE: MoveId = 165;
/// Swift, whose accuracy is the game's "never misses".
pub const SWIFT: MoveId = 129;

/// Physical, special or status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum MoveCategory {
    Physical,
    Special,
    Status,
}

/// Which stat a move changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum StatChangeType {
    Attack,
    Defense,
    SpecialAttack,
    SpecialDefense,
    Speed,
    Accuracy,
    Evasion,
    All,
    Special,
}

/// Whom a move's stat changes hit, and whether it does damage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum StatChangeMoveType {
    #[default]
    NoneOrUnknown,
    NoDamageTarget,
    NoDamageUser,
    NoDamageAll,
    NoDamageAlly,
    DamageTarget,
    DamageUser,
}

/// A status condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum StatusType {
    Paralyze,
    Sleep,
    Freeze,
    Burn,
    Poison,
    Confusion,
    ToxicPoison,
}

/// Whether a status move does damage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum StatusMoveType {
    #[default]
    NoneOrUnknown,
    NoDamage,
    Damage,
}

/// A move's critical hit chance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum CriticalChance {
    None,
    #[default]
    Normal,
    Increased,
    Guaranteed,
}

/// One stat change of a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatChange {
    /// The stat.
    pub stat: StatChangeType,
    /// Stages up (positive) or down.
    pub stages: i8,
    /// Chance in percent, for damaging moves; 0 for the others.
    pub percent_chance: u8,
}

/// One move of the game.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts about a move; not a state machine"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    /// Number, the same in every generation.
    pub number: MoveId,
    /// Name as the game spells it.
    pub name: String,
    /// The game's effect number.
    pub effect: u16,
    /// Base power; 0 for moves without one.
    pub power: u16,
    /// Power points.
    pub pp: u8,
    /// Accuracy in percent; the game's "never misses" value for moves that can't miss.
    pub accuracy: u8,
    /// Type; `None` for the "???" type of Curse before Gen 5.
    pub move_type: Option<Type>,
    /// Physical, special or status.
    pub category: MoveCategory,
    /// The game's target number.
    pub target: u8,
    /// Priority.
    pub priority: i8,
    /// Hits on average, in hundredths: 100 for most, 271 for Triple Kick.
    pub hit_count_hundredths: u16,
    /// Makes contact.
    pub makes_contact: bool,
    /// Whom its stat changes hit.
    pub stat_change_move_type: StatChangeMoveType,
    /// Its stat changes.
    pub stat_changes: Vec<StatChange>,
    /// Whether its status does damage.
    pub status_move_type: StatusMoveType,
    /// The status it causes.
    pub status: Option<StatusType>,
    /// Chance of the status in percent, for damaging moves.
    pub status_percent_chance: u8,
    /// Critical hit chance.
    pub critical_chance: CriticalChance,
    /// Chance to flinch in percent.
    pub flinch_percent_chance: u8,
    /// Damage taken back, in percent.
    pub recoil_percent: u8,
    /// Damage healed, in percent.
    pub absorb_percent: u8,
    /// Charges a turn first.
    pub is_charge: bool,
    /// Needs a turn to recharge.
    pub is_recharge: bool,
    /// A punch.
    pub is_punch: bool,
    /// A sound move.
    pub is_sound: bool,
    /// Binds or traps the target.
    pub is_trap: bool,
}

impl Move {
    /// A move with only these set; the rest as for a plain move.
    pub fn basic(number: MoveId, name: &str, power: u16, move_type: Type, category: MoveCategory) -> Self {
        Self {
            number,
            name: name.to_owned(),
            effect: 0,
            power,
            pp: 20,
            accuracy: 100,
            move_type: Some(move_type),
            category,
            target: 0,
            priority: 0,
            hit_count_hundredths: 100,
            makes_contact: false,
            stat_change_move_type: StatChangeMoveType::NoneOrUnknown,
            stat_changes: Vec::new(),
            status_move_type: StatusMoveType::NoneOrUnknown,
            status: None,
            status_percent_chance: 0,
            critical_chance: CriticalChance::Normal,
            flinch_percent_chance: 0,
            recoil_percent: 0,
            absorb_percent: 0,
            is_charge: false,
            is_recharge: false,
            is_punch: false,
            is_sound: false,
            is_trap: false,
        }
    }
}

/// A move a species learns by level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelUpMove {
    /// The level; 0 is on evolving (Gen 7).
    pub level: u8,
    /// The move.
    pub move_id: MoveId,
}

impl LevelUpMove {
    /// The move `move_id` at `level`.
    pub const fn new(level: u8, move_id: MoveId) -> Self {
        Self { level, move_id }
    }
}
