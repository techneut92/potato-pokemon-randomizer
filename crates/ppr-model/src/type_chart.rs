//! The type chart: how effective each attacking type is against each defending type.

use serde::{Deserialize, Serialize};

use crate::Type;

/// Damage multiplier of one attacking type against one defending type (limited to the values a type chart can hold).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Effectiveness {
    /// No effect.
    Zero,
    /// Not very effective.
    Half,
    /// Normal damage.
    #[default]
    Neutral,
    /// Super effective.
    Double,
}

/// Effectiveness of every pair of the chart's types; pairs not set are [`Effectiveness::Neutral`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeChart {
    types: Vec<Type>,
    /// Row = attacker, column = defender, both in the order of `types`.
    table: Vec<Effectiveness>,
}

impl TypeChart {
    /// A chart of `types` where everything is neutral. Duplicate types are dropped.
    pub fn neutral(types: &[Type]) -> Self {
        let mut unique: Vec<Type> = Vec::with_capacity(types.len());
        for &t in types {
            if !unique.contains(&t) {
                unique.push(t);
            }
        }
        let n = unique.len();
        Self {
            types: unique,
            table: vec![Effectiveness::Neutral; n * n],
        }
    }

    /// The chart's types, in its order.
    pub fn types(&self) -> &[Type] {
        &self.types
    }

    fn index(&self, t: Type) -> Option<usize> {
        self.types.iter().position(|&x| x == t)
    }

    fn cell(&self, attacker: Type, defender: Type) -> Option<usize> {
        Some(self.index(attacker)? * self.types.len() + self.index(defender)?)
    }

    /// How `attacker` does against `defender`; `None` when either is not in the chart.
    pub fn get(&self, attacker: Type, defender: Type) -> Option<Effectiveness> {
        self.cell(attacker, defender).map(|i| self.table[i])
    }

    /// Sets one pair. Returns `false`, changing nothing, when either type is not in the chart.
    pub fn set(&mut self, attacker: Type, defender: Type, effectiveness: Effectiveness) -> bool {
        match self.cell(attacker, defender) {
            Some(i) => {
                self.table[i] = effectiveness;
                true
            }
            None => false,
        }
    }

    /// The defending types `attacker` has this effectiveness against, in chart order.
    pub fn when_attacking(&self, attacker: Type, effectiveness: Effectiveness) -> Vec<Type> {
        self.types
            .iter()
            .copied()
            .filter(|&d| self.get(attacker, d) == Some(effectiveness))
            .collect()
    }

    /// The attacking types that have this effectiveness against `defender`, in chart order.
    pub fn when_defending(&self, defender: Type, effectiveness: Effectiveness) -> Vec<Type> {
        self.types
            .iter()
            .copied()
            .filter(|&a| self.get(a, defender) == Some(effectiveness))
            .collect()
    }

    /// The defending types `attacker` is super effective against, in chart order.
    pub fn super_effective_when_attacking(&self, attacker: Type) -> Vec<Type> {
        self.when_attacking(attacker, Effectiveness::Double)
    }

    /// How many pairs are not neutral.
    pub fn non_neutral_count(&self) -> usize {
        self.table.iter().filter(|&&e| e != Effectiveness::Neutral).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_query() {
        let mut chart = TypeChart::neutral(&[Type::Fire, Type::Water, Type::Grass, Type::Fire]);
        assert_eq!(chart.types(), [Type::Fire, Type::Water, Type::Grass]);
        assert!(chart.set(Type::Water, Type::Fire, Effectiveness::Double));
        assert!(chart.set(Type::Fire, Type::Water, Effectiveness::Half));
        assert!(
            !chart.set(Type::Dark, Type::Fire, Effectiveness::Double),
            "not in the chart"
        );
        assert_eq!(chart.get(Type::Water, Type::Fire), Some(Effectiveness::Double));
        assert_eq!(chart.get(Type::Grass, Type::Fire), Some(Effectiveness::Neutral));
        assert_eq!(chart.get(Type::Dark, Type::Fire), None);
        assert_eq!(chart.super_effective_when_attacking(Type::Water), [Type::Fire]);
        assert_eq!(chart.when_attacking(Type::Fire, Effectiveness::Half), [Type::Water]);
        assert_eq!(chart.non_neutral_count(), 2);
    }
}
