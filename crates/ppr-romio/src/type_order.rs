//! The order in which the type charts are written, in every generation.

use ppr_model::Type;

/// The types in type chart order.
pub(crate) const TYPE_ORDER: [Type; 17] = [
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
];
