use ppr_model::ExpCurve;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which species a standardized EXP curve goes to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ExpCurveScope {
    /// Legendaries get Slow, the rest the chosen curve.
    #[default]
    Legendaries,
    /// The strongest legendaries get Slow, the rest the chosen curve.
    StrongLegendaries,
    /// Every species gets the chosen curve.
    All,
}

/// EXP curve settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExpCurveSettings {
    /// Give species this curve; `None` keeps theirs.
    pub standardize: Option<ExpCurve>,
    /// Which species get it.
    pub apply_to: ExpCurveScope,
}
