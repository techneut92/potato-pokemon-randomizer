//! Standardized EXP curves.

use ppr_model::{ExpCurve, GameData};
use ppr_settings::ExpCurveScope;

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "EXP Curves";

/// Standardizes EXP curves according to [`ppr_settings::ExpCurveSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ExpCurveStandardizer;

impl Step<RunData> for ExpCurveStandardizer {
    fn id(&self) -> u64 {
        step_ids::EXP_CURVES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.exp_curves.standardize.is_some()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let s = ctx.data.settings.exp_curves.clone();
        if let Some(curve) = s.standardize {
            standardize(&mut ctx.data.game, curve, s.apply_to);
        }
        Ok(())
    }
}

/// `curve` for every species and alternate forme, but Slow for the (strong) legendaries unless `scope`
/// is [`ExpCurveScope::All`].
fn standardize(game: &mut GameData, curve: ExpCurve, scope: ExpCurveScope) {
    for sp in &mut game.species {
        let slow = match scope {
            ExpCurveScope::Legendaries => sp.is_legendary(),
            ExpCurveScope::StrongLegendaries => sp.is_strong_legendary(),
            ExpCurveScope::All => false,
        };
        sp.growth_curve = if slow { ExpCurve::Slow } else { curve };
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::Type;

    use super::*;
    use crate::randomizers::testing::{emerald, species};

    fn curves(scope: ExpCurveScope) -> Vec<ExpCurve> {
        let mut g = emerald([1, 144, 150].map(|n| species(n, Type::Normal, None, 50)).to_vec());
        standardize(&mut g, ExpCurve::Fast, scope);
        g.species.iter().map(|s| s.growth_curve).collect()
    }

    #[test]
    fn legendaries_get_slow_unless_all_are_standardized() {
        use ExpCurve::{Fast, Slow};
        assert_eq!(curves(ExpCurveScope::Legendaries), [Fast, Slow, Slow]);
        assert_eq!(curves(ExpCurveScope::StrongLegendaries), [Fast, Fast, Slow]);
        assert_eq!(curves(ExpCurveScope::All), [Fast, Fast, Fast]);
    }
}
