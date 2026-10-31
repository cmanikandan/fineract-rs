//! Two-pass locale-aware (`en_US`, `de_DE`, `fr_FR`, `es_ES`, etc.) and `dateFormat`-aware JSON
//! command parser ported from Apache Fineract's:
//! - `org.apache.fineract.infrastructure.core.serialization.FromJsonHelper`
//! - `org.apache.fineract.infrastructure.core.serialization.JsonParserHelper`
//! - `org.apache.fineract.infrastructure.core.api.JsonCommand`

use std::collections::HashMap;
use crate::ExternalId;
use std::fmt;
use std::str::FromStr;

use chrono::NaiveDate;
use fineract_validation_rs::{
    ApiErrorArg, ApiParameterError, FineractError, PlatformApiDataValidationException,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Constructs a wire-compatible [`FineractError::DataValidation`] wrapping a single [`ApiParameterError`].
fn validation_error(
    globalisation_code: &str,
    default_user_message: impl Into<String>,
    parameter_name: &str,
    rejected_value: Option<Value>,
) -> FineractError {
    let msg = default_user_message.into();
    let mut args = Vec::new();
    if let Some(ref val) = rejected_value {
        args.push(ApiErrorArg { value: val.clone() });
    }
    let param_err = ApiParameterError {
        developer_message: msg.clone(),
        default_user_message: msg,
        user_message_globalisation_code: globalisation_code.to_string(),
        parameter_name: parameter_name.to_string(),
        value: rejected_value,
        args,
    };
    FineractError::DataValidation(PlatformApiDataValidationException::new(vec![param_err]))
}

/// Supported Fineract client locales with thousands separator, decimal separator, and localized
/// month name tables for two-pass JSON parsing (`JsonParserHelper.localeFromString`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FineractLocale {
    En,
    EnUs,
    EnGb,
    EnIn,
    De,
    DeDe,
    DeAt,
    DeCh,
    Fr,
    FrFr,
    FrCa,
    Es,
    EsEs,
    EsMx,
    Pt,
    PtBr,
    PtPt,
    It,
    ItIt,
    Nl,
    NlNl,
    Ru,
    RuRu,
    Hi,
    HiIn,
    Ar,
    ArSa,
    Zh,
    ZhCn,
    Ja,
    JaJp,
    Custom {
        language: String,
        country: Option<String>,
        variant: Option<String>,
        thousands_sep: char,
        decimal_sep: char,
    },
}

/// ISO 639-1 two-letter language codes accepted by `Locale.getISOLanguages()`.
const VALID_ISO_LANGUAGES: &[&str] = &[
    "aa", "ab", "ae", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", "ba", "be", "bg",
    "bh", "bi", "bm", "bn", "bo", "br", "bs", "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv",
    "cy", "da", "de", "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu", "fa", "ff", "fi",
    "fj", "fo", "fr", "fy", "ga", "gd", "gl", "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr",
    "ht", "hu", "hy", "hz", "ia", "id", "ie", "ig", "ii", "ik", "in", "io", "is", "it", "iu",
    "iw", "ja", "ji", "jv", "ka", "kg", "ki", "kj", "kk", "kl", "km", "kn", "ko", "kr", "ks",
    "ku", "kv", "kw", "ky", "la", "lb", "lg", "li", "ln", "lo", "lt", "lu", "lv", "mg", "mh",
    "mi", "mk", "ml", "mn", "mo", "mr", "ms", "mt", "my", "na", "nb", "nd", "ne", "ng", "nl",
    "nn", "no", "nr", "nv", "ny", "oc", "oj", "om", "or", "os", "pa", "pi", "pl", "ps", "pt",
    "qu", "rm", "rn", "ro", "ru", "rw", "sa", "sc", "sd", "se", "sg", "si", "sk", "sl", "sm",
    "sn", "so", "sq", "sr", "ss", "st", "su", "sv", "sw", "ta", "te", "tg", "th", "ti", "tk",
    "tl", "tn", "to", "tr", "ts", "tt", "tw", "ty", "ug", "uk", "ur", "uz", "ve", "vi", "vo",
    "wa", "wo", "xh", "yi", "yo", "za", "zh", "zu",
];

impl FineractLocale {
    /// Parses a locale identifier (e.g., `"en"`, `"en_US"`, `"en-US"`, `"de_DE"`, `"fr_FR"`, `"es_ES"`)
    /// matching `JsonParserHelper.localeFromString`.
    pub fn parse(raw: &str) -> Result<Self, FineractError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(validation_error(
                "validation.msg.invalid.locale.format",
                "The parameter `locale` is invalid. It cannot be blank.",
                "locale",
                None,
            ));
        }

        let normalized = trimmed.replace('-', "_");
        let parts: Vec<&str> = normalized.split('_').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(validation_error(
                "validation.msg.invalid.locale.format",
                format!("The parameter `locale` has an invalid format `{trimmed}`."),
                "locale",
                Some(Value::String(trimmed.to_string())),
            ));
        }

        let lang = parts[0].to_ascii_lowercase();
        if !VALID_ISO_LANGUAGES.contains(&lang.as_str()) {
            return Err(validation_error(
                "validation.msg.invalid.locale.format",
                format!("The parameter `locale` has an invalid language value {lang} ."),
                "locale",
                Some(Value::String(lang)),
            ));
        }

        let country = if parts.len() >= 2 && !parts[1].is_empty() {
            let c = parts[1].to_ascii_uppercase();
            if c.len() != 2 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
                return Err(validation_error(
                    "validation.msg.invalid.locale.format",
                    format!("The parameter `locale` has an invalid country value {c} ."),
                    "locale",
                    Some(Value::String(c)),
                ));
            }
            Some(c)
        } else {
            None
        };

        let variant = if parts.len() == 3 && !parts[2].is_empty() {
            Some(parts[2].to_string())
        } else {
            None
        };

        if variant.is_some() {
            let (thousands_sep, decimal_sep) =
                Self::default_separators_for(&lang, country.as_deref());
            return Ok(Self::Custom {
                language: lang,
                country,
                variant,
                thousands_sep,
                decimal_sep,
            });
        }

        match (lang.as_str(), country.as_deref()) {
            ("en", None) => Ok(Self::En),
            ("en", Some("US")) => Ok(Self::EnUs),
            ("en", Some("GB")) => Ok(Self::EnGb),
            ("en", Some("IN")) => Ok(Self::EnIn),
            ("de", None) => Ok(Self::De),
            ("de", Some("DE")) => Ok(Self::DeDe),
            ("de", Some("AT")) => Ok(Self::DeAt),
            ("de", Some("CH")) => Ok(Self::DeCh),
            ("fr", None) => Ok(Self::Fr),
            ("fr", Some("FR")) => Ok(Self::FrFr),
            ("fr", Some("CA")) => Ok(Self::FrCa),
            ("es", None) => Ok(Self::Es),
            ("es", Some("ES")) => Ok(Self::EsEs),
            ("es", Some("MX")) => Ok(Self::EsMx),
            ("pt", None) => Ok(Self::Pt),
            ("pt", Some("BR")) => Ok(Self::PtBr),
            ("pt", Some("PT")) => Ok(Self::PtPt),
            ("it", None) => Ok(Self::It),
            ("it", Some("IT")) => Ok(Self::ItIt),
            ("nl", None) => Ok(Self::Nl),
            ("nl", Some("NL")) => Ok(Self::NlNl),
            ("ru", None) => Ok(Self::Ru),
            ("ru", Some("RU")) => Ok(Self::RuRu),
            ("hi", None) => Ok(Self::Hi),
            ("hi", Some("IN")) => Ok(Self::HiIn),
            ("ar", None) => Ok(Self::Ar),
            ("ar", Some("SA")) => Ok(Self::ArSa),
            ("zh", None) => Ok(Self::Zh),
            ("zh", Some("CN")) => Ok(Self::ZhCn),
            ("ja", None) => Ok(Self::Ja),
            ("ja", Some("JP")) => Ok(Self::JaJp),
            _ => {
                let (thousands_sep, decimal_sep) =
                    Self::default_separators_for(&lang, country.as_deref());
                Ok(Self::Custom {
                    language: lang,
                    country,
                    variant: None,
                    thousands_sep,
                    decimal_sep,
                })
            }
        }
    }

    fn default_separators_for(lang: &str, country: Option<&str>) -> (char, char) {
        match (lang, country) {
            ("fr" | "ru" | "pl" | "cs" | "sk" | "sv" | "fi", _) => (' ', ','),
            ("de", Some("CH")) => ('\'', '.'),
            ("es", Some("MX" | "US" | "GT" | "HN" | "NI" | "PA" | "PR" | "SV" | "DO")) => (',', '.'),
            (
                "de" | "es" | "pt" | "it" | "nl" | "da" | "el" | "tr" | "ro" | "hu" | "hr"
                | "sl" | "sr" | "bg" | "id" | "vi",
                _,
            ) => ('.', ','),
            _ => (',', '.'),
        }
    }

    /// Returns the primary thousands (grouping) separator for this locale.
    pub const fn thousands_separator(&self) -> char {
        match self {
            Self::En
            | Self::EnUs
            | Self::EnGb
            | Self::EnIn
            | Self::EsMx
            | Self::Hi
            | Self::HiIn
            | Self::Ar
            | Self::ArSa
            | Self::Zh
            | Self::ZhCn
            | Self::Ja
            | Self::JaJp => ',',
            Self::De
            | Self::DeDe
            | Self::DeAt
            | Self::Es
            | Self::EsEs
            | Self::Pt
            | Self::PtBr
            | Self::PtPt
            | Self::It
            | Self::ItIt
            | Self::Nl
            | Self::NlNl => '.',
            Self::DeCh => '\'',
            Self::Fr | Self::FrFr | Self::FrCa | Self::Ru | Self::RuRu => ' ',
            Self::Custom { thousands_sep, .. } => *thousands_sep,
        }
    }

    /// Returns the decimal separator for this locale (`.` vs `,`).
    pub const fn decimal_separator(&self) -> char {
        match self {
            Self::En
            | Self::EnUs
            | Self::EnGb
            | Self::EnIn
            | Self::DeCh
            | Self::EsMx
            | Self::Hi
            | Self::HiIn
            | Self::Ar
            | Self::ArSa
            | Self::Zh
            | Self::ZhCn
            | Self::Ja
            | Self::JaJp => '.',
            Self::De
            | Self::DeDe
            | Self::DeAt
            | Self::Fr
            | Self::FrFr
            | Self::FrCa
            | Self::Es
            | Self::EsEs
            | Self::Pt
            | Self::PtBr
            | Self::PtPt
            | Self::It
            | Self::ItIt
            | Self::Nl
            | Self::NlNl
            | Self::Ru
            | Self::RuRu => ',',
            Self::Custom { decimal_sep, .. } => *decimal_sep,
        }
    }

    /// Checks whether `c` is a valid grouping separator for this locale (handling ASCII space,
    /// non-breaking space `\u{00a0}`, and narrow non-breaking space `\u{202f}` for French/space-grouped locales).
    pub fn is_grouping_separator(&self, c: char) -> bool {
        let primary = self.thousands_separator();
        if primary == ' ' {
            c == ' ' || c == '\u{00a0}' || c == '\u{202f}'
        } else if primary == '\'' {
            c == '\'' || c == '\u{2019}'
        } else {
            c == primary
        }
    }

    /// Returns the two-letter lowercase ISO language code (e.g., `"en"`, `"de"`, `"fr"`, `"es"`).
    pub fn language(&self) -> &str {
        match self {
            Self::En | Self::EnUs | Self::EnGb | Self::EnIn => "en",
            Self::De | Self::DeDe | Self::DeAt | Self::DeCh => "de",
            Self::Fr | Self::FrFr | Self::FrCa => "fr",
            Self::Es | Self::EsEs | Self::EsMx => "es",
            Self::Pt | Self::PtBr | Self::PtPt => "pt",
            Self::It | Self::ItIt => "it",
            Self::Nl | Self::NlNl => "nl",
            Self::Ru | Self::RuRu => "ru",
            Self::Hi | Self::HiIn => "hi",
            Self::Ar | Self::ArSa => "ar",
            Self::Zh | Self::ZhCn => "zh",
            Self::Ja | Self::JaJp => "ja",
            Self::Custom { language, .. } => language.as_str(),
        }
    }

    /// Parses a full (`MMMM`) or abbreviated (`MMM`) month name into a month number (`1..=12`).
    pub fn parse_month(&self, raw_month: &str) -> Option<u32> {
        let cleaned = raw_month.trim().trim_end_matches('.').to_lowercase();
        if cleaned.is_empty() {
            return None;
        }
        Self::parse_month_for_lang(self.language(), &cleaned)
            .or_else(|| Self::parse_month_for_lang("en", &cleaned))
    }

    fn parse_month_for_lang(lang: &str, lower: &str) -> Option<u32> {
        match lang {
            "en" => match lower {
                "january" | "jan" => Some(1),
                "february" | "feb" => Some(2),
                "march" | "mar" => Some(3),
                "april" | "apr" => Some(4),
                "may" => Some(5),
                "june" | "jun" => Some(6),
                "july" | "jul" => Some(7),
                "august" | "aug" => Some(8),
                "september" | "sep" | "sept" => Some(9),
                "october" | "oct" => Some(10),
                "november" | "nov" => Some(11),
                "december" | "dec" => Some(12),
                _ => None,
            },
            "de" => match lower {
                "januar" | "jänner" | "jaenner" | "jan" | "jän" => Some(1),
                "februar" | "feb" => Some(2),
                "märz" | "maerz" | "mär" | "maer" | "mrz" => Some(3),
                "april" | "apr" => Some(4),
                "mai" => Some(5),
                "juni" | "jun" => Some(6),
                "juli" | "jul" => Some(7),
                "august" | "aug" => Some(8),
                "september" | "sep" | "sept" => Some(9),
                "oktober" | "okt" => Some(10),
                "november" | "nov" => Some(11),
                "dezember" | "dez" => Some(12),
                _ => None,
            },
            "fr" => match lower {
                "janvier" | "janv" | "jan" => Some(1),
                "février" | "fevrier" | "févr" | "fevr" | "fév" | "fev" => Some(2),
                "mars" | "mar" => Some(3),
                "avril" | "avr" => Some(4),
                "mai" => Some(5),
                "juin" | "jun" => Some(6),
                "juillet" | "juil" | "jul" => Some(7),
                "août" | "aout" | "aoû" | "aou" => Some(8),
                "septembre" | "sept" | "sep" => Some(9),
                "octobre" | "oct" => Some(10),
                "novembre" | "nov" => Some(11),
                "décembre" | "decembre" | "déc" | "dec" => Some(12),
                _ => None,
            },
            "es" => match lower {
                "enero" | "ene" => Some(1),
                "febrero" | "feb" => Some(2),
                "marzo" | "mar" => Some(3),
                "abril" | "abr" => Some(4),
                "mayo" | "may" => Some(5),
                "junio" | "jun" => Some(6),
                "julio" | "jul" => Some(7),
                "agosto" | "ago" => Some(8),
                "septiembre" | "setiembre" | "sep" | "sept" | "set" => Some(9),
                "octubre" | "oct" => Some(10),
                "noviembre" | "nov" => Some(11),
                "diciembre" | "dic" => Some(12),
                _ => None,
            },
            "pt" => match lower {
                "janeiro" | "jan" => Some(1),
                "fevereiro" | "fev" => Some(2),
                "março" | "marco" | "mar" => Some(3),
                "abril" | "abr" => Some(4),
                "maio" | "mai" => Some(5),
                "junho" | "jun" => Some(6),
                "julho" | "jul" => Some(7),
                "agosto" | "ago" => Some(8),
                "setembro" | "set" => Some(9),
                "outubro" | "out" => Some(10),
                "novembro" | "nov" => Some(11),
                "dezembro" | "dez" => Some(12),
                _ => None,
            },
            "it" => match lower {
                "gennaio" | "gen" => Some(1),
                "febbraio" | "feb" => Some(2),
                "marzo" | "mar" => Some(3),
                "aprile" | "apr" => Some(4),
                "maggio" | "mag" => Some(5),
                "giugno" | "giu" => Some(6),
                "luglio" | "lug" => Some(7),
                "agosto" | "ago" => Some(8),
                "settembre" | "set" => Some(9),
                "ottobre" | "ott" => Some(10),
                "novembre" | "nov" => Some(11),
                "dicembre" | "dic" => Some(12),
                _ => None,
            },
            _ => None,
        }
    }
}

impl fmt::Display for FineractLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::En => f.write_str("en"),
            Self::EnUs => f.write_str("en_US"),
            Self::EnGb => f.write_str("en_GB"),
            Self::EnIn => f.write_str("en_IN"),
            Self::De => f.write_str("de"),
            Self::DeDe => f.write_str("de_DE"),
            Self::DeAt => f.write_str("de_AT"),
            Self::DeCh => f.write_str("de_CH"),
            Self::Fr => f.write_str("fr"),
            Self::FrFr => f.write_str("fr_FR"),
            Self::FrCa => f.write_str("fr_CA"),
            Self::Es => f.write_str("es"),
            Self::EsEs => f.write_str("es_ES"),
            Self::EsMx => f.write_str("es_MX"),
            Self::Pt => f.write_str("pt"),
            Self::PtBr => f.write_str("pt_BR"),
            Self::PtPt => f.write_str("pt_PT"),
            Self::It => f.write_str("it"),
            Self::ItIt => f.write_str("it_IT"),
            Self::Nl => f.write_str("nl"),
            Self::NlNl => f.write_str("nl_NL"),
            Self::Ru => f.write_str("ru"),
            Self::RuRu => f.write_str("ru_RU"),
            Self::Hi => f.write_str("hi"),
            Self::HiIn => f.write_str("hi_IN"),
            Self::Ar => f.write_str("ar"),
            Self::ArSa => f.write_str("ar_SA"),
            Self::Zh => f.write_str("zh"),
            Self::ZhCn => f.write_str("zh_CN"),
            Self::Ja => f.write_str("ja"),
            Self::JaJp => f.write_str("ja_JP"),
            Self::Custom {
                language,
                country,
                variant,
                ..
            } => {
                write!(f, "{language}")?;
                if let Some(c) = country {
                    write!(f, "_{c}")?;
                }
                if let Some(v) = variant {
                    write!(f, "_{v}")?;
                }
                Ok(())
            }
        }
    }
}

impl FromStr for FineractLocale {
    type Err = FineractError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DatePatternToken {
    Year4,
    Year2,
    MonthFull,
    MonthShort,
    MonthNum2,
    MonthNum1,
    Day2,
    Day1,
    Hour,
    Minute,
    Second,
    Literal(String),
}

/// Locale-aware and Java `DateTimeFormatter`-compatible date parser ported from `JsonParserHelper.convertFrom`.
pub struct FineractDateParser;

impl FineractDateParser {
    /// Parses a date string using a Java `DateTimeFormatter` pattern (such as `"dd MMMM yyyy"`,
    /// `"dd MMM yyyy"`, `"yyyy-MM-dd"`, `"dd/MM/yyyy"`, `"MM/dd/yyyy"`) and a Fineract locale string.
    pub fn parse_date(
        raw: &str,
        date_format: &str,
        locale: &str,
    ) -> Result<NaiveDate, FineractError> {
        Self::parse_date_for_param("date", raw, date_format, locale)
    }

    /// Parses a date string for a named parameter, preserving Fineract's parameter-scoped validation error codes.
    pub fn parse_date_for_param(
        param_name: &str,
        raw: &str,
        date_format: &str,
        locale: &str,
    ) -> Result<NaiveDate, FineractError> {
        let df_trimmed = date_format.trim();
        if df_trimmed.is_empty() {
            return Err(validation_error(
                "validation.msg.missing.dateFormat.parameter",
                format!(
                    "The parameter `{param_name}` requires a `dateFormat` parameter to be passed with it."
                ),
                param_name,
                None,
            ));
        }

        let loc_trimmed = locale.trim();
        if loc_trimmed.is_empty() {
            return Err(validation_error(
                "validation.msg.missing.locale.parameter",
                format!(
                    "The parameter `{param_name}` requires a `locale` parameter to be passed with it."
                ),
                param_name,
                None,
            ));
        }

        let parsed_locale = FineractLocale::parse(loc_trimmed)?;
        let tokens = Self::compile_pattern(df_trimmed, param_name)?;

        let input = raw.trim();
        if input.is_empty() {
            return Err(Self::invalid_date_error(
                param_name,
                raw,
                df_trimmed,
                &parsed_locale,
            ));
        }

        Self::execute_tokens(&tokens, input, &parsed_locale).ok_or_else(|| {
            Self::invalid_date_error(param_name, raw, df_trimmed, &parsed_locale)
        })
    }

    /// Parses a `[year, month, day]` JSON array matching `JsonParserHelper.extractLocalDateAsArrayNamed`.
    pub fn parse_date_array(
        arr: &[Value],
        param_name: &str,
    ) -> Result<NaiveDate, FineractError> {
        if arr.len() < 3 {
            return Err(validation_error(
                "validation.msg.invalid.dateFormat.format",
                format!("The parameter `{param_name}` array must have [year, month, day]."),
                param_name,
                Some(Value::Array(arr.to_vec())),
            ));
        }
        let year = arr[0].as_i64().and_then(|v| i32::try_from(v).ok());
        let month = arr[1].as_u64().and_then(|v| u32::try_from(v).ok());
        let day = arr[2].as_u64().and_then(|v| u32::try_from(v).ok());

        match (year, month, day) {
            (Some(y), Some(m), Some(d)) => NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| {
                validation_error(
                    "validation.msg.invalid.dateFormat.format",
                    format!("The parameter `{param_name}` contains an invalid calendar date."),
                    param_name,
                    Some(Value::Array(arr.to_vec())),
                )
            }),
            _ => Err(validation_error(
                "validation.msg.invalid.dateFormat.format",
                format!("The parameter `{param_name}` array elements must be integers."),
                param_name,
                Some(Value::Array(arr.to_vec())),
            )),
        }
    }

    fn invalid_date_error(
        param_name: &str,
        raw: &str,
        date_format: &str,
        locale: &FineractLocale,
    ) -> FineractError {
        validation_error(
            "validation.msg.invalid.dateFormat.format",
            format!(
                "The parameter `{param_name}` is invalid based on the dateFormat: `{date_format}` and locale: `{locale}` provided:"
            ),
            param_name,
            Some(Value::String(raw.to_string())),
        )
    }

    fn compile_pattern(
        pattern: &str,
        param_name: &str,
    ) -> Result<Vec<DatePatternToken>, FineractError> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            if c == '\'' {
                i += 1;
                if i < chars.len() && chars[i] == '\'' {
                    tokens.push(DatePatternToken::Literal("'".to_string()));
                    i += 1;
                    continue;
                }
                let mut lit = String::new();
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == '\'' {
                        i += 1;
                        if i < chars.len() && chars[i] == '\'' {
                            lit.push('\'');
                            i += 1;
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        lit.push(chars[i]);
                        i += 1;
                    }
                }
                if !closed {
                    return Err(validation_error(
                        "validation.msg.invalid.dateFormat.format",
                        format!("Invalid dateFormat: `{pattern}`"),
                        param_name,
                        Some(Value::String(pattern.to_string())),
                    ));
                }
                if !lit.is_empty() {
                    tokens.push(DatePatternToken::Literal(lit));
                }
            } else if c.is_ascii_alphabetic() {
                let start = i;
                while i < chars.len() && chars[i] == c {
                    i += 1;
                }
                let count = i - start;
                let token = match (c, count) {
                    ('y' | 'u' | 'Y', 4) => DatePatternToken::Year4,
                    ('y' | 'u' | 'Y', 2) => DatePatternToken::Year2,
                    ('M' | 'L', 4) => DatePatternToken::MonthFull,
                    ('M' | 'L', 3) => DatePatternToken::MonthShort,
                    ('M' | 'L', 2) => DatePatternToken::MonthNum2,
                    ('M' | 'L', 1) => DatePatternToken::MonthNum1,
                    ('d', 2) => DatePatternToken::Day2,
                    ('d', 1) => DatePatternToken::Day1,
                    ('H' | 'h', 1 | 2) => DatePatternToken::Hour,
                    ('m', 1 | 2) => DatePatternToken::Minute,
                    ('s', 1 | 2) => DatePatternToken::Second,
                    _ => {
                        return Err(validation_error(
                            "validation.msg.invalid.dateFormat.format",
                            format!("Invalid dateFormat: `{pattern}`"),
                            param_name,
                            Some(Value::String(pattern.to_string())),
                        ));
                    }
                };
                tokens.push(token);
            } else if c.is_ascii_digit() {
                // Unquoted digits in a pattern (e.g. "02 February 2026" passed as a pattern) are invalid
                return Err(validation_error(
                    "validation.msg.invalid.dateFormat.format",
                    format!("Invalid dateFormat: `{pattern}`"),
                    param_name,
                    Some(Value::String(pattern.to_string())),
                ));
            } else {
                let mut lit = String::new();
                while i < chars.len()
                    && !chars[i].is_ascii_alphabetic()
                    && !chars[i].is_ascii_digit()
                    && chars[i] != '\''
                {
                    lit.push(chars[i]);
                    i += 1;
                }
                tokens.push(DatePatternToken::Literal(lit));
            }
        }

        let has_year = tokens
            .iter()
            .any(|t| matches!(t, DatePatternToken::Year4 | DatePatternToken::Year2));
        let has_month = tokens.iter().any(|t| {
            matches!(
                t,
                DatePatternToken::MonthFull
                    | DatePatternToken::MonthShort
                    | DatePatternToken::MonthNum2
                    | DatePatternToken::MonthNum1
            )
        });
        let has_day = tokens
            .iter()
            .any(|t| matches!(t, DatePatternToken::Day2 | DatePatternToken::Day1));

        if !has_year || !has_month || !has_day {
            return Err(validation_error(
                "validation.msg.invalid.dateFormat.format",
                format!("Invalid dateFormat: `{pattern}`"),
                param_name,
                Some(Value::String(pattern.to_string())),
            ));
        }

        Ok(tokens)
    }

    fn execute_tokens(
        tokens: &[DatePatternToken],
        input: &str,
        locale: &FineractLocale,
    ) -> Option<NaiveDate> {
        let mut rest = input;
        let mut year: Option<i32> = None;
        let mut month: Option<u32> = None;
        let mut day: Option<u32> = None;

        for (idx, token) in tokens.iter().enumerate() {
            let next_is_numeric = tokens.get(idx + 1).is_some_and(|next| {
                matches!(
                    next,
                    DatePatternToken::Year4
                        | DatePatternToken::Year2
                        | DatePatternToken::MonthNum2
                        | DatePatternToken::MonthNum1
                        | DatePatternToken::Day2
                        | DatePatternToken::Day1
                        | DatePatternToken::Hour
                        | DatePatternToken::Minute
                        | DatePatternToken::Second
                )
            });

            match token {
                DatePatternToken::Literal(lit) => {
                    if lit.chars().all(|c| c.is_whitespace()) {
                        let trimmed = rest.trim_start();
                        if trimmed.len() == rest.len() {
                            return None;
                        }
                        rest = trimmed;
                    } else {
                        let lit_trimmed = lit.trim();
                        rest = rest.trim_start();
                        if !lit_trimmed.is_empty() {
                            if rest.len() < lit_trimmed.len()
                                || !rest[..lit_trimmed.len()].eq_ignore_ascii_case(lit_trimmed)
                            {
                                return None;
                            }
                            rest = &rest[lit_trimmed.len()..];
                        }
                        if lit.ends_with(char::is_whitespace) {
                            rest = rest.trim_start();
                        }
                    }
                }
                DatePatternToken::Year4 => {
                    let (digits, rem) = Self::consume_digits(rest, 4, 4)?;
                    year = Some(digits.parse::<i32>().ok()?);
                    rest = rem;
                }
                DatePatternToken::Year2 => {
                    let (digits, rem) = Self::consume_digits(rest, 2, 2)?;
                    let y2 = digits.parse::<i32>().ok()?;
                    year = Some(if y2 >= 70 { 1900 + y2 } else { 2000 + y2 });
                    rest = rem;
                }
                DatePatternToken::MonthNum2 => {
                    let min_len = if next_is_numeric { 2 } else { 1 };
                    let (digits, rem) = Self::consume_digits(rest, min_len, 2)?;
                    month = Some(digits.parse::<u32>().ok()?);
                    rest = rem;
                }
                DatePatternToken::MonthNum1 => {
                    let (digits, rem) = Self::consume_digits(rest, 1, 2)?;
                    month = Some(digits.parse::<u32>().ok()?);
                    rest = rem;
                }
                DatePatternToken::MonthFull | DatePatternToken::MonthShort => {
                    let (word, rem) = Self::consume_month_word(rest)?;
                    month = Some(locale.parse_month(word)?);
                    rest = rem;
                }
                DatePatternToken::Day2 => {
                    let min_len = if next_is_numeric { 2 } else { 1 };
                    let (digits, rem) = Self::consume_digits(rest, min_len, 2)?;
                    day = Some(digits.parse::<u32>().ok()?);
                    rest = rem;
                }
                DatePatternToken::Day1 => {
                    let (digits, rem) = Self::consume_digits(rest, 1, 2)?;
                    day = Some(digits.parse::<u32>().ok()?);
                    rest = rem;
                }
                DatePatternToken::Hour => {
                    let (digits, rem) = Self::consume_digits(rest, 1, 2)?;
                    let h = digits.parse::<u32>().ok()?;
                    if h > 23 {
                        return None;
                    }
                    rest = rem;
                }
                DatePatternToken::Minute | DatePatternToken::Second => {
                    let (digits, rem) = Self::consume_digits(rest, 1, 2)?;
                    let m = digits.parse::<u32>().ok()?;
                    if m > 59 {
                        return None;
                    }
                    rest = rem;
                }
            }
        }

        rest = rest.trim_start();
        // Match JsonParserHelper.convertDateTimeFrom: optional trailing "HH:mm:ss"
        if !rest.is_empty() {
            if !Self::consume_optional_time_suffix(rest) {
                return None;
            }
        }

        let y = year?;
        let m = month?;
        let d = day?;
        NaiveDate::from_ymd_opt(y, m, d)
    }

    fn consume_digits(s: &str, min_digits: usize, max_digits: usize) -> Option<(&str, &str)> {
        let count = s
            .chars()
            .take(max_digits)
            .take_while(|c| c.is_ascii_digit())
            .count();
        if count < min_digits {
            return None;
        }
        Some((&s[..count], &s[count..]))
    }

    fn consume_month_word(s: &str) -> Option<(&str, &str)> {
        let mut end = 0;
        for (idx, ch) in s.char_indices() {
            if ch.is_alphabetic() || ch == '.' {
                end = idx + ch.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            None
        } else {
            Some((&s[..end], &s[end..]))
        }
    }

    fn consume_optional_time_suffix(s: &str) -> bool {
        let parts: Vec<&str> = s.trim().split(':').collect();
        if parts.len() != 3 {
            return false;
        }
        let Ok(h) = parts[0].parse::<u32>() else {
            return false;
        };
        let Ok(m) = parts[1].parse::<u32>() else {
            return false;
        };
        let Ok(sec) = parts[2].parse::<u32>() else {
            return false;
        };
        h <= 23 && m <= 59 && sec <= 59
    }
}

/// Locale-aware number parser ported from `JsonParserHelper.convertFrom` and `JsonParserHelper.convertToInteger`.
pub struct FineractNumberParser;

impl FineractNumberParser {
    /// Parses a JSON value (number or locale-formatted string like `"10.000,50"` in `de_DE` or
    /// `"10,000.50"` in `en_US` or `"10 000,50"` in `fr_FR`) into `Option<Decimal>`.
    pub fn parse_decimal(raw: &Value, locale: &str) -> Result<Option<Decimal>, FineractError> {
        Self::parse_decimal_for_param("amount", raw, locale)
    }

    /// Parses a JSON value into `Option<Decimal>` for a specific parameter name.
    pub fn parse_decimal_for_param(
        param_name: &str,
        raw: &Value,
        locale: &str,
    ) -> Result<Option<Decimal>, FineractError> {
        match raw {
            Value::Null => Ok(None),
            Value::Number(num) => {
                let s = num.to_string();
                let dec = Decimal::from_str_exact(&s)
                    .or_else(|_| Decimal::from_scientific(&s))
                    .map_err(|_| Self::invalid_decimal_error(param_name, &s, locale))?;
                Ok(Some(dec))
            }
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                let loc_trimmed = locale.trim();
                if loc_trimmed.is_empty() {
                    return Err(Self::missing_locale_error(param_name));
                }
                let parsed_locale = FineractLocale::parse(loc_trimmed)?;
                let normalized = Self::normalize_decimal_str(trimmed, &parsed_locale)
                    .ok_or_else(|| {
                        Self::invalid_decimal_error(
                            param_name,
                            trimmed,
                            &parsed_locale.to_string(),
                        )
                    })?;
                let dec = Decimal::from_str_exact(&normalized)
                    .or_else(|_| Decimal::from_scientific(&normalized))
                    .map_err(|_| {
                        Self::invalid_decimal_error(
                            param_name,
                            trimmed,
                            &parsed_locale.to_string(),
                        )
                    })?;
                Ok(Some(dec))
            }
            other => Err(Self::invalid_decimal_error(
                param_name,
                &other.to_string(),
                locale,
            )),
        }
    }

    /// Parses a JSON value (number or locale-formatted integer string) into `Option<i64>`.
    pub fn parse_i64(raw: &Value, locale: &str) -> Result<Option<i64>, FineractError> {
        Self::parse_i64_for_param("id", raw, locale)
    }

    /// Parses a JSON value into `Option<i64>` for a specific parameter name.
    pub fn parse_i64_for_param(
        param_name: &str,
        raw: &Value,
        locale: &str,
    ) -> Result<Option<i64>, FineractError> {
        match raw {
            Value::Null => Ok(None),
            Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    Ok(Some(i))
                } else {
                    Err(Self::invalid_integer_error(
                        param_name,
                        &num.to_string(),
                        locale,
                    ))
                }
            }
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                let loc_trimmed = locale.trim();
                if loc_trimmed.is_empty() {
                    // Notice: JsonParserHelper.extractLongNamed allows unformatted Longs without locale,
                    // while locale-aware integer parsing strips grouping separators when locale is present.
                    if let Ok(val) = trimmed.parse::<i64>() {
                        return Ok(Some(val));
                    }
                    return Err(Self::missing_locale_error(param_name));
                }
                let parsed_locale = FineractLocale::parse(loc_trimmed)?;
                let normalized = Self::normalize_integer_str(trimmed, &parsed_locale)
                    .ok_or_else(|| {
                        Self::invalid_integer_error(
                            param_name,
                            trimmed,
                            &parsed_locale.to_string(),
                        )
                    })?;
                let val = normalized.parse::<i64>().map_err(|_| {
                    Self::invalid_integer_error(param_name, trimmed, &parsed_locale.to_string())
                })?;
                Ok(Some(val))
            }
            other => Err(Self::invalid_integer_error(
                param_name,
                &other.to_string(),
                locale,
            )),
        }
    }

    /// Parses a JSON value (number or locale-formatted integer string) into `Option<i32>`
    /// matching `JsonParserHelper.convertToInteger`.
    pub fn parse_i32(raw: &Value, locale: &str) -> Result<Option<i32>, FineractError> {
        Self::parse_i32_for_param("integer", raw, locale)
    }

    /// Parses a JSON value into `Option<i32>` for a specific parameter name.
    pub fn parse_i32_for_param(
        param_name: &str,
        raw: &Value,
        locale: &str,
    ) -> Result<Option<i32>, FineractError> {
        match raw {
            Value::Null => Ok(None),
            Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    let v = i32::try_from(i).map_err(|_| {
                        Self::invalid_integer_error(param_name, &num.to_string(), locale)
                    })?;
                    Ok(Some(v))
                } else {
                    Err(Self::invalid_integer_error(
                        param_name,
                        &num.to_string(),
                        locale,
                    ))
                }
            }
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                let loc_trimmed = locale.trim();
                if loc_trimmed.is_empty() {
                    return Err(Self::missing_locale_error(param_name));
                }
                let parsed_locale = FineractLocale::parse(loc_trimmed)?;
                let normalized = Self::normalize_integer_str(trimmed, &parsed_locale)
                    .ok_or_else(|| {
                        Self::invalid_integer_error(
                            param_name,
                            trimmed,
                            &parsed_locale.to_string(),
                        )
                    })?;
                let val = normalized.parse::<i32>().map_err(|_| {
                    Self::invalid_integer_error(param_name, trimmed, &parsed_locale.to_string())
                })?;
                Ok(Some(val))
            }
            other => Err(Self::invalid_integer_error(
                param_name,
                &other.to_string(),
                locale,
            )),
        }
    }

    /// Parses an unformatted integer without locale (`JsonParserHelper.convertToIntegerSanLocale`).
    pub fn parse_i32_sans_locale(
        raw: &Value,
        param_name: &str,
    ) -> Result<Option<i32>, FineractError> {
        match raw {
            Value::Null => Ok(None),
            Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    if let Ok(v) = i32::try_from(i) {
                        return Ok(Some(v));
                    }
                }
                Err(validation_error(
                    "validation.msg.invalid.integer",
                    format!(
                        "The parameter `{param_name}` has value: {num} which is invalid integer."
                    ),
                    param_name,
                    Some(Value::String(num.to_string())),
                ))
            }
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                trimmed.parse::<i32>().map(Some).map_err(|_| {
                    validation_error(
                        "validation.msg.invalid.integer",
                        format!(
                            "The parameter `{param_name}` has value: {trimmed} which is invalid integer."
                        ),
                        param_name,
                        Some(Value::String(trimmed.to_string())),
                    )
                })
            }
            other => Err(validation_error(
                "validation.msg.invalid.integer",
                format!(
                    "The parameter `{param_name}` has value: {other} which is invalid integer."
                ),
                param_name,
                Some(other.clone()),
            )),
        }
    }

    fn normalize_decimal_str(source: &str, locale: &FineractLocale) -> Option<String> {
        let dec_sep = locale.decimal_separator();
        let mut out = String::with_capacity(source.len());
        let mut seen_decimal = false;
        let mut seen_digit = false;

        for (idx, ch) in source.chars().enumerate() {
            if idx == 0 && (ch == '+' || ch == '-') {
                out.push(ch);
            } else if ch.is_ascii_digit() {
                seen_digit = true;
                out.push(ch);
            } else if ch == dec_sep {
                if seen_decimal {
                    return None;
                }
                seen_decimal = true;
                out.push('.');
            } else if locale.is_grouping_separator(ch) {
                // Grouping separators cannot appear before the first digit or after the decimal separator
                if !seen_digit || seen_decimal {
                    return None;
                }
            } else {
                return None;
            }
        }

        if !seen_digit {
            return None;
        }
        Some(out)
    }

    fn normalize_integer_str(source: &str, locale: &FineractLocale) -> Option<String> {
        let dec_sep = locale.decimal_separator();
        if source.contains(dec_sep) {
            return None;
        }
        let mut out = String::with_capacity(source.len());
        let mut seen_digit = false;

        for (idx, ch) in source.chars().enumerate() {
            if idx == 0 && (ch == '+' || ch == '-') {
                out.push(ch);
            } else if ch.is_ascii_digit() {
                seen_digit = true;
                out.push(ch);
            } else if locale.is_grouping_separator(ch) {
                if !seen_digit {
                    return None;
                }
            } else {
                return None;
            }
        }

        if !seen_digit {
            return None;
        }
        Some(out)
    }

    fn missing_locale_error(param_name: &str) -> FineractError {
        validation_error(
            "validation.msg.missing.locale.parameter",
            format!(
                "The parameter `{param_name}` requires a `locale` parameter to be passed with it."
            ),
            param_name,
            None,
        )
    }

    fn invalid_decimal_error(param_name: &str, val: &str, locale: &str) -> FineractError {
        validation_error(
            "validation.msg.invalid.decimal.format",
            format!(
                "The parameter `{param_name}` has value: {val} which is invalid decimal value for provided locale of [{locale}]."
            ),
            param_name,
            Some(Value::String(val.to_string())),
        )
    }

    fn invalid_integer_error(param_name: &str, val: &str, locale: &str) -> FineractError {
        validation_error(
            "validation.msg.invalid.integer.format",
            format!(
                "The parameter `{param_name}` has value: {val} which is invalid integer value for provided locale of [{locale}]."
            ),
            param_name,
            Some(Value::String(val.to_string())),
        )
    }
}

/// Immutable representation of a Fineract write command wrapping raw and parsed JSON with
/// two-pass locale/dateFormat extraction and change-detection methods (`JsonCommand.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonCommand {
    pub command_id: Option<i64>,
    pub raw_json: String,
    pub parsed: Value,
    pub entity_id: Option<i64>,
    pub subresource_id: Option<i64>,
    pub client_id: Option<i64>,
    pub loan_id: Option<i64>,
    pub savings_id: Option<i64>,
    pub resource_external_id: Option<ExternalId>,
}

impl JsonCommand {
    /// Creates a [`JsonCommand`] from raw JSON and an optional entity ID without failing on malformed JSON.
    pub fn from_raw(raw_json: impl Into<String>, entity_id: Option<i64>) -> Self {
        let raw_json = raw_json.into();
        let parsed = serde_json::from_str(&raw_json).unwrap_or(Value::Null);
        Self {
            command_id: None,
            raw_json,
            parsed,
            entity_id,
            subresource_id: None,
            client_id: None,
            loan_id: None,
            savings_id: None,
            resource_external_id: None,
        }
    }

    /// Creates a new [`JsonCommand`] with all resource identifiers explicitly populated.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_id: Option<i64>,
        raw_json: impl Into<String>,
        parsed: Value,
        entity_id: Option<i64>,
        subresource_id: Option<i64>,
        client_id: Option<i64>,
        loan_id: Option<i64>,
        savings_id: Option<i64>,
    ) -> Self {
        Self {
            command_id,
            raw_json: raw_json.into(),
            parsed,
            entity_id,
            subresource_id,
            client_id,
            loan_id,
            savings_id,
            resource_external_id: None,
        }
    }

    /// Parses a raw JSON payload into a [`JsonCommand`], returning a validation error if the JSON is blank or malformed.
    pub fn from_json(raw_json: impl Into<String>) -> Result<Self, FineractError> {
        let raw_json = raw_json.into();
        if raw_json.trim().is_empty() {
            return Err(validation_error(
                "error.msg.invalid.json.data",
                "The request JSON body cannot be blank.",
                "json",
                None,
            ));
        }
        let parsed: Value = serde_json::from_str(&raw_json).map_err(|e| {
            validation_error(
                "error.msg.invalid.json.data",
                format!("Malformed JSON payload: {e}"),
                "json",
                None,
            )
        })?;
        Ok(Self {
            command_id: None,
            raw_json,
            parsed,
            entity_id: None,
            subresource_id: None,
            client_id: None,
            loan_id: None,
            savings_id: None,
            resource_external_id: None,
        })
    }

    /// Constructs a [`JsonCommand`] from a pre-parsed [`serde_json::Value`] and optional `entity_id`.
    pub fn from_value(entity_id: Option<i64>, parsed: Value) -> Self {
        let raw_json = parsed.to_string();
        Self {
            command_id: None,
            raw_json,
            parsed,
            entity_id,
            subresource_id: None,
            client_id: None,
            loan_id: None,
            savings_id: None,
            resource_external_id: None,
        }
    }

    pub fn with_command_id(mut self, command_id: i64) -> Self {
        self.command_id = Some(command_id);
        self
    }

    pub fn with_entity_id(mut self, entity_id: i64) -> Self {
        self.entity_id = Some(entity_id);
        self
    }

    pub fn with_subresource_id(mut self, subresource_id: i64) -> Self {
        self.subresource_id = Some(subresource_id);
        self
    }

    pub fn with_client_id(mut self, client_id: i64) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub fn with_loan_id(mut self, loan_id: i64) -> Self {
        self.loan_id = Some(loan_id);
        self
    }

    pub fn with_savings_id(mut self, savings_id: i64) -> Self {
        self.savings_id = Some(savings_id);
        self
    }

    pub fn entity_id(&self) -> Option<i64> {
        self.entity_id
    }

    pub fn command_id(&self) -> Option<i64> {
        self.command_id
    }

    /// Returns the raw JSON string (`JsonCommand.json()`).
    pub fn json(&self) -> &str {
        &self.raw_json
    }

    /// Returns a reference to the parsed JSON root value (`JsonCommand.parsedJson()`).
    pub fn parsed_json(&self) -> &Value {
        &self.parsed
    }

    /// Returns the JSON element for `param` if present (`JsonCommand.jsonElement(paramName)`).
    pub fn json_element(&self, param: &str) -> Option<&Value> {
        self.parsed.as_object()?.get(param)
    }

    /// Serializes a nested JSON attribute back into a string (`JsonCommand.jsonFragment(paramName)`).
    pub fn json_fragment(&self, param: &str) -> Option<String> {
        self.json_element(param).map(ToString::to_string)
    }

    /// Returns the `"locale"` parameter from the JSON payload (empty string if absent, matching `JsonCommand.locale()`).
    pub fn locale(&self) -> String {
        self.string_value_of_parameter_named("locale")
            .unwrap_or_default()
    }

    /// Returns the `"dateFormat"` parameter from the JSON payload (empty string if absent, matching `JsonCommand.dateFormat()`).
    pub fn date_format(&self) -> String {
        self.string_value_of_parameter_named("dateFormat")
            .unwrap_or_default()
    }

    /// Checks whether `param` exists as a key on the root JSON object (`JsonCommand.parameterExists`).
    pub fn parameter_exists(&self, param: &str) -> bool {
        self.parsed
            .as_object()
            .is_some_and(|obj| obj.contains_key(param))
    }

    /// Alias for [`JsonCommand::parameter_exists`] (`JsonCommand.hasParameter`).
    pub fn has_parameter(&self, param: &str) -> bool {
        self.parameter_exists(param)
    }

    /// Checks whether `param` exists and is not JSON `null` (`JsonCommand.hasNonNullParameter`).
    pub fn has_non_null_parameter(&self, param: &str) -> bool {
        self.json_element(param).is_some_and(|v| !v.is_null())
    }

    /// Checks whether `param` has a non-blank / non-empty value (`JsonCommand.hasParameterValue`).
    pub fn has_parameter_value(&self, param: &str) -> bool {
        let Some(val) = self.json_element(param) else {
            return false;
        };
        match val {
            Value::Null => false,
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(obj) => !obj.is_empty(),
            Value::String(s) => !s.trim().is_empty(),
            Value::Number(_) | Value::Bool(_) => true,
        }
    }

    /// Extracts a non-blank string parameter (`FromJsonHelper.extractStringNamed` / `JsonCommand.stringValueOfParameterNamed`).
    pub fn string_value_of_parameter_named(&self, param: &str) -> Option<String> {
        let val = self.json_element(param)?;
        match val {
            Value::Null | Value::Array(_) | Value::Object(_) => None,
            Value::String(s) => {
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s.clone())
                }
            }
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
        }
    }

    /// Extracts a `Long` (`i64`) parameter (`JsonCommand.longValueOfParameterNamed`).
    pub fn long_value_of_parameter_named(&self, param: &str) -> Result<Option<i64>, FineractError> {
        let Some(val) = self.json_element(param) else {
            return Ok(None);
        };
        let locale = self.locale();
        FineractNumberParser::parse_i64_for_param(param, val, &locale)
    }

    /// Extracts a locale-formatted `Integer` (`i32`) parameter (`JsonCommand.integerValueOfParameterNamed`).
    pub fn integer_value_of_parameter_named(
        &self,
        param: &str,
    ) -> Result<Option<i32>, FineractError> {
        let Some(val) = self.json_element(param) else {
            return Ok(None);
        };
        let locale = self.locale();
        FineractNumberParser::parse_i32_for_param(param, val, &locale)
    }

    /// Extracts an unformatted `Integer` (`i32`) parameter without requiring a `"locale"` attribute
    /// (`JsonCommand.integerValueSansLocaleOfParameterNamed`).
    pub fn integer_value_sans_locale_of_parameter_named(
        &self,
        param: &str,
    ) -> Result<Option<i32>, FineractError> {
        let Some(val) = self.json_element(param) else {
            return Ok(None);
        };
        FineractNumberParser::parse_i32_sans_locale(val, param)
    }

    /// Extracts a locale-formatted `BigDecimal` (`rust_decimal::Decimal`) parameter
    /// (`JsonCommand.bigDecimalValueOfParameterNamed`).
    pub fn big_decimal_value_of_parameter_named(
        &self,
        param: &str,
    ) -> Result<Option<Decimal>, FineractError> {
        let Some(val) = self.json_element(param) else {
            return Ok(None);
        };
        let locale = self.locale();
        FineractNumberParser::parse_decimal_for_param(param, val, &locale)
    }

    /// Extracts a `BigDecimal` parameter and normalizes `0` to `None`
    /// (`JsonCommand.bigDecimalValueOfParameterNamedDefaultToNullIfZero`).
    pub fn big_decimal_value_of_parameter_named_default_to_null_if_zero(
        &self,
        param: &str,
    ) -> Result<Option<Decimal>, FineractError> {
        let val = self.big_decimal_value_of_parameter_named(param)?;
        Ok(val.filter(|d| !d.is_zero()))
    }

    /// Extracts a nullable `Boolean` parameter (`JsonCommand.booleanObjectValueOfParameterNamed`).
    pub fn boolean_object_value_of_parameter_named(&self, param: &str) -> Option<bool> {
        let val = self.json_element(param)?;
        match val {
            Value::Bool(b) => Some(*b),
            Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    /// Extracts a primitive `bool` parameter defaulting to `false` if absent or null
    /// (`JsonCommand.booleanPrimitiveValueOfParameterNamed`).
    pub fn boolean_primitive_value_of_parameter_named(&self, param: &str) -> bool {
        self.boolean_object_value_of_parameter_named(param)
            .unwrap_or(false)
    }

    /// Extracts a `LocalDate` (`chrono::NaiveDate`) parameter using the sibling `"dateFormat"` and `"locale"`
    /// attributes (`JsonCommand.localDateValueOfParameterNamed`). Also supports `[year, month, day]` JSON arrays.
    pub fn local_date_value_of_parameter_named(
        &self,
        param: &str,
    ) -> Result<Option<NaiveDate>, FineractError> {
        let Some(val) = self.json_element(param) else {
            return Ok(None);
        };
        match val {
            Value::Null => Ok(None),
            Value::Array(arr) => FineractDateParser::parse_date_array(arr, param).map(Some),
            Value::String(s) => {
                if s.trim().is_empty() {
                    return Ok(None);
                }
                let date_format = self.date_format();
                let locale = self.locale();
                FineractDateParser::parse_date_for_param(param, s, &date_format, &locale).map(Some)
            }
            other => Err(validation_error(
                "validation.msg.invalid.dateFormat.format",
                format!("The parameter `{param}` is not a valid date string."),
                param,
                Some(other.clone()),
            )),
        }
    }

    /// Extracts a `String[]` array parameter (`JsonCommand.arrayValueOfParameterNamed`).
    pub fn array_value_of_parameter_named(&self, param: &str) -> Option<Vec<String>> {
        let arr = self.json_element(param)?.as_array()?;
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            match item {
                Value::String(s) => out.push(s.clone()),
                Value::Number(n) => out.push(n.to_string()),
                Value::Bool(b) => out.push(b.to_string()),
                _ => {}
            }
        }
        Some(out)
    }

    /// Checks whether `param` is present in the command and differs from `existing`
    /// (`JsonCommand.isChangeInStringParameterNamed`).
    pub fn is_change_in_string_parameter_named(
        &self,
        param: &str,
        existing: Option<&str>,
    ) -> bool {
        if !self.parameter_exists(param) {
            return false;
        }
        let working = self.string_value_of_parameter_named(param);
        let existing_norm = existing.filter(|s| !s.is_empty());
        working.as_deref() != existing_norm
    }

    /// Checks whether `param` is present in the command and differs from `existing`
    /// using scale-invariant decimal comparison (`MathUtil.isEqualTo` in `JsonCommand.isChangeInBigDecimalParameterNamed`).
    pub fn is_change_in_big_decimal_parameter_named(
        &self,
        param: &str,
        existing: Option<Decimal>,
    ) -> Result<bool, FineractError> {
        if !self.parameter_exists(param) {
            return Ok(false);
        }
        let working = self.big_decimal_value_of_parameter_named(param)?;
        match (existing, working) {
            (None, None) => Ok(false),
            (Some(a), Some(b)) => Ok(a != b),
            (Some(_), None) | (None, Some(_)) => Ok(true),
        }
    }

    /// Checks whether `param` is present in the command and differs from `existing` `NaiveDate`
    /// (`JsonCommand.isChangeInLocalDateParameterNamed`).
    pub fn is_change_in_local_date_parameter_named(
        &self,
        param: &str,
        existing: Option<NaiveDate>,
    ) -> Result<bool, FineractError> {
        if !self.parameter_exists(param) {
            return Ok(false);
        }
        let working = self.local_date_value_of_parameter_named(param)?;
        Ok(existing != working)
    }

    /// Checks whether `param` is present in the command and differs from `existing` `i64`
    /// (`JsonCommand.isChangeInLongParameterNamed`).
    pub fn is_change_in_long_parameter_named(
        &self,
        param: &str,
        existing: Option<i64>,
    ) -> Result<bool, FineractError> {
        if !self.parameter_exists(param) {
            return Ok(false);
        }
        let working = self.long_value_of_parameter_named(param)?;
        Ok(existing != working)
    }

    /// Checks whether `param` is present in the command and differs from `existing` `i32`
    /// (`JsonCommand.isChangeInIntegerParameterNamed`).
    pub fn is_change_in_integer_parameter_named(
        &self,
        param: &str,
        existing: Option<i32>,
    ) -> Result<bool, FineractError> {
        if !self.parameter_exists(param) {
            return Ok(false);
        }
        let working = self.integer_value_of_parameter_named(param)?;
        Ok(existing != working)
    }

    /// Checks whether `param` is present in the command and differs from `existing` `bool`
    /// (`JsonCommand.isChangeInBooleanParameterNamed`).
    pub fn is_change_in_boolean_parameter_named(
        &self,
        param: &str,
        existing: Option<bool>,
    ) -> bool {
        if !self.parameter_exists(param) {
            return false;
        }
        let working = self.boolean_object_value_of_parameter_named(param);
        existing != working
    }

    /// Validates that all top-level keys in the JSON payload belong to `supported_params`
    /// (`FromJsonHelper.checkForUnsupportedParameters`).
    pub fn check_for_unsupported_parameters(
        &self,
        supported_params: &[&str],
    ) -> Result<(), FineractError> {
        let Some(obj) = self.parsed.as_object() else {
            return Err(validation_error(
                "error.msg.invalid.json.data",
                "Expected a JSON object.",
                "json",
                None,
            ));
        };
        let mut errors = Vec::new();
        for key in obj.keys() {
            if !supported_params.contains(&key.as_str()) {
                let msg = format!("The parameter `{key}` is not supported.");
                errors.push(ApiParameterError {
                    developer_message: msg.clone(),
                    default_user_message: msg,
                    user_message_globalisation_code: "error.msg.parameter.unsupported".to_string(),
                    parameter_name: key.clone(),
                    value: None,
                    args: vec![ApiErrorArg {
                        value: Value::String(key.clone()),
                    }],
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(FineractError::DataValidation(
                PlatformApiDataValidationException::new(errors),
            ))
        }
    }

    /// Records a string parameter change into `changes` if it differs from `existing`.
    pub fn update_string_if_changed(
        &self,
        param: &str,
        existing: Option<&str>,
        changes: &mut HashMap<String, Value>,
    ) -> Option<String> {
        if self.is_change_in_string_parameter_named(param, existing) {
            let new_val = self.string_value_of_parameter_named(param);
            changes.insert(
                param.to_string(),
                new_val
                    .clone()
                    .map(Value::String)
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            new_val
        } else {
            existing.map(ToString::to_string)
        }
    }

    /// Records a `Decimal` parameter change into `changes` (plus `"locale"` when formatted) if it differs from `existing`.
    pub fn update_decimal_if_changed(
        &self,
        param: &str,
        existing: Option<Decimal>,
        changes: &mut HashMap<String, Value>,
    ) -> Result<Option<Decimal>, FineractError> {
        if self.is_change_in_big_decimal_parameter_named(param, existing)? {
            let new_val = self.big_decimal_value_of_parameter_named(param)?;
            changes.insert(
                param.to_string(),
                new_val
                    .map(|d| Value::String(d.normalize().to_string()))
                    .unwrap_or(Value::Null),
            );
            let loc = self.locale();
            if !loc.is_empty() {
                changes.insert("locale".to_string(), Value::String(loc));
            }
            Ok(new_val)
        } else {
            Ok(existing)
        }
    }

    /// Records a `NaiveDate` parameter change into `changes` (plus `"dateFormat"` and `"locale"`) if it differs from `existing`.
    pub fn update_date_if_changed(
        &self,
        param: &str,
        existing: Option<NaiveDate>,
        changes: &mut HashMap<String, Value>,
    ) -> Result<Option<NaiveDate>, FineractError> {
        if self.is_change_in_local_date_parameter_named(param, existing)? {
            let new_val = self.local_date_value_of_parameter_named(param)?;
            let raw_str = self
                .string_value_of_parameter_named(param)
                .or_else(|| new_val.map(|d| d.format("%Y-%m-%d").to_string()));
            changes.insert(
                param.to_string(),
                raw_str.map(Value::String).unwrap_or(Value::Null),
            );
            let df = self.date_format();
            if !df.is_empty() {
                changes.insert("dateFormat".to_string(), Value::String(df));
            }
            let loc = self.locale();
            if !loc.is_empty() {
                changes.insert("locale".to_string(), Value::String(loc));
            }
            Ok(new_val)
        } else {
            Ok(existing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_locale_separators_and_validation() {
        let en_us = FineractLocale::parse("en_US").unwrap();
        assert_eq!(en_us.thousands_separator(), ',');
        assert_eq!(en_us.decimal_separator(), '.');

        let de_de = FineractLocale::parse("de_DE").unwrap();
        assert_eq!(de_de.thousands_separator(), '.');
        assert_eq!(de_de.decimal_separator(), ',');

        let fr_fr = FineractLocale::parse("fr_FR").unwrap();
        assert_eq!(fr_fr.thousands_separator(), ' ');
        assert_eq!(fr_fr.decimal_separator(), ',');
        assert!(fr_fr.is_grouping_separator(' '));
        assert!(fr_fr.is_grouping_separator('\u{00a0}'));
        assert!(fr_fr.is_grouping_separator('\u{202f}'));

        let es_es = FineractLocale::parse("es_ES").unwrap();
        assert_eq!(es_es.thousands_separator(), '.');
        assert_eq!(es_es.decimal_separator(), ',');

        assert!(FineractLocale::parse("").is_err());
        assert!(FineractLocale::parse("xx_YY").is_err());
    }

    #[test]
    fn test_number_parser_en_us_de_de_fr_fr_decimals() {
        let expected = Decimal::from_str("10000.50").unwrap();

        // en_US: "10,000.50"
        let us_val = serde_json::json!("10,000.50");
        let parsed_us = FineractNumberParser::parse_decimal(&us_val, "en_US")
            .unwrap()
            .unwrap();
        assert_eq!(parsed_us, expected);

        // de_DE: "10.000,50"
        let de_val = serde_json::json!("10.000,50");
        let parsed_de = FineractNumberParser::parse_decimal(&de_val, "de_DE")
            .unwrap()
            .unwrap();
        assert_eq!(parsed_de, expected);

        // fr_FR: "10 000,50" and non-breaking space "10\u{00a0}000,50"
        let fr_val = serde_json::json!("10 000,50");
        let parsed_fr = FineractNumberParser::parse_decimal(&fr_val, "fr_FR")
            .unwrap()
            .unwrap();
        assert_eq!(parsed_fr, expected);

        let fr_nbsp = serde_json::json!("10\u{00a0}000,50");
        let parsed_fr_nbsp = FineractNumberParser::parse_decimal(&fr_nbsp, "fr_FR")
            .unwrap()
            .unwrap();
        assert_eq!(parsed_fr_nbsp, expected);

        // JSON numeric literal
        let raw_num = serde_json::json!(10000.5);
        let parsed_num = FineractNumberParser::parse_decimal(&raw_num, "en_US")
            .unwrap()
            .unwrap();
        assert_eq!(parsed_num, expected);

        // Integers with grouping separators vs rejected decimal separators
        assert_eq!(
            FineractNumberParser::parse_i32(&serde_json::json!("10.000"), "de_DE").unwrap(),
            Some(10_000)
        );
        assert!(FineractNumberParser::parse_i32(&serde_json::json!("10,5"), "de_DE").is_err());
    }

    #[test]
    fn test_date_parser_patterns_and_multilingual_months() {
        let expected = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();

        assert_eq!(
            FineractDateParser::parse_date("30 October 2026", "dd MMMM yyyy", "en_US").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("30 Oct 2026", "dd MMM yyyy", "en_US").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("30 Oktober 2026", "dd MMMM yyyy", "de_DE").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("15 März 2026", "dd MMMM yyyy", "de_DE").unwrap(),
            NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
        );
        assert_eq!(
            FineractDateParser::parse_date("30 octobre 2026", "dd MMMM yyyy", "fr_FR").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("12 février 2026", "dd MMMM yyyy", "fr_FR").unwrap(),
            NaiveDate::from_ymd_opt(2026, 2, 12).unwrap()
        );
        assert_eq!(
            FineractDateParser::parse_date("30 octubre 2026", "dd MMMM yyyy", "es_ES").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("2026-10-30", "yyyy-MM-dd", "en").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("30/10/2026", "dd/MM/yyyy", "en_GB").unwrap(),
            expected
        );
        assert_eq!(
            FineractDateParser::parse_date("10/30/2026", "MM/dd/yyyy", "en_US").unwrap(),
            expected
        );

        // Strict calendar validation rejects non-leap year Feb 29
        assert!(FineractDateParser::parse_date("29 February 2026", "dd MMMM yyyy", "en").is_err());
        assert!(FineractDateParser::parse_date("29 February 2024", "dd MMMM yyyy", "en").is_ok());
    }

    #[test]
    fn test_json_command_extraction_and_change_detection() {
        let payload = r#"{
            "locale": "de_DE",
            "dateFormat": "dd MMMM yyyy",
            "firstname": "Maximilian",
            "principal": "25.000,75",
            "numberOfRepayments": "12",
            "submittedOnDate": "30 Oktober 2026",
            "active": true,
            "externalId": "900001"
        }"#;

        let cmd = JsonCommand::from_json(payload)
            .unwrap()
            .with_command_id(42)
            .with_client_id(7)
            .with_loan_id(101);

        assert_eq!(cmd.command_id, Some(42));
        assert_eq!(cmd.client_id, Some(7));
        assert_eq!(cmd.loan_id, Some(101));
        assert_eq!(cmd.locale(), "de_DE");
        assert_eq!(cmd.date_format(), "dd MMMM yyyy");
        assert!(cmd.parameter_exists("principal"));
        assert!(!cmd.parameter_exists("nonExistentParam"));

        assert_eq!(
            cmd.string_value_of_parameter_named("firstname").as_deref(),
            Some("Maximilian")
        );
        assert_eq!(
            cmd.big_decimal_value_of_parameter_named("principal")
                .unwrap(),
            Some(Decimal::from_str("25000.75").unwrap())
        );
        assert_eq!(
            cmd.integer_value_of_parameter_named("numberOfRepayments")
                .unwrap(),
            Some(12)
        );
        assert_eq!(
            cmd.long_value_of_parameter_named("externalId").unwrap(),
            Some(900_001)
        );
        assert_eq!(
            cmd.boolean_object_value_of_parameter_named("active"),
            Some(true)
        );
        assert_eq!(
            cmd.local_date_value_of_parameter_named("submittedOnDate")
                .unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap())
        );

        // Scale-invariant BigDecimal comparison: 25000.7500 == 25000.75 -> no change
        assert!(!cmd
            .is_change_in_big_decimal_parameter_named(
                "principal",
                Some(Decimal::from_str("25000.7500").unwrap())
            )
            .unwrap());

        // Track changes into HashMap<String, serde_json::Value>
        let mut changes: HashMap<String, Value> = HashMap::new();
        cmd.update_string_if_changed("firstname", Some("Max"), &mut changes);
        cmd.update_decimal_if_changed(
            "principal",
            Some(Decimal::from_str("20000.00").unwrap()),
            &mut changes,
        )
        .unwrap();
        cmd.update_date_if_changed(
            "submittedOnDate",
            Some(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()),
            &mut changes,
        )
        .unwrap();

        assert_eq!(
            changes.get("firstname"),
            Some(&Value::String("Maximilian".to_string()))
        );
        assert_eq!(
            changes.get("principal"),
            Some(&Value::String("25000.75".to_string()))
        );
        assert_eq!(
            changes.get("submittedOnDate"),
            Some(&Value::String("30 Oktober 2026".to_string()))
        );
        assert_eq!(
            changes.get("locale"),
            Some(&Value::String("de_DE".to_string()))
        );
        assert_eq!(
            changes.get("dateFormat"),
            Some(&Value::String("dd MMMM yyyy".to_string()))
        );
    }
}
