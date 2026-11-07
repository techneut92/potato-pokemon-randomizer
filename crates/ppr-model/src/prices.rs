//! What item prices each generation can store, shared by the ROM handlers and the manual editor.
//!
//! - Gen 1 stores six decimal digits (up to 999,999); its TMs keep one decimal digit of thousands (up to 9,000).
//! - Gen 2–4 store a 16-bit number (up to 65,535).
//! - Gen 5–7 store the price in tens as a 16-bit number (up to 655,350, in steps of 10).
//!
//! Deliberate choices:
//! - Handlers write a price that does not fit as [`PriceFormat::fit`] makes it: rounded down to the step, as the
//!   original does, and capped at the highest price. The original keeps only the low bits or digits of a price that
//!   is too high, so 70,000 would cost 4,464 in Gen 2.
//! - Gen 1 TMs go up to 9,000: the game reads their digit as the thousands of a decimal price, so 10 and up would not
//!   be a valid digit. The original writes any number below 16 there.

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Generation;

/// How a game stores an item's price: the highest price and the step between prices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PriceFormat {
    /// The highest price.
    pub max: u32,
    /// Prices are whole multiples of this.
    pub step: u32,
}

impl PriceFormat {
    /// Gen 1: six decimal digits.
    pub const DECIMAL: Self = Self { max: 999_999, step: 1 };
    /// Gen 1 TMs: one decimal digit of thousands.
    pub const THOUSANDS: Self = Self {
        max: 9_000,
        step: 1_000,
    };
    /// Gen 2–4: a 16-bit number.
    pub const WORD: Self = Self { max: 65_535, step: 1 };
    /// Gen 5–7: a 16-bit number of tens.
    pub const TENS: Self = Self { max: 655_350, step: 10 };

    /// The format of an item's price in a generation; `tm` is whether the item is a TM, which matters in Gen 1 only.
    pub const fn of(generation: Generation, tm: bool) -> Self {
        match generation.number() {
            1 if tm => Self::THOUSANDS,
            1 => Self::DECIMAL,
            2..=4 => Self::WORD,
            _ => Self::TENS,
        }
    }

    /// Whether the price can be stored as it is.
    ///
    /// # Errors
    /// When the price is above the highest or not a multiple of the step.
    pub const fn check(self, price: u32) -> Result<(), PriceError> {
        if price > self.max {
            Err(PriceError::TooHigh { max: self.max })
        } else if !price.is_multiple_of(self.step) {
            Err(PriceError::Step { step: self.step })
        } else {
            Ok(())
        }
    }

    /// The price as it is stored: rounded down to the step and at most the highest price.
    pub const fn fit(self, price: u32) -> u32 {
        let capped = if price > self.max { self.max } else { price };
        capped - capped % self.step
    }

    /// The number the game stores for the price, after [`Self::fit`]: the price in steps.
    pub const fn stored(self, price: u32) -> u32 {
        self.fit(price) / self.step
    }

    /// [`Self::stored`] as a 16-bit number, for the formats that store one: [`Self::WORD`] and [`Self::TENS`].
    pub fn stored_u16(self, price: u32) -> u16 {
        u16::try_from(self.stored(price)).unwrap_or(u16::MAX)
    }
}

/// Why a price cannot be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceError {
    /// Above the highest price.
    TooHigh {
        /// The highest price.
        max: u32,
    },
    /// Not a whole multiple of the step.
    Step {
        /// The step between prices.
        step: u32,
    },
}

impl fmt::Display for PriceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooHigh { max } => write!(f, "the price can be at most {max}"),
            Self::Step { step } => write!(f, "the price must be a multiple of {step}"),
        }
    }
}

impl std::error::Error for PriceError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation(n: u8) -> Generation {
        Generation::new(n).expect("valid")
    }

    #[test]
    fn each_generation_has_its_format() {
        assert_eq!(PriceFormat::of(generation(1), false), PriceFormat::DECIMAL);
        assert_eq!(PriceFormat::of(generation(1), true), PriceFormat::THOUSANDS);
        for n in 2..=4 {
            assert_eq!(PriceFormat::of(generation(n), true), PriceFormat::WORD);
        }
        for n in 5..=7 {
            assert_eq!(PriceFormat::of(generation(n), false), PriceFormat::TENS);
        }
    }

    #[test]
    fn check_takes_the_limits_and_steps() {
        assert_eq!(PriceFormat::DECIMAL.check(999_999), Ok(()));
        assert_eq!(
            PriceFormat::DECIMAL.check(1_000_000),
            Err(PriceError::TooHigh { max: 999_999 })
        );
        assert_eq!(PriceFormat::THOUSANDS.check(9_000), Ok(()));
        assert_eq!(
            PriceFormat::THOUSANDS.check(10_000),
            Err(PriceError::TooHigh { max: 9_000 })
        );
        assert_eq!(
            PriceFormat::THOUSANDS.check(2_500),
            Err(PriceError::Step { step: 1_000 })
        );
        assert_eq!(PriceFormat::WORD.check(65_535), Ok(()));
        assert_eq!(
            PriceFormat::WORD.check(65_536),
            Err(PriceError::TooHigh { max: 65_535 })
        );
        assert_eq!(PriceFormat::TENS.check(655_350), Ok(()));
        assert_eq!(
            PriceFormat::TENS.check(655_360),
            Err(PriceError::TooHigh { max: 655_350 })
        );
        assert_eq!(PriceFormat::TENS.check(1_235), Err(PriceError::Step { step: 10 }));
        assert_eq!(PriceFormat::TENS.check(0), Ok(()));
    }

    #[test]
    fn fit_rounds_down_and_caps() {
        assert_eq!(PriceFormat::DECIMAL.fit(1_234_567), 999_999);
        assert_eq!(PriceFormat::THOUSANDS.fit(2_999), 2_000);
        assert_eq!(PriceFormat::THOUSANDS.fit(15_000), 9_000);
        assert_eq!(PriceFormat::WORD.fit(70_000), 65_535);
        assert_eq!(PriceFormat::TENS.fit(1_239), 1_230);
        assert_eq!(PriceFormat::TENS.fit(700_000), 655_350);
        assert_eq!(PriceFormat::TENS.stored(100_000), 10_000);
        assert_eq!(PriceFormat::TENS.stored(u32::MAX), 65_535);
        assert_eq!(PriceFormat::THOUSANDS.stored(9_999), 9);
        assert_eq!(PriceFormat::WORD.stored_u16(u32::MAX), u16::MAX);
        assert_eq!(PriceFormat::TENS.stored_u16(4_567), 456);
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(
            PriceError::TooHigh { max: 65_535 }.to_string(),
            "the price can be at most 65535"
        );
        assert_eq!(
            PriceError::Step { step: 10 }.to_string(),
            "the price must be a multiple of 10"
        );
    }
}
