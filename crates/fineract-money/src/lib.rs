//! `fineract-money` — Deterministic financial arithmetic, `MonetaryCurrency`, `Money`,
//! `FineractDecimal`, `CurrencyData`, and `FineractRoundingMode` for Apache Fineract.
//!
//! Ported from `org.apache.fineract.organisation.monetary.domain.*` and
//! `org.apache.fineract.organisation.monetary.data.*`.
//!
//! ### Constitutional Invariant
//! Never use `f32` or `f64` for monetary amounts, interest rates, fees, taxes, or exchange rates.
//! All calculations use `rust_decimal::Decimal` wrapped in [`FineractDecimal`] or [`Money`] with
//! explicit [`FineractRoundingMode`].

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::{Add, Deref, Mul, Neg, Sub};
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

pub use rust_decimal::Decimal;
pub use rust_decimal::RoundingStrategy;
pub use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors raised by monetary and decimal arithmetic operations.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum MoneyError {
    /// Attempted to combine or compare [`Money`] instances with different ISO currency codes.
    #[error("Currency mismatch: expected '{expected}', got '{actual}'")]
    CurrencyMismatch { expected: String, actual: String },

    /// Attempted to divide a monetary or decimal amount by zero.
    #[error("Division by zero in monetary calculation")]
    DivisionByZero,

    /// Arithmetic operation exceeded `rust_decimal::Decimal` bounds (96-bit mantissa).
    #[error("Arithmetic overflow in monetary calculation")]
    Overflow,

    /// Rounding mode integer code was outside Java's `RoundingMode` range `0..=6`.
    #[error("Invalid rounding mode code: {0}. Valid values are 0..=6 (corresponding to java.math.RoundingMode ordinals)")]
    InvalidRoundingCode(i32),
}

impl MoneyError {
    /// Helper constructor for [`MoneyError::CurrencyMismatch`].
    #[inline]
    pub fn currency_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self::CurrencyMismatch {
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

/// Deterministic rounding modes mapping Java's `java.math.RoundingMode` integer codes (`0..=6`)
/// to [`rust_decimal::RoundingStrategy`].
///
/// Mirrors `org.apache.fineract.organisation.monetary.domain.MoneyHelper.validateAndConvertRoundingMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum FineractRoundingMode {
    /// Round away from zero (`java.math.RoundingMode.UP`, ordinal `0`).
    Up = 0,
    /// Round towards zero / truncate (`java.math.RoundingMode.DOWN`, ordinal `1`).
    Down = 1,
    /// Round towards positive infinity (`java.math.RoundingMode.CEILING`, ordinal `2`).
    Ceiling = 2,
    /// Round towards negative infinity (`java.math.RoundingMode.FLOOR`, ordinal `3`).
    Floor = 3,
    /// Round towards nearest neighbor; if equidistant, round away from zero (`java.math.RoundingMode.HALF_UP`, ordinal `4`).
    HalfUp = 4,
    /// Round towards nearest neighbor; if equidistant, round towards zero (`java.math.RoundingMode.HALF_DOWN`, ordinal `5`).
    HalfDown = 5,
    /// Round towards nearest neighbor; if equidistant, round towards even neighbor (`java.math.RoundingMode.HALF_EVEN`, ordinal `6`).
    #[default]
    HalfEven = 6,
}

/// Type alias for ergonomic compatibility across Fineract domain crates.
pub type RoundingMode = FineractRoundingMode;

impl FineractRoundingMode {
    /// Converts a Java `java.math.RoundingMode` integer ordinal (`0..=6`) into [`FineractRoundingMode`].
    #[inline]
    pub const fn from_code(code: i32) -> Result<Self, MoneyError> {
        match code {
            0 => Ok(Self::Up),
            1 => Ok(Self::Down),
            2 => Ok(Self::Ceiling),
            3 => Ok(Self::Floor),
            4 => Ok(Self::HalfUp),
            5 => Ok(Self::HalfDown),
            6 => Ok(Self::HalfEven),
            other => Err(MoneyError::InvalidRoundingCode(other)),
        }
    }

    /// Returns the Java `java.math.RoundingMode` integer ordinal (`0..=6`).
    #[inline]
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Alias for [`FineractRoundingMode::code`].
    #[inline]
    pub const fn as_code(self) -> i32 {
        self as i32
    }

    /// Maps this rounding mode to the exact corresponding [`rust_decimal::RoundingStrategy`].
    #[inline]
    pub const fn to_rounding_strategy(self) -> RoundingStrategy {
        match self {
            Self::Up => RoundingStrategy::AwayFromZero,
            Self::Down => RoundingStrategy::ToZero,
            Self::Ceiling => RoundingStrategy::ToPositiveInfinity,
            Self::Floor => RoundingStrategy::ToNegativeInfinity,
            Self::HalfUp => RoundingStrategy::MidpointAwayFromZero,
            Self::HalfDown => RoundingStrategy::MidpointTowardZero,
            Self::HalfEven => RoundingStrategy::MidpointNearestEven,
        }
    }

    /// Alias for [`FineractRoundingMode::to_rounding_strategy`].
    #[inline]
    pub const fn strategy(self) -> RoundingStrategy {
        self.to_rounding_strategy()
    }

    /// Converts a [`rust_decimal::RoundingStrategy`] into [`FineractRoundingMode`].
    #[inline]
    pub const fn from_rounding_strategy(strategy: RoundingStrategy) -> Self {
        match strategy {
            RoundingStrategy::AwayFromZero => Self::Up,
            RoundingStrategy::ToZero => Self::Down,
            RoundingStrategy::ToPositiveInfinity => Self::Ceiling,
            RoundingStrategy::ToNegativeInfinity => Self::Floor,
            RoundingStrategy::MidpointAwayFromZero => Self::HalfUp,
            RoundingStrategy::MidpointTowardZero => Self::HalfDown,
            RoundingStrategy::MidpointNearestEven => Self::HalfEven,
        }
    }

    /// Returns the canonical Java `RoundingMode` enum constant name.
    #[inline]
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Ceiling => "CEILING",
            Self::Floor => "FLOOR",
            Self::HalfUp => "HALF_UP",
            Self::HalfDown => "HALF_DOWN",
            Self::HalfEven => "HALF_EVEN",
        }
    }
}

impl TryFrom<i32> for FineractRoundingMode {
    type Error = MoneyError;

    #[inline]
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::from_code(value)
    }
}

impl TryFrom<i64> for FineractRoundingMode {
    type Error = MoneyError;

    #[inline]
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        let code = i32::try_from(value).map_err(|_| MoneyError::InvalidRoundingCode(i32::MAX))?;
        Self::from_code(code)
    }
}

impl From<FineractRoundingMode> for i32 {
    #[inline]
    fn from(mode: FineractRoundingMode) -> Self {
        mode.code()
    }
}

impl From<FineractRoundingMode> for RoundingStrategy {
    #[inline]
    fn from(mode: FineractRoundingMode) -> Self {
        mode.to_rounding_strategy()
    }
}

impl From<RoundingStrategy> for FineractRoundingMode {
    #[inline]
    fn from(strategy: RoundingStrategy) -> Self {
        Self::from_rounding_strategy(strategy)
    }
}

impl fmt::Display for FineractRoundingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

impl FromStr for FineractRoundingMode {
    type Err = MoneyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if let Ok(code) = trimmed.parse::<i32>() {
            return Self::from_code(code);
        }
        match trimmed.to_ascii_uppercase().as_str() {
            "UP" | "ROUND_UP" => Ok(Self::Up),
            "DOWN" | "ROUND_DOWN" => Ok(Self::Down),
            "CEILING" | "ROUND_CEILING" => Ok(Self::Ceiling),
            "FLOOR" | "ROUND_FLOOR" => Ok(Self::Floor),
            "HALF_UP" | "HALFUP" | "ROUND_HALF_UP" => Ok(Self::HalfUp),
            "HALF_DOWN" | "HALFDOWN" | "ROUND_HALF_DOWN" => Ok(Self::HalfDown),
            "HALF_EVEN" | "HALFEVEN" | "ROUND_HALF_EVEN" => Ok(Self::HalfEven),
            _ => Err(MoneyError::InvalidRoundingCode(-1)),
        }
    }
}

/// Newtype wrapper around [`rust_decimal::Decimal`] providing explicit scale and
/// cash-multiple rounding operations matching Apache Fineract's `BigDecimal` semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FineractDecimal(pub Decimal);

impl FineractDecimal {
    /// Zero (`0`).
    pub const ZERO: Self = Self(Decimal::ZERO);
    /// One (`1`).
    pub const ONE: Self = Self(Decimal::ONE);
    /// Ten (`10`).
    pub const TEN: Self = Self(Decimal::TEN);
    /// One hundred (`100`).
    pub const ONE_HUNDRED: Self = Self(Decimal::ONE_HUNDRED);

    /// Wraps a [`Decimal`] in [`FineractDecimal`].
    #[inline]
    pub const fn new(value: Decimal) -> Self {
        Self(value)
    }

    /// Creates a [`FineractDecimal`] from a mantissa and scale.
    #[inline]
    pub fn from_scale(num: i64, scale: u32) -> Self {
        Self(Decimal::new(num, scale))
    }

    /// Returns the underlying [`Decimal`] by value.
    #[inline]
    pub const fn inner(self) -> Decimal {
        self.0
    }

    /// Returns a reference to the underlying [`Decimal`].
    #[inline]
    pub const fn as_decimal(&self) -> &Decimal {
        &self.0
    }

    /// Unwraps the underlying [`Decimal`].
    #[inline]
    pub const fn into_inner(self) -> Decimal {
        self.0
    }

    /// Returns the number of digits after the decimal point.
    #[inline]
    pub const fn scale(&self) -> u32 {
        self.0.scale()
    }

    /// Rounds this decimal to `scale` decimal places using `mode`, preserving exact trailing scale
    /// (matching Java's `BigDecimal.setScale(scale, roundingMode)`).
    ///
    /// Also canonicalizes `-0.00` to positive `0.00` to match Java's `BigDecimal` representation.
    #[inline]
    pub fn round_to(self, scale: u32, mode: FineractRoundingMode) -> Self {
        let mut rounded = self.0.round_dp_with_strategy(scale, mode.to_rounding_strategy());
        if rounded.is_zero() {
            rounded.set_sign_positive(true);
        }
        rounded.rescale(scale);
        Self(rounded)
    }

    /// Normalizes this decimal into integer multiples of `multiples` using `mode`, matching
    /// `Money.roundToMultiplesOf(BigDecimal, Integer)` in Fineract:
    ///
    /// `(value / multiples).round_to_scale(0, mode) * multiples` when `multiples > 0`,
    /// or returns `self` unchanged if `multiples <= 0`.
    #[inline]
    pub fn normalize_multiples_of(self, multiples: i32, mode: FineractRoundingMode) -> Self {
        if multiples <= 0 {
            return self;
        }
        let mult_dec = Decimal::from(multiples);
        let Some(quotient) = self.0.checked_div(mult_dec) else {
            return self;
        };
        let quotient_rounded = quotient.round_dp_with_strategy(0, mode.to_rounding_strategy());
        let mut scaled = quotient_rounded.checked_mul(mult_dec).unwrap_or(self.0);
        if scaled.is_zero() {
            scaled.set_sign_positive(true);
        }
        Self(scaled)
    }

    /// Strips trailing fractional zeros (matching `BigDecimal.stripTrailingZeros()`).
    #[inline]
    pub fn strip_trailing_zeros(self) -> Self {
        let mut normalized = self.0.normalize();
        if normalized.is_zero() {
            normalized.set_sign_positive(true);
        }
        Self(normalized)
    }

    /// Returns `true` if the value is numerically zero.
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns `true` if the value is strictly greater than zero.
    #[inline]
    pub fn is_greater_than_zero(&self) -> bool {
        self.0 > Decimal::ZERO
    }

    /// Returns `true` if the value is strictly less than zero.
    #[inline]
    pub fn is_less_than_zero(&self) -> bool {
        self.0 < Decimal::ZERO
    }

    /// Returns the absolute value.
    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Returns the negated value, canonicalizing `-0` to `0`.
    #[inline]
    pub fn negated(self) -> Self {
        if self.0.is_zero() {
            return self;
        }
        Self(-self.0)
    }

    /// Clamps negative values to `0` (matching `MathUtil.negativeToZero`).
    #[inline]
    pub fn negative_to_zero(self) -> Self {
        if self.is_greater_than_zero() {
            self
        } else {
            Self::ZERO
        }
    }

    /// Checked addition returning [`MoneyError::Overflow`] on overflow.
    #[inline]
    pub fn checked_add(self, rhs: Self) -> Result<Self, MoneyError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    /// Checked subtraction returning [`MoneyError::Overflow`] on overflow.
    #[inline]
    pub fn checked_sub(self, rhs: Self) -> Result<Self, MoneyError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    /// Checked multiplication returning [`MoneyError::Overflow`] on overflow.
    #[inline]
    pub fn checked_mul(self, rhs: Self) -> Result<Self, MoneyError> {
        self.0
            .checked_mul(rhs.0)
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    /// Checked division rounded to `scale` decimal places using `mode`.
    #[inline]
    pub fn checked_div(
        self,
        rhs: Self,
        scale: u32,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        if rhs.0.is_zero() {
            return Err(MoneyError::DivisionByZero);
        }
        let quotient = self.0.checked_div(rhs.0).ok_or(MoneyError::Overflow)?;
        Ok(Self(quotient).round_to(scale, mode))
    }
}

impl Deref for FineractDecimal {
    type Target = Decimal;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Decimal> for FineractDecimal {
    #[inline]
    fn from(value: Decimal) -> Self {
        Self(value)
    }
}

impl From<FineractDecimal> for Decimal {
    #[inline]
    fn from(value: FineractDecimal) -> Self {
        value.0
    }
}

impl From<i32> for FineractDecimal {
    #[inline]
    fn from(value: i32) -> Self {
        Self(Decimal::from(value))
    }
}

impl From<i64> for FineractDecimal {
    #[inline]
    fn from(value: i64) -> Self {
        Self(Decimal::from(value))
    }
}

impl From<u32> for FineractDecimal {
    #[inline]
    fn from(value: u32) -> Self {
        Self(Decimal::from(value))
    }
}

impl From<u64> for FineractDecimal {
    #[inline]
    fn from(value: u64) -> Self {
        Self(Decimal::from(value))
    }
}

impl fmt::Display for FineractDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for FineractDecimal {
    type Err = rust_decimal::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Decimal::from_str(s).map(Self)
    }
}

impl Add for FineractDecimal {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for FineractDecimal {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul for FineractDecimal {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Neg for FineractDecimal {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        self.negated()
    }
}

/// Data Transfer Object matching `org.apache.fineract.organisation.monetary.data.CurrencyData`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyData {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(alias = "decimal_places")]
    pub decimal_places: u32,
    #[serde(default, alias = "in_multiples_of", skip_serializing_if = "Option::is_none")]
    pub in_multiples_of: Option<i32>,
    #[serde(default, alias = "display_symbol", skip_serializing_if = "Option::is_none")]
    pub display_symbol: Option<String>,
    #[serde(default, alias = "name_code", skip_serializing_if = "Option::is_none")]
    pub name_code: Option<String>,
    #[serde(default, alias = "display_label", skip_serializing_if = "Option::is_none")]
    pub display_label: Option<String>,
}

impl CurrencyData {
    /// Full constructor matching `CurrencyData(code, name, decimalPlaces, inMultiplesOf, displaySymbol, nameCode)`.
    /// Automatically computes `display_label` using Fineract's `generateDisplayLabel()` logic.
    pub fn new(
        code: impl Into<String>,
        name: Option<String>,
        decimal_places: u32,
        in_multiples_of: Option<i32>,
        display_symbol: Option<String>,
        name_code: Option<String>,
    ) -> Self {
        let code = code.into();
        let display_label = Some(Self::compute_display_label(
            &code,
            name.as_deref(),
            display_symbol.as_deref(),
        ));
        Self {
            code,
            name,
            decimal_places,
            in_multiples_of,
            display_symbol,
            name_code,
            display_label,
        }
    }

    /// Minimal constructor matching `CurrencyData(code, decimalPlaces, inMultiplesOf)`.
    pub fn from_minimal(
        code: impl Into<String>,
        decimal_places: u32,
        in_multiples_of: Option<i32>,
    ) -> Self {
        Self {
            code: code.into(),
            name: None,
            decimal_places,
            in_multiples_of,
            display_symbol: None,
            name_code: None,
            display_label: None,
        }
    }

    /// Constructor from currency code alone, matching `CurrencyData(String code)`.
    pub fn from_code(code: impl Into<String>) -> Self {
        Self::from_minimal(code, 0, None)
    }

    /// Blank currency data matching `CurrencyData.blank()`.
    pub fn blank() -> Self {
        Self::new(
            "",
            Some(String::new()),
            0,
            Some(0),
            Some(String::new()),
            Some(String::new()),
        )
    }

    /// Generates the human-readable display label matching `CurrencyData.generateDisplayLabel()`:
    /// - Appends `"{name} "` if `name` is present
    /// - Appends `"({display_symbol})"` if `display_symbol` is present and non-whitespace
    /// - Otherwise appends `"[{code}]"`
    pub fn compute_display_label(
        code: &str,
        name: Option<&str>,
        display_symbol: Option<&str>,
    ) -> String {
        let mut builder = String::with_capacity(20);
        if let Some(n) = name {
            builder.push_str(n);
            builder.push(' ');
        }
        match display_symbol {
            Some(sym) if !sym.trim().is_empty() => {
                builder.push('(');
                builder.push_str(sym);
                builder.push(')');
            }
            _ => {
                builder.push('[');
                builder.push_str(code);
                builder.push(']');
            }
        }
        builder
    }

    /// Generates (or regenerates) the display label for this [`CurrencyData`].
    pub fn generate_display_label(&self) -> String {
        Self::compute_display_label(
            &self.code,
            self.name.as_deref(),
            self.display_symbol.as_deref(),
        )
    }

    /// Converts this [`CurrencyData`] into a [`MonetaryCurrency`].
    #[inline]
    pub fn to_monetary_currency(&self) -> MonetaryCurrency {
        MonetaryCurrency::from_currency_data(self)
    }
}

/// Embeddable domain currency definition matching
/// `org.apache.fineract.organisation.monetary.domain.MonetaryCurrency`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonetaryCurrency {
    pub code: String,
    #[serde(alias = "digits_after_decimal", alias = "decimal_places", alias = "decimalPlaces")]
    pub digits_after_decimal: u32,
    #[serde(default, alias = "in_multiples_of", skip_serializing_if = "Option::is_none")]
    pub in_multiples_of: Option<i32>,
}

impl MonetaryCurrency {
    /// Creates a new [`MonetaryCurrency`].
    #[inline]
    pub fn new(
        code: impl Into<String>,
        digits_after_decimal: u32,
        in_multiples_of: Option<i32>,
    ) -> Self {
        Self {
            code: code.into(),
            digits_after_decimal,
            in_multiples_of,
        }
    }

    /// Convenience constructor with `in_multiples_of = None`.
    #[inline]
    pub fn with_code(code: impl Into<String>, digits_after_decimal: u32) -> Self {
        Self::new(code, digits_after_decimal, None)
    }

    /// Standard USD (`"USD"`, 2 decimal places, no cash multiple).
    #[inline]
    pub fn usd() -> Self {
        Self::new("USD", 2, None)
    }

    /// Standard EUR (`"EUR"`, 2 decimal places, no cash multiple).
    #[inline]
    pub fn eur() -> Self {
        Self::new("EUR", 2, None)
    }

    /// Returns the ISO currency code.
    #[inline]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Returns the number of digits after the decimal point.
    #[inline]
    pub const fn digits_after_decimal(&self) -> u32 {
        self.digits_after_decimal
    }

    /// Alias for [`MonetaryCurrency::digits_after_decimal`].
    #[inline]
    pub const fn decimal_places(&self) -> u32 {
        self.digits_after_decimal
    }

    /// Returns the optional cash rounding multiple.
    #[inline]
    pub const fn in_multiples_of(&self) -> Option<i32> {
        self.in_multiples_of
    }

    /// Creates a [`MonetaryCurrency`] from a [`CurrencyData`] DTO.
    #[inline]
    pub fn from_currency_data(data: &CurrencyData) -> Self {
        Self {
            code: data.code.clone(),
            digits_after_decimal: data.decimal_places,
            in_multiples_of: data.in_multiples_of,
        }
    }

    /// Creates a [`MonetaryCurrency`] from an [`ApplicationCurrency`] entity.
    #[inline]
    pub fn from_application_currency(app_currency: &ApplicationCurrency) -> Self {
        Self {
            code: app_currency.code.clone(),
            digits_after_decimal: app_currency.decimal_places,
            in_multiples_of: app_currency.in_multiples_of,
        }
    }

    /// Converts this [`MonetaryCurrency`] into a minimal [`CurrencyData`] DTO.
    #[inline]
    pub fn to_data(&self) -> CurrencyData {
        CurrencyData::from_minimal(
            self.code.clone(),
            self.digits_after_decimal,
            self.in_multiples_of,
        )
    }
}

impl From<&CurrencyData> for MonetaryCurrency {
    #[inline]
    fn from(data: &CurrencyData) -> Self {
        Self::from_currency_data(data)
    }
}

impl From<CurrencyData> for MonetaryCurrency {
    #[inline]
    fn from(data: CurrencyData) -> Self {
        Self {
            code: data.code,
            digits_after_decimal: data.decimal_places,
            in_multiples_of: data.in_multiples_of,
        }
    }
}

impl From<&MonetaryCurrency> for CurrencyData {
    #[inline]
    fn from(currency: &MonetaryCurrency) -> Self {
        currency.to_data()
    }
}

impl From<MonetaryCurrency> for CurrencyData {
    #[inline]
    fn from(currency: MonetaryCurrency) -> Self {
        Self::from_minimal(
            currency.code,
            currency.digits_after_decimal,
            currency.in_multiples_of,
        )
    }
}

/// Domain model matching `org.apache.fineract.organisation.monetary.domain.ApplicationCurrency` (`m_currency`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationCurrency {
    pub id: Option<i64>,
    pub code: String,
    pub name: String,
    pub decimal_places: u32,
    pub in_multiples_of: Option<i32>,
    pub name_code: String,
    pub display_symbol: Option<String>,
}

impl ApplicationCurrency {
    /// Creates a new [`ApplicationCurrency`].
    pub fn new(
        code: impl Into<String>,
        name: impl Into<String>,
        decimal_places: u32,
        in_multiples_of: Option<i32>,
        name_code: impl Into<String>,
        display_symbol: Option<String>,
    ) -> Self {
        Self {
            id: None,
            code: code.into(),
            name: name.into(),
            decimal_places,
            in_multiples_of,
            name_code: name_code.into(),
            display_symbol,
        }
    }

    /// Clones an [`ApplicationCurrency`] with overridden `decimal_places` and `in_multiples_of`,
    /// matching `ApplicationCurrency.from(currency, decimalPlaces, inMultiplesOf)`.
    pub fn from_currency(
        currency: &Self,
        decimal_places: u32,
        in_multiples_of: Option<i32>,
    ) -> Self {
        Self {
            id: currency.id,
            code: currency.code.clone(),
            name: currency.name.clone(),
            decimal_places,
            in_multiples_of,
            name_code: currency.name_code.clone(),
            display_symbol: currency.display_symbol.clone(),
        }
    }

    /// Converts this [`ApplicationCurrency`] to [`CurrencyData`].
    pub fn to_data(&self) -> CurrencyData {
        CurrencyData::new(
            self.code.clone(),
            Some(self.name.clone()),
            self.decimal_places,
            self.in_multiples_of,
            self.display_symbol.clone(),
            Some(self.name_code.clone()),
        )
    }

    /// Converts this [`ApplicationCurrency`] to [`MonetaryCurrency`].
    pub fn to_monetary_currency(&self) -> MonetaryCurrency {
        MonetaryCurrency::from_application_currency(self)
    }
}

/// Tenant-scoped rounding mode registry matching `org.apache.fineract.organisation.monetary.domain.MoneyHelper`.
pub struct MoneyHelper;

impl MoneyHelper {
    /// Standard precision (`19`) matching `MoneyHelper.PRECISION`.
    pub const PRECISION: u32 = 19;

    fn cache() -> &'static RwLock<HashMap<String, FineractRoundingMode>> {
        static CACHE: OnceLock<RwLock<HashMap<String, FineractRoundingMode>>> = OnceLock::new();
        CACHE.get_or_init(|| RwLock::new(HashMap::new()))
    }

    /// Validates and converts an integer rounding mode code (`0..=6`) into [`FineractRoundingMode`].
    #[inline]
    pub const fn validate_and_convert_rounding_mode(
        rounding_mode_value: i32,
    ) -> Result<FineractRoundingMode, MoneyError> {
        FineractRoundingMode::from_code(rounding_mode_value)
    }

    /// Initializes the rounding mode for `tenant_identifier`.
    pub fn initialize_tenant_rounding_mode(
        tenant_identifier: impl Into<String>,
        rounding_mode_value: i32,
    ) -> Result<FineractRoundingMode, MoneyError> {
        let mode = Self::validate_and_convert_rounding_mode(rounding_mode_value)?;
        if let Ok(mut guard) = Self::cache().write() {
            guard.insert(tenant_identifier.into(), mode);
        }
        Ok(mode)
    }

    /// Updates the rounding mode for `tenant_identifier`.
    pub fn update_tenant_rounding_mode(
        tenant_identifier: impl Into<String>,
        rounding_mode_value: i32,
    ) -> Result<FineractRoundingMode, MoneyError> {
        Self::initialize_tenant_rounding_mode(tenant_identifier, rounding_mode_value)
    }

    /// Retrieves the configured rounding mode for `tenant_identifier`, or `None` if not initialized.
    pub fn get_tenant_rounding_mode(tenant_identifier: &str) -> Option<FineractRoundingMode> {
        Self::cache()
            .read()
            .ok()
            .and_then(|guard| guard.get(tenant_identifier).copied())
    }

    /// Checks whether a tenant has an initialized rounding mode.
    pub fn is_tenant_initialized(tenant_identifier: &str) -> bool {
        Self::cache()
            .read()
            .map(|guard| guard.contains_key(tenant_identifier))
            .unwrap_or(false)
    }

    /// Returns the set of initialized tenant identifiers.
    pub fn get_initialized_tenants() -> HashSet<String> {
        Self::cache()
            .read()
            .map(|guard| guard.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Clears cached rounding mode for a single tenant.
    pub fn clear_cache_for_tenant(tenant_identifier: &str) {
        if let Ok(mut guard) = Self::cache().write() {
            guard.remove(tenant_identifier);
        }
    }

    /// Clears all cached tenant rounding modes.
    pub fn clear_cache() {
        if let Ok(mut guard) = Self::cache().write() {
            guard.clear();
        }
    }
}

/// Immutable currency-bound monetary value matching `org.apache.fineract.organisation.monetary.domain.Money`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    pub currency: MonetaryCurrency,
    pub amount: Decimal,
}

impl Money {
    /// Constructs a [`Money`] instance normalized and rounded to `currency.digits_after_decimal`
    /// (and `currency.in_multiples_of` when `digits_after_decimal == 0` and `amount > 0`),
    /// matching `Money.of(MonetaryCurrency, BigDecimal, MathContext)` in `Money.java`.
    pub fn of(currency: MonetaryCurrency, amount: Decimal, mode: FineractRoundingMode) -> Self {
        let mut amount_scaled = FineractDecimal(amount).strip_trailing_zeros();

        // Match Money.java lines 48-51:
        // if (currency.getInMultiplesOf() != null && currency.getDecimalPlaces() == 0
        //     && currency.getInMultiplesOf() > 0 && MathUtil.isGreaterThanZero(amountScaled))
        if let Some(multiples) = currency.in_multiples_of {
            if currency.digits_after_decimal == 0
                && multiples > 0
                && amount_scaled.is_greater_than_zero()
            {
                amount_scaled = amount_scaled.normalize_multiples_of(multiples, mode);
            }
        }

        let final_amount = amount_scaled
            .round_to(currency.digits_after_decimal, mode)
            .into_inner();

        Self {
            currency,
            amount: final_amount,
        }
    }

    /// Constructs a zero [`Money`] instance for `currency` with scale set to `currency.digits_after_decimal`,
    /// matching `Money.zero(MonetaryCurrency)` in `Money.java`.
    pub fn zero(currency: MonetaryCurrency) -> Self {
        let mut zero = Decimal::ZERO;
        zero.rescale(currency.digits_after_decimal);
        Self {
            currency,
            amount: zero,
        }
    }

    /// Returns a zero [`Money`] of the same currency as `self` (matching `money.zero()` instance method).
    #[inline]
    pub fn zero_of(&self) -> Self {
        Self::zero(self.currency.clone())
    }

    /// Returns a reference to the [`MonetaryCurrency`].
    #[inline]
    pub const fn currency(&self) -> &MonetaryCurrency {
        &self.currency
    }

    /// Returns the [`CurrencyData`] representation of this money's currency.
    #[inline]
    pub fn currency_data(&self) -> CurrencyData {
        self.currency.to_data()
    }

    /// Returns the ISO currency code.
    #[inline]
    pub fn currency_code(&self) -> &str {
        &self.currency.code
    }

    /// Returns the optional cash rounding multiple of the currency.
    #[inline]
    pub const fn in_multiples_of(&self) -> Option<i32> {
        self.currency.in_multiples_of
    }

    /// Returns the normalized [`Decimal`] amount.
    #[inline]
    pub const fn amount(&self) -> Decimal {
        self.amount
    }

    /// Returns the amount wrapped in [`FineractDecimal`].
    #[inline]
    pub const fn fine_amount(&self) -> FineractDecimal {
        FineractDecimal(self.amount)
    }

    /// Returns `None` if the amount is zero, otherwise `Some(self.amount)`,
    /// matching `Money.getAmountDefaultedToNullIfZero()`.
    #[inline]
    pub fn amount_defaulted_to_null_if_zero(&self) -> Option<Decimal> {
        if self.is_zero() {
            None
        } else {
            Some(self.amount)
        }
    }

    /// Checks whether `other` shares the same currency code as `self`.
    #[inline]
    pub fn is_same_currency(&self, other: &Self) -> bool {
        self.currency.code == other.currency.code
    }

    #[inline]
    fn check_currency_equal(&self, other: &Self) -> Result<(), MoneyError> {
        if !self.is_same_currency(other) {
            return Err(MoneyError::CurrencyMismatch {
                expected: self.currency.code.clone(),
                actual: other.currency.code.clone(),
            });
        }
        Ok(())
    }

    /// Adds `other` to `self`, returning [`MoneyError::CurrencyMismatch`] if currencies differ
    /// or [`MoneyError::Overflow`] on arithmetic overflow.
    pub fn plus(&self, other: &Self) -> Result<Self, MoneyError> {
        self.check_currency_equal(other)?;
        if other.is_zero() {
            return Ok(self.clone());
        }
        let sum = self
            .amount
            .checked_add(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(
            self.currency.clone(),
            sum,
            FineractRoundingMode::default(),
        ))
    }

    /// Adds `other` to `self` using an explicit [`FineractRoundingMode`].
    pub fn plus_with_mode(
        &self,
        other: &Self,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        self.check_currency_equal(other)?;
        if other.is_zero() {
            return Ok(self.clone());
        }
        let sum = self
            .amount
            .checked_add(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), sum, mode))
    }

    /// Adds a raw [`Decimal`] amount to `self` using `mode` (matching `Money.plus(BigDecimal, MathContext)`).
    pub fn plus_amount(
        &self,
        amount_to_add: Decimal,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        if amount_to_add.is_zero() {
            return Ok(self.clone());
        }
        let sum = self
            .amount
            .checked_add(amount_to_add)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), sum, mode))
    }

    /// Alias for [`Money::plus`] matching `Money.add(Money)` in `Money.java`.
    #[inline]
    pub fn add(&self, other: &Self) -> Result<Self, MoneyError> {
        self.plus(other)
    }

    /// Subtracts `other` from `self`, returning [`MoneyError::CurrencyMismatch`] if currencies differ
    /// or [`MoneyError::Overflow`] on arithmetic overflow.
    pub fn minus(&self, other: &Self) -> Result<Self, MoneyError> {
        self.check_currency_equal(other)?;
        if other.is_zero() {
            return Ok(self.clone());
        }
        let diff = self
            .amount
            .checked_sub(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(
            self.currency.clone(),
            diff,
            FineractRoundingMode::default(),
        ))
    }

    /// Subtracts `other` from `self` using an explicit [`FineractRoundingMode`].
    pub fn minus_with_mode(
        &self,
        other: &Self,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        self.check_currency_equal(other)?;
        if other.is_zero() {
            return Ok(self.clone());
        }
        let diff = self
            .amount
            .checked_sub(other.amount)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), diff, mode))
    }

    /// Subtracts a raw [`Decimal`] amount from `self` using `mode` (matching `Money.minus(BigDecimal, MathContext)`).
    pub fn minus_amount(
        &self,
        amount_to_subtract: Decimal,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        if amount_to_subtract.is_zero() {
            return Ok(self.clone());
        }
        let diff = self
            .amount
            .checked_sub(amount_to_subtract)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), diff, mode))
    }

    /// Multiplies `self` by `factor` and rounds according to `mode` and currency rules,
    /// matching `Money.multipliedBy(BigDecimal, MathContext)`.
    pub fn multiplied_by(
        &self,
        factor: Decimal,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        if factor == Decimal::ONE {
            return Ok(self.clone());
        }
        let product = self
            .amount
            .checked_mul(factor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), product, mode))
    }

    /// Multiplies `self` by `factor` while explicitly retaining currency scale,
    /// matching `Money.multiplyRetainScale(BigDecimal, MathContext)`.
    #[inline]
    pub fn multiply_retain_scale(
        &self,
        factor: Decimal,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        self.multiplied_by(factor, mode)
    }

    /// Divides `self` by `divisor` and rounds according to `mode` and currency rules,
    /// matching `Money.dividedBy(BigDecimal, MathContext)`.
    ///
    /// Returns [`MoneyError::DivisionByZero`] if `divisor.is_zero()`.
    pub fn divided_by(
        &self,
        divisor: Decimal,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        if divisor.is_zero() {
            return Err(MoneyError::DivisionByZero);
        }
        if divisor == Decimal::ONE {
            return Ok(self.clone());
        }
        let quotient = self
            .amount
            .checked_div(divisor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), quotient, mode))
    }

    /// Computes `(self * percentage) / 100` rounded according to `mode`,
    /// matching `Money.percentageOf(BigDecimal, MathContext)`.
    pub fn percentage_of(
        &self,
        percentage: Decimal,
        mode: FineractRoundingMode,
    ) -> Result<Self, MoneyError> {
        let scaled = self
            .amount
            .checked_mul(percentage)
            .ok_or(MoneyError::Overflow)?;
        let quotient = scaled
            .checked_div(Decimal::ONE_HUNDRED)
            .ok_or(MoneyError::Overflow)?;
        Ok(Self::of(self.currency.clone(), quotient, mode))
    }

    /// Negates the monetary amount, returning `self` unchanged if zero (matching `Money.negated()`).
    pub fn negated(&self) -> Self {
        if self.is_zero() {
            return self.clone();
        }
        let mut neg = -self.amount;
        neg.rescale(self.currency.digits_after_decimal);
        Self {
            currency: self.currency.clone(),
            amount: neg,
        }
    }

    /// Returns the absolute value of this [`Money`] (matching `Money.abs()`).
    pub fn abs(&self) -> Self {
        if self.is_less_than_zero() {
            self.negated()
        } else {
            self.clone()
        }
    }

    /// Rounds this [`Money`] to multiples of `in_multiples_of` using `mode`,
    /// matching Fineract's `Money.roundToMultiplesOf(Money, Integer, MathContext)` and `MoneyHelper`.
    pub fn round_to_multiples_of(&self, in_multiples_of: i32, mode: FineractRoundingMode) -> Self {
        let scaled = FineractDecimal(self.amount)
            .normalize_multiples_of(in_multiples_of, mode)
            .into_inner();
        Self::of(self.currency.clone(), scaled, mode)
    }

    /// Static helper matching `Money.roundToMultiplesOf(BigDecimal, Integer)` with explicit rounding mode.
    #[inline]
    pub fn round_decimal_to_multiples_of(
        existing_val: Decimal,
        in_multiples_of: i32,
        mode: FineractRoundingMode,
    ) -> Decimal {
        FineractDecimal(existing_val)
            .normalize_multiples_of(in_multiples_of, mode)
            .into_inner()
    }

    /// Sums an iterator of [`Money`] references (matching `Money.total(Iterable)`).
    pub fn total<'a, I>(monies: I) -> Result<Self, MoneyError>
    where
        I: IntoIterator<Item = &'a Self>,
    {
        let mut iter = monies.into_iter();
        let first = iter.next().ok_or(MoneyError::Overflow)?;
        let mut acc = first.clone();
        for item in iter {
            acc = acc.plus(item)?;
        }
        Ok(acc)
    }

    /// Returns `true` if `self.amount` is zero (matching `Money.isZero()`).
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// Returns `true` if `self.amount > 0` (matching `Money.isGreaterThanZero()`).
    #[inline]
    pub fn is_greater_than_zero(&self) -> bool {
        self.amount > Decimal::ZERO
    }

    /// Returns `true` if `self.amount < 0` (matching `Money.isLessThanZero()`).
    #[inline]
    pub fn is_less_than_zero(&self) -> bool {
        self.amount < Decimal::ZERO
    }

    /// Compares `self` and `other`, returning [`MoneyError::CurrencyMismatch`] if currencies differ.
    #[inline]
    pub fn try_cmp(&self, other: &Self) -> Result<Ordering, MoneyError> {
        self.check_currency_equal(other)?;
        Ok(self.amount.cmp(&other.amount))
    }

    /// Returns `true` if `self` and `other` share the same currency code and `self.amount > other.amount`
    /// (matching `Money.isGreaterThan(Money)`).
    #[inline]
    pub fn is_greater_than(&self, other: &Self) -> bool {
        self.is_same_currency(other) && self.amount > other.amount
    }

    /// Returns `true` if `self` and `other` share the same currency code and `self.amount >= other.amount`
    /// (matching `Money.isGreaterThanOrEqualTo(Money)`).
    #[inline]
    pub fn is_greater_than_or_equal_to(&self, other: &Self) -> bool {
        self.is_same_currency(other) && self.amount >= other.amount
    }

    /// Returns `true` if `self` and `other` share the same currency code and `self.amount < other.amount`
    /// (matching `Money.isLessThan(Money)`).
    #[inline]
    pub fn is_less_than(&self, other: &Self) -> bool {
        self.is_same_currency(other) && self.amount < other.amount
    }

    /// Returns `true` if `self` and `other` share the same currency code and `self.amount <= other.amount`
    /// (matching `Money.isLessThanOrEqualTo(Money)`).
    #[inline]
    pub fn is_less_than_or_equal_to(&self, other: &Self) -> bool {
        self.is_same_currency(other) && self.amount <= other.amount
    }

    /// Returns `true` if `self` and `other` share the same currency code and `self.amount == other.amount`
    /// (matching `Money.isEqualTo(Money)`).
    #[inline]
    pub fn is_equal_to(&self, other: &Self) -> bool {
        self.is_same_currency(other) && self.amount == other.amount
    }

    /// Returns `true` if `!self.is_equal_to(other)` (matching `Money.isNotEqualTo(Money)`).
    #[inline]
    pub fn is_not_equal_to(&self, other: &Self) -> bool {
        !self.is_equal_to(other)
    }

    /// Clamps negative [`Money`] to zero (matching `MathUtil.negativeToZero(Money)`).
    #[inline]
    pub fn negative_to_zero(&self) -> Self {
        if self.is_greater_than_zero() {
            self.clone()
        } else {
            Self::zero(self.currency.clone())
        }
    }

    /// Subtracts `other` from `self` and clamps negative results to zero (matching `MathUtil.minusToZero`).
    #[inline]
    pub fn minus_to_zero(&self, other: &Self) -> Result<Self, MoneyError> {
        Ok(self.minus(other)?.negative_to_zero())
    }
}

impl PartialOrd for Money {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.is_same_currency(other) {
            Some(self.amount.cmp(&other.amount))
        } else {
            None
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.currency.code, self.amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn usd() -> MonetaryCurrency {
        MonetaryCurrency::new("USD", 2, None)
    }

    fn eur() -> MonetaryCurrency {
        MonetaryCurrency::new("EUR", 2, None)
    }

    fn jpy() -> MonetaryCurrency {
        MonetaryCurrency::new("JPY", 0, None)
    }

    fn ugx_mult_50() -> MonetaryCurrency {
        MonetaryCurrency::new("UGX", 0, Some(50))
    }

    #[test]
    fn test_rounding_mode_codes_and_strategies() {
        let mappings = [
            (0, FineractRoundingMode::Up, RoundingStrategy::AwayFromZero, "UP"),
            (1, FineractRoundingMode::Down, RoundingStrategy::ToZero, "DOWN"),
            (
                2,
                FineractRoundingMode::Ceiling,
                RoundingStrategy::ToPositiveInfinity,
                "CEILING",
            ),
            (
                3,
                FineractRoundingMode::Floor,
                RoundingStrategy::ToNegativeInfinity,
                "FLOOR",
            ),
            (
                4,
                FineractRoundingMode::HalfUp,
                RoundingStrategy::MidpointAwayFromZero,
                "HALF_UP",
            ),
            (
                5,
                FineractRoundingMode::HalfDown,
                RoundingStrategy::MidpointTowardZero,
                "HALF_DOWN",
            ),
            (
                6,
                FineractRoundingMode::HalfEven,
                RoundingStrategy::MidpointNearestEven,
                "HALF_EVEN",
            ),
        ];

        for (code, expected_mode, expected_strategy, java_name) in mappings {
            let mode = FineractRoundingMode::from_code(code).unwrap();
            assert_eq!(mode, expected_mode);
            assert_eq!(mode.code(), code);
            assert_eq!(mode.to_rounding_strategy(), expected_strategy);
            assert_eq!(
                FineractRoundingMode::from_rounding_strategy(expected_strategy),
                expected_mode
            );
            assert_eq!(mode.to_string(), java_name);
            assert_eq!(FineractRoundingMode::from_str(java_name).unwrap(), mode);
        }

        assert_eq!(
            FineractRoundingMode::from_code(-1),
            Err(MoneyError::InvalidRoundingCode(-1))
        );
        assert_eq!(
            FineractRoundingMode::from_code(7),
            Err(MoneyError::InvalidRoundingCode(7))
        );
    }

    #[test]
    fn test_fineract_decimal_rounding_all_modes() {
        let pos_mid = FineractDecimal::new(dec!(2.5));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::Up).inner(), dec!(3));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::Down).inner(), dec!(2));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::Ceiling).inner(), dec!(3));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::Floor).inner(), dec!(2));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::HalfUp).inner(), dec!(3));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::HalfDown).inner(), dec!(2));
        assert_eq!(pos_mid.round_to(0, FineractRoundingMode::HalfEven).inner(), dec!(2));

        let pos_mid_odd = FineractDecimal::new(dec!(3.5));
        assert_eq!(
            pos_mid_odd.round_to(0, FineractRoundingMode::HalfEven).inner(),
            dec!(4)
        );

        let neg_mid = FineractDecimal::new(dec!(-2.5));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::Up).inner(), dec!(-3));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::Down).inner(), dec!(-2));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::Ceiling).inner(), dec!(-2));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::Floor).inner(), dec!(-3));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::HalfUp).inner(), dec!(-3));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::HalfDown).inner(), dec!(-2));
        assert_eq!(neg_mid.round_to(0, FineractRoundingMode::HalfEven).inner(), dec!(-2));
    }

    #[test]
    fn test_fineract_decimal_normalize_multiples_of() {
        let val = FineractDecimal::new(dec!(75));
        assert_eq!(
            val.normalize_multiples_of(50, FineractRoundingMode::Up).inner(),
            dec!(100)
        );
        assert_eq!(
            val.normalize_multiples_of(50, FineractRoundingMode::Down).inner(),
            dec!(50)
        );
        assert_eq!(
            val.normalize_multiples_of(50, FineractRoundingMode::HalfEven).inner(),
            dec!(100)
        );
        assert_eq!(
            val.normalize_multiples_of(0, FineractRoundingMode::Up).inner(),
            dec!(75)
        );
        assert_eq!(
            val.normalize_multiples_of(-50, FineractRoundingMode::Up).inner(),
            dec!(75)
        );
    }

    #[test]
    fn test_money_arithmetic_and_comparisons() {
        let ten = Money::of(usd(), dec!(10), FineractRoundingMode::HalfEven);
        let one = Money::of(usd(), dec!(1), FineractRoundingMode::HalfEven);
        let zero = Money::zero(usd());
        let neg_five = Money::of(usd(), dec!(-5), FineractRoundingMode::HalfEven);

        assert_eq!(ten.to_string(), "USD 10.00");
        assert_eq!(zero.to_string(), "USD 0.00");
        assert!(zero.is_zero());
        assert!(!ten.is_zero());

        let eleven = ten.plus(&one).unwrap();
        assert_eq!(eleven.amount(), dec!(11.00));

        let nine = ten.minus(&one).unwrap();
        assert_eq!(nine.amount(), dec!(9.00));

        let thirty = ten
            .multiplied_by(dec!(3), FineractRoundingMode::HalfEven)
            .unwrap();
        assert_eq!(thirty.amount(), dec!(30.00));

        let five = ten
            .divided_by(dec!(2), FineractRoundingMode::HalfEven)
            .unwrap();
        assert_eq!(five.amount(), dec!(5.00));

        assert_eq!(
            ten.divided_by(Decimal::ZERO, FineractRoundingMode::HalfEven),
            Err(MoneyError::DivisionByZero)
        );

        assert_eq!(neg_five.abs(), five);
        assert_eq!(ten.negated().amount(), dec!(-10.00));
        assert_eq!(zero.negated(), zero);

        assert!(ten.is_greater_than(&one));
        assert!(ten.is_greater_than_or_equal_to(&ten));
        assert!(one.is_less_than(&ten));
        assert!(one.is_less_than_or_equal_to(&one));
        assert!(ten.is_equal_to(&Money::of(usd(), dec!(10.000), FineractRoundingMode::Up)));
    }

    #[test]
    fn test_currency_mismatch_error() {
        let ten_usd = Money::of(usd(), dec!(10), FineractRoundingMode::HalfEven);
        let five_eur = Money::of(eur(), dec!(5), FineractRoundingMode::HalfEven);

        assert_eq!(
            ten_usd.plus(&five_eur),
            Err(MoneyError::CurrencyMismatch {
                expected: "USD".to_string(),
                actual: "EUR".to_string(),
            })
        );
        assert_eq!(
            ten_usd.minus(&five_eur),
            Err(MoneyError::CurrencyMismatch {
                expected: "USD".to_string(),
                actual: "EUR".to_string(),
            })
        );
        assert!(!ten_usd.is_equal_to(&five_eur));
        assert!(!ten_usd.is_greater_than(&five_eur));
        assert!(!ten_usd.is_less_than(&five_eur));
    }

    #[test]
    fn test_in_multiples_of_cash_rounding() {
        let ccy = ugx_mult_50();
        let m_up = Money::of(ccy.clone(), dec!(75), FineractRoundingMode::Up);
        assert_eq!(m_up.amount(), dec!(100));

        let m_down = Money::of(ccy.clone(), dec!(75), FineractRoundingMode::Down);
        assert_eq!(m_down.amount(), dec!(50));

        let usd_75 = Money::of(usd(), dec!(75), FineractRoundingMode::Up);
        assert_eq!(usd_75.amount(), dec!(75.00));
        let usd_rounded = usd_75.round_to_multiples_of(50, FineractRoundingMode::Up);
        assert_eq!(usd_rounded.amount(), dec!(100.00));
    }

    #[test]
    fn test_currency_data_display_label() {
        let usd_data = CurrencyData::new(
            "USD",
            Some("US Dollar".to_string()),
            2,
            None,
            Some("$".to_string()),
            Some("currency.USD".to_string()),
        );
        assert_eq!(usd_data.display_label.as_deref(), Some("US Dollar ($)"));

        let no_symbol = CurrencyData::new(
            "KES",
            Some("Kenyan Shilling".to_string()),
            2,
            None,
            Some("   ".to_string()),
            Some("currency.KES".to_string()),
        );
        assert_eq!(
            no_symbol.display_label.as_deref(),
            Some("Kenyan Shilling [KES]")
        );

        let blank = CurrencyData::blank();
        assert_eq!(blank.display_label.as_deref(), Some(" []"));
    }

    #[test]
    fn test_money_helper_tenant_cache() {
        MoneyHelper::clear_cache();
        assert!(!MoneyHelper::is_tenant_initialized("default"));
        let mode = MoneyHelper::initialize_tenant_rounding_mode("default", 6).unwrap();
        assert_eq!(mode, FineractRoundingMode::HalfEven);
        assert!(MoneyHelper::is_tenant_initialized("default"));
        assert_eq!(
            MoneyHelper::get_tenant_rounding_mode("default"),
            Some(FineractRoundingMode::HalfEven)
        );
        MoneyHelper::clear_cache_for_tenant("default");
        assert!(!MoneyHelper::is_tenant_initialized("default"));
    }

    proptest! {
        #[test]
        fn prop_money_addition_subtraction_inverse(
            a_cents in -1_000_000_000i64..1_000_000_000i64,
            b_cents in -1_000_000_000i64..1_000_000_000i64,
            mode_code in 0i32..=6i32,
        ) {
            let mode = FineractRoundingMode::from_code(mode_code).unwrap();
            let ccy = usd();
            let a = Money::of(ccy.clone(), Decimal::new(a_cents, 2), mode);
            let b = Money::of(ccy, Decimal::new(b_cents, 2), mode);

            let sum = a.plus(&b).unwrap();
            let back = sum.minus(&b).unwrap();
            prop_assert!(back.is_equal_to(&a));
            prop_assert_eq!(sum.amount().scale(), 2);
            prop_assert_eq!(back.amount().scale(), 2);
        }

        #[test]
        fn prop_money_negated_and_abs_invariants(
            cents in -1_000_000_000i64..1_000_000_000i64,
        ) {
            let ccy = usd();
            let m = Money::of(ccy.clone(), Decimal::new(cents, 2), FineractRoundingMode::HalfEven);
            let neg = m.negated();
            let double_neg = neg.negated();
            prop_assert!(double_neg.is_equal_to(&m));

            let sum = m.plus(&neg).unwrap();
            prop_assert!(sum.is_zero());

            let abs_m = m.abs();
            prop_assert!(abs_m.is_greater_than_or_equal_to(&Money::zero(ccy)));
        }

        #[test]
        fn prop_in_multiples_of_divisibility(
            amount in 1i64..1_000_000i64,
            multiple in 1i32..500i32,
            mode_code in 0i32..=6i32,
        ) {
            let mode = FineractRoundingMode::from_code(mode_code).unwrap();
            let ccy = MonetaryCurrency::new("UGX", 0, Some(multiple));
            let m = Money::of(ccy, Decimal::from(amount), mode);
            let rem = m.amount() % Decimal::from(multiple);
            prop_assert_eq!(rem, Decimal::ZERO);
            prop_assert_eq!(m.amount().scale(), 0);
        }

        #[test]
        fn prop_jpy_zero_scale(
            amount_milli in -1_000_000_000i64..1_000_000_000i64,
            mode_code in 0i32..=6i32,
        ) {
            let mode = FineractRoundingMode::from_code(mode_code).unwrap();
            let m = Money::of(jpy(), Decimal::new(amount_milli, 3), mode);
            prop_assert_eq!(m.amount().scale(), 0);
        }
    }
}
