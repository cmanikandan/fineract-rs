//! Apache Fineract Calendar, Recurrence Rules (RFC 2445 / RFC 5545), Working Days, and Holiday Rescheduling.
//!
//! Ported from:
//! - `org.apache.fineract.portfolio.calendar.service.CalendarUtils`
//! - `org.apache.fineract.portfolio.calendar.domain.CalendarFrequencyType`
//! - `org.apache.fineract.portfolio.calendar.domain.CalendarWeekDaysType`
//! - `org.apache.fineract.portfolio.common.domain.NthDayType`
//! - `org.apache.fineract.portfolio.common.domain.MonthlyDueDateResolver`
//! - `org.apache.fineract.portfolio.common.domain.MonthEndDueDateStrategy`
//! - `org.apache.fineract.organisation.workingdays.domain.RepaymentRescheduleType`
//! - `org.apache.fineract.organisation.workingdays.domain.WorkingDays`
//! - `org.apache.fineract.organisation.workingdays.service.WorkingDaysUtil`
//! - `org.apache.fineract.organisation.holiday.domain.Holiday`
//! - `org.apache.fineract.organisation.holiday.domain.RescheduleType`
//! - `org.apache.fineract.organisation.holiday.service.HolidayUtil`

use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors arising from parsing or validating calendar recurrence rules (`RRULE`).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CalendarError {
    #[error("empty recurrence rule")]
    EmptyRule,
    #[error("missing FREQ in recurrence rule: '{0}'")]
    MissingFrequency(String),
    #[error("invalid calendar frequency: '{0}'")]
    InvalidFrequency(String),
    #[error("invalid recurrence interval: '{0}'")]
    InvalidInterval(String),
    #[error("invalid weekday in BYDAY: '{0}'")]
    InvalidWeekDay(String),
    #[error("invalid BYSETPOS value: '{0}'")]
    InvalidBySetPos(String),
    #[error("invalid BYMONTHDAY value: '{0}'")]
    InvalidByMonthDay(String),
    #[error("invalid COUNT value: '{0}'")]
    InvalidCount(String),
    #[error("invalid UNTIL date: '{0}'")]
    InvalidUntil(String),
    #[error("malformed RRULE segment: '{0}'")]
    MalformedSegment(String),
}

/// Calendar recurrence frequency (`CalendarFrequencyType.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum CalendarFrequencyType {
    #[default]
    Invalid = 0,
    Daily = 1,
    Weekly = 2,
    Monthly = 3,
    Yearly = 4,
}

impl CalendarFrequencyType {
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Invalid),
            1 => Some(Self::Daily),
            2 => Some(Self::Weekly),
            3 => Some(Self::Monthly),
            4 => Some(Self::Yearly),
            _ => None,
        }
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "calendarFrequencyType.invalid",
            Self::Daily => "calendarFrequencyType.daily",
            Self::Weekly => "calendarFrequencyType.weekly",
            Self::Monthly => "calendarFrequencyType.monthly",
            Self::Yearly => "calendarFrequencyType.yearly",
        }
    }

    pub const fn as_rrule_str(self) -> &'static str {
        match self {
            Self::Invalid => "INVALID",
            Self::Daily => "DAILY",
            Self::Weekly => "WEEKLY",
            Self::Monthly => "MONTHLY",
            Self::Yearly => "YEARLY",
        }
    }

    pub const fn is_daily(self) -> bool {
        matches!(self, Self::Daily)
    }

    pub const fn is_weekly(self) -> bool {
        matches!(self, Self::Weekly)
    }

    pub const fn is_monthly(self) -> bool {
        matches!(self, Self::Monthly)
    }

    pub const fn is_yearly(self) -> bool {
        matches!(self, Self::Yearly)
    }

    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }

    pub const fn is_valid(self) -> bool {
        !matches!(self, Self::Invalid)
    }
}

impl fmt::Display for CalendarFrequencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_rrule_str())
    }
}

impl FromStr for CalendarFrequencyType {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "DAILY" | "DAYS" => Ok(Self::Daily),
            "WEEKLY" | "WEEKS" => Ok(Self::Weekly),
            "MONTHLY" | "MONTHS" => Ok(Self::Monthly),
            "YEARLY" | "YEARS" => Ok(Self::Yearly),
            _ => Err(CalendarError::InvalidFrequency(s.to_string())),
        }
    }
}

/// RFC 2445 / RFC 5545 two-letter day-of-week codes (`CalendarWeekDaysType.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum CalendarWeekDaysType {
    MO = 1,
    TU = 2,
    WE = 3,
    TH = 4,
    FR = 5,
    SA = 6,
    SU = 7,
}

impl CalendarWeekDaysType {
    pub const fn from_int(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::MO),
            2 => Some(Self::TU),
            3 => Some(Self::WE),
            4 => Some(Self::TH),
            5 => Some(Self::FR),
            6 => Some(Self::SA),
            7 => Some(Self::SU),
            _ => None,
        }
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::MO => "calendarWeekDaysType.monday",
            Self::TU => "calendarWeekDaysType.tuesday",
            Self::WE => "calendarWeekDaysType.wednesday",
            Self::TH => "calendarWeekDaysType.thursday",
            Self::FR => "calendarWeekDaysType.friday",
            Self::SA => "calendarWeekDaysType.saturday",
            Self::SU => "calendarWeekDaysType.sunday",
        }
    }

    pub const fn full_name(self) -> &'static str {
        match self {
            Self::MO => "Monday",
            Self::TU => "Tuesday",
            Self::WE => "Wednesday",
            Self::TH => "Thursday",
            Self::FR => "Friday",
            Self::SA => "Saturday",
            Self::SU => "Sunday",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MO => "MO",
            Self::TU => "TU",
            Self::WE => "WE",
            Self::TH => "TH",
            Self::FR => "FR",
            Self::SA => "SA",
            Self::SU => "SU",
        }
    }

    pub const fn from_weekday(weekday: Weekday) -> Self {
        match weekday {
            Weekday::Mon => Self::MO,
            Weekday::Tue => Self::TU,
            Weekday::Wed => Self::WE,
            Weekday::Thu => Self::TH,
            Weekday::Fri => Self::FR,
            Weekday::Sat => Self::SA,
            Weekday::Sun => Self::SU,
        }
    }

    pub const fn to_weekday(self) -> Weekday {
        match self {
            Self::MO => Weekday::Mon,
            Self::TU => Weekday::Tue,
            Self::WE => Weekday::Wed,
            Self::TH => Weekday::Thu,
            Self::FR => Weekday::Fri,
            Self::SA => Weekday::Sat,
            Self::SU => Weekday::Sun,
        }
    }
}

impl fmt::Display for CalendarWeekDaysType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CalendarWeekDaysType {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "MO" | "MON" | "MONDAY" => Ok(Self::MO),
            "TU" | "TUE" | "TUESDAY" => Ok(Self::TU),
            "WE" | "WED" | "WEDNESDAY" => Ok(Self::WE),
            "TH" | "THU" | "THURSDAY" => Ok(Self::TH),
            "FR" | "FRI" | "FRIDAY" => Ok(Self::FR),
            "SA" | "SAT" | "SATURDAY" => Ok(Self::SA),
            "SU" | "SUN" | "SUNDAY" => Ok(Self::SU),
            _ => Err(CalendarError::InvalidWeekDay(s.to_string())),
        }
    }
}

/// Nth day of month selector (`NthDayType.java` / `CalendarUtils.NthDayNameEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum CalendarNthDayType {
    Invalid = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Last = -1,
    OnDay = -2,
}

pub type NthDayType = CalendarNthDayType;

impl CalendarNthDayType {
    pub const fn from_int(value: i32) -> Self {
        match value {
            1 => Self::One,
            2 => Self::Two,
            3 => Self::Three,
            4 => Self::Four,
            5 => Self::Five,
            -1 => Self::Last,
            -2 => Self::OnDay,
            _ => Self::Invalid,
        }
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::One => "nthDayType.one",
            Self::Two => "nthDayType.two",
            Self::Three => "nthDayType.three",
            Self::Four => "nthDayType.four",
            Self::Five => "nthDayType.five",
            Self::Last => "nthDayType.last",
            Self::OnDay => "nthDayType.onday",
            Self::Invalid => "nthDayType.invalid",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::One => "First",
            Self::Two => "Second",
            Self::Three => "Third",
            Self::Four => "Fourth",
            Self::Five => "Fifth",
            Self::Last => "Last",
            Self::OnDay => "OnDay",
            Self::Invalid => "Invalid",
        }
    }

    pub const fn is_last_day(self) -> bool {
        matches!(self, Self::Last)
    }

    pub const fn is_on_day(self) -> bool {
        matches!(self, Self::OnDay)
    }
}

/// Non-working day repayment rescheduling strategy (`RepaymentRescheduleType.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RepaymentRescheduleType {
    Invalid = 0,
    SameDay = 1,
    MoveToNextWorkingDay = 2,
    MoveToNextRepaymentMeetingDay = 3,
    MoveToPreviousWorkingDay = 4,
    MoveToNextMeetingDay = 5,
}

impl RepaymentRescheduleType {
    pub const fn from_int(value: i32) -> Self {
        match value {
            1 => Self::SameDay,
            2 => Self::MoveToNextWorkingDay,
            3 => Self::MoveToNextRepaymentMeetingDay,
            4 => Self::MoveToPreviousWorkingDay,
            5 => Self::MoveToNextMeetingDay,
            _ => Self::Invalid,
        }
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::Invalid => "RepaymentRescheduleType.invalid",
            Self::SameDay => "RepaymentRescheduleType.same.day",
            Self::MoveToNextWorkingDay => "RepaymentRescheduleType.move.to.next.working.day",
            Self::MoveToNextRepaymentMeetingDay => {
                "RepaymentRescheduleType.move.to.next.repayment.meeting.day"
            }
            Self::MoveToPreviousWorkingDay => {
                "RepaymentRescheduleType.move.to.previous.working.day"
            }
            Self::MoveToNextMeetingDay => "RepaymentRescheduleType.move.to.next.meeting.day",
        }
    }

    pub const fn is_move_to_next_repayment_day(self) -> bool {
        matches!(
            self,
            Self::MoveToNextRepaymentMeetingDay | Self::MoveToNextMeetingDay
        )
    }
}

/// Holiday repayment rescheduling strategy (`org.apache.fineract.organisation.holiday.domain.RescheduleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum HolidayRescheduleType {
    Invalid = 0,
    RescheduleToNextRepaymentDate = 1,
    RescheduleToSpecificDate = 2,
}

impl HolidayRescheduleType {
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::RescheduleToNextRepaymentDate,
            2 => Self::RescheduleToSpecificDate,
            _ => Self::Invalid,
        }
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Strategy for placing monthly due dates when the seed anchor day (29th, 30th, 31st)
/// is absent in the target month (`MonthEndDueDateStrategy.java`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MonthEndDueDateStrategy {
    /// Clamps a missing anchor day back to the last day of the target month (e.g., Jan 31 + 1 month -> Feb 28/29).
    #[default]
    LastDayOfMonth,
    /// Rolls a missing anchor day forward to the 1st of the following month (e.g., Jan 31 + 1 month -> Mar 1).
    FirstDayOfNextMonth,
}

impl MonthEndDueDateStrategy {
    pub const fn rolls_forward(self) -> bool {
        matches!(self, Self::FirstDayOfNextMonth)
    }
}

/// Parsed RFC 2445 / RFC 5545 `RRULE` recurrence specification used by Apache Fineract.
///
/// Examples of supported rules:
/// - `FREQ=MONTHLY;INTERVAL=1`
/// - `FREQ=WEEKLY;INTERVAL=2;BYDAY=MO`
/// - `FREQ=MONTHLY;INTERVAL=1;BYSETPOS=2;BYDAY=TU`
/// - `FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,TU,WE,TH,FR`
/// - `FREQ=MONTHLY;INTERVAL=1;BYMONTHDAY=-1`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceRule {
    pub frequency: CalendarFrequencyType,
    pub interval: u32,
    pub by_day: Vec<CalendarWeekDaysType>,
    pub by_set_pos: Option<i32>,
    pub by_month_day: Option<i32>,
    pub count: Option<u32>,
    pub until: Option<NaiveDate>,
}

impl RecurrenceRule {
    pub fn new(frequency: CalendarFrequencyType, interval: u32) -> Self {
        Self {
            frequency,
            interval: interval.max(1),
            by_day: Vec::new(),
            by_set_pos: None,
            by_month_day: None,
            count: None,
            until: None,
        }
    }

    pub fn with_by_day(mut self, by_day: Vec<CalendarWeekDaysType>) -> Self {
        self.by_day = by_day;
        self
    }

    pub fn with_by_set_pos(mut self, by_set_pos: i32) -> Self {
        self.by_set_pos = Some(by_set_pos);
        self
    }

    pub fn with_by_month_day(mut self, by_month_day: i32) -> Self {
        self.by_month_day = Some(by_month_day);
        self
    }

    pub fn with_count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    pub fn with_until(mut self, until: NaiveDate) -> Self {
        self.until = Some(until);
        self
    }

    /// Parses an RFC 2445 / RFC 5545 `RRULE` string.
    pub fn parse(rrule: &str) -> Result<Self, CalendarError> {
        let trimmed = rrule.trim();
        if trimmed.is_empty() {
            return Err(CalendarError::EmptyRule);
        }

        let body = if trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case("RRULE:") {
            trimmed[6..].trim()
        } else {
            trimmed
        };

        if body.is_empty() {
            return Err(CalendarError::EmptyRule);
        }

        let mut frequency: Option<CalendarFrequencyType> = None;
        let mut interval: u32 = 1;
        let mut by_day: Vec<CalendarWeekDaysType> = Vec::new();
        let mut by_set_pos: Option<i32> = None;
        let mut by_month_day: Option<i32> = None;
        let mut count: Option<u32> = None;
        let mut until: Option<NaiveDate> = None;

        for part in body.split(';') {
            let segment = part.trim();
            if segment.is_empty() {
                continue;
            }
            let Some((raw_key, raw_val)) = segment.split_once('=') else {
                return Err(CalendarError::MalformedSegment(segment.to_string()));
            };
            let key = raw_key.trim().to_ascii_uppercase();
            let val = raw_val.trim();

            match key.as_str() {
                "FREQ" => {
                    frequency = Some(CalendarFrequencyType::from_str(val)?);
                }
                "INTERVAL" => {
                    let parsed = val
                        .parse::<i32>()
                        .map_err(|_| CalendarError::InvalidInterval(val.to_string()))?;
                    if parsed <= 0 {
                        return Err(CalendarError::InvalidInterval(val.to_string()));
                    }
                    interval = parsed as u32;
                }
                "BYDAY" => {
                    for token in val.split(',') {
                        let tok = token.trim();
                        if tok.is_empty() {
                            return Err(CalendarError::InvalidWeekDay(val.to_string()));
                        }
                        // Support optional numeric prefix in BYDAY (e.g., "2TU" or "-1FR")
                        let alpha_start = tok
                            .find(|c: char| c.is_ascii_alphabetic())
                            .ok_or_else(|| CalendarError::InvalidWeekDay(tok.to_string()))?;
                        if alpha_start > 0 {
                            let prefix = &tok[..alpha_start];
                            let pos = prefix
                                .parse::<i32>()
                                .map_err(|_| CalendarError::InvalidWeekDay(tok.to_string()))?;
                            if pos == 0 || !(-5..=5).contains(&pos) {
                                return Err(CalendarError::InvalidBySetPos(prefix.to_string()));
                            }
                            if by_set_pos.is_none() {
                                by_set_pos = Some(pos);
                            }
                        }
                        let day_code = &tok[alpha_start..];
                        let day = CalendarWeekDaysType::from_str(day_code)?;
                        if !by_day.contains(&day) {
                            by_day.push(day);
                        }
                    }
                }
                "BYSETPOS" => {
                    let pos = val
                        .parse::<i32>()
                        .map_err(|_| CalendarError::InvalidBySetPos(val.to_string()))?;
                    if pos == 0 || !(-31..=31).contains(&pos) {
                        return Err(CalendarError::InvalidBySetPos(val.to_string()));
                    }
                    by_set_pos = Some(pos);
                }
                "BYMONTHDAY" => {
                    let mday = val
                        .parse::<i32>()
                        .map_err(|_| CalendarError::InvalidByMonthDay(val.to_string()))?;
                    if mday == 0 || !(-31..=31).contains(&mday) {
                        return Err(CalendarError::InvalidByMonthDay(val.to_string()));
                    }
                    by_month_day = Some(mday);
                }
                "COUNT" => {
                    let c = val
                        .parse::<u32>()
                        .map_err(|_| CalendarError::InvalidCount(val.to_string()))?;
                    if c == 0 {
                        return Err(CalendarError::InvalidCount(val.to_string()));
                    }
                    count = Some(c);
                }
                "UNTIL" => {
                    until = Some(parse_rrule_until_date(val)?);
                }
                _ => {
                    // Ignore additional RFC 5545 properties such as WKST=MO
                }
            }
        }

        let Some(frequency) = frequency else {
            return Err(CalendarError::MissingFrequency(rrule.to_string()));
        };

        Ok(Self {
            frequency,
            interval,
            by_day,
            by_set_pos,
            by_month_day,
            count,
            until,
        })
    }

    /// Formats the recurrence rule as a canonical Fineract `RRULE` string.
    pub fn to_rrule_string(&self) -> String {
        self.to_string()
    }

    /// Generates a human-readable description matching `CalendarUtils.getRRuleReadable`.
    pub fn human_readable(&self, start_date: NaiveDate) -> String {
        let mut out = match self.frequency {
            CalendarFrequencyType::Invalid => "Invalid".to_string(),
            CalendarFrequencyType::Daily => {
                if self.interval == 1 {
                    "Daily".to_string()
                } else {
                    format!("Every {} days", self.interval)
                }
            }
            CalendarFrequencyType::Weekly => {
                let prefix = if self.interval == 1 {
                    "Weekly".to_string()
                } else {
                    format!("Every {} weeks", self.interval)
                };
                if self.by_day.is_empty() {
                    prefix
                } else {
                    let days: String = self.by_day.iter().map(|d| d.full_name()).collect();
                    format!("{prefix} on {days}")
                }
            }
            CalendarFrequencyType::Monthly => {
                if let (Some(nth), Some(weekday)) = (self.by_set_pos, self.by_day.first()) {
                    let nth_label = CalendarNthDayType::from_int(nth)
                        .label()
                        .to_ascii_lowercase();
                    let day_label = weekday.full_name().to_ascii_lowercase();
                    if self.interval == 1 {
                        format!("Monthly on {nth_label} {day_label}")
                    } else {
                        format!("Every {} months on {nth_label} {day_label}", self.interval)
                    }
                } else if let Some(month_day) = self.by_month_day {
                    if month_day == -1 {
                        if self.interval == 1 {
                            "Monthly on last day".to_string()
                        } else {
                            format!("Every {} months on last day", self.interval)
                        }
                    } else if self.interval == 1 {
                        format!("Monthly on day {month_day}")
                    } else {
                        format!("Every {} months on day {month_day}", self.interval)
                    }
                } else if self.interval == 1 {
                    format!("Monthly on day {}", start_date.day())
                } else {
                    format!("Every {} months on day {}", self.interval, start_date.day())
                }
            }
            CalendarFrequencyType::Yearly => {
                let month_short = start_date.format("%b");
                if self.interval == 1 {
                    format!("Annually on {} {}", month_short, start_date.day())
                } else {
                    format!(
                        "Every {} years on {} {}",
                        self.interval,
                        month_short,
                        start_date.day()
                    )
                }
            }
        };

        if let Some(c) = self.count {
            if c == 1 {
                out = "Once".to_string();
            }
            out.push_str(&format!(", {c} times"));
        }

        if let Some(until) = self.until {
            out.push_str(&format!(", until {}", until.format("%d %B %y")));
        }

        out
    }
}

impl fmt::Display for RecurrenceRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FREQ={};INTERVAL={}", self.frequency, self.interval)?;
        if let Some(pos) = self.by_set_pos {
            write!(f, ";BYSETPOS={pos}")?;
        }
        if let Some(mday) = self.by_month_day {
            write!(f, ";BYMONTHDAY={mday}")?;
        }
        if !self.by_day.is_empty() {
            f.write_str(";BYDAY=")?;
            for (idx, day) in self.by_day.iter().enumerate() {
                if idx > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{day}")?;
            }
        }
        if let Some(count) = self.count {
            write!(f, ";COUNT={count}")?;
        }
        if let Some(until) = self.until {
            write!(f, ";UNTIL={}", until.format("%Y%m%d"))?;
        }
        Ok(())
    }
}

impl FromStr for RecurrenceRule {
    type Err = CalendarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Organization working days configuration (`m_working_days` / `WorkingDays.java`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingDaysConfig {
    pub recurrence: String,
    pub repayment_rescheduling_type: RepaymentRescheduleType,
    pub extend_term_for_daily_repayments: bool,
    pub extend_term_for_holidays: bool,
}

impl WorkingDaysConfig {
    pub fn new(
        recurrence: impl Into<String>,
        repayment_rescheduling_type: RepaymentRescheduleType,
        extend_term_for_daily_repayments: bool,
        extend_term_for_holidays: bool,
    ) -> Self {
        Self {
            recurrence: recurrence.into(),
            repayment_rescheduling_type,
            extend_term_for_daily_repayments,
            extend_term_for_holidays,
        }
    }

    /// Standard Monday-through-Friday working week with the given rescheduling policy.
    pub fn monday_to_friday(repayment_rescheduling_type: RepaymentRescheduleType) -> Self {
        Self {
            recurrence: "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,TU,WE,TH,FR".to_string(),
            repayment_rescheduling_type,
            extend_term_for_daily_repayments: false,
            extend_term_for_holidays: false,
        }
    }
}

impl Default for WorkingDaysConfig {
    fn default() -> Self {
        Self::monday_to_friday(RepaymentRescheduleType::MoveToNextWorkingDay)
    }
}

/// Organization holiday rule (`m_holiday` / `Holiday.java`).
///
/// `rescheduling_type` follows Fineract's `RescheduleType`:
/// - `1` (`RESCHEDULETONEXTREPAYMENTDATE`): reschedule onto `next_meeting_date`
/// - `2` (`RESCHEDULETOSPECIFICDATE`): reschedule onto `repayments_rescheduled_to`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HolidayRule {
    pub id: i64,
    pub from_date: NaiveDate,
    pub to_date: NaiveDate,
    pub repayments_rescheduled_to: Option<NaiveDate>,
    pub rescheduling_type: u8,
}

impl HolidayRule {
    pub fn to_specific_date(
        id: i64,
        from_date: NaiveDate,
        to_date: NaiveDate,
        repayments_rescheduled_to: NaiveDate,
    ) -> Self {
        Self {
            id,
            from_date,
            to_date,
            repayments_rescheduled_to: Some(repayments_rescheduled_to),
            rescheduling_type: HolidayRescheduleType::RescheduleToSpecificDate.as_u8(),
        }
    }

    pub fn to_next_repayment_date(id: i64, from_date: NaiveDate, to_date: NaiveDate) -> Self {
        Self {
            id,
            from_date,
            to_date,
            repayments_rescheduled_to: None,
            rescheduling_type: HolidayRescheduleType::RescheduleToNextRepaymentDate.as_u8(),
        }
    }

    /// Returns `true` if `date` falls within `[self.from_date, self.to_date]` inclusive.
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.from_date && date <= self.to_date
    }
}

/// Stateless calendar, working-day, and holiday calculation utilities ported from
/// `CalendarUtils`, `MonthlyDueDateResolver`, `WorkingDaysUtil`, and `HolidayUtil`.
pub struct CalendarUtils;

impl CalendarUtils {
    /// Returns total number of days in `year` (366 for leap years, 365 otherwise).
    #[inline]
    pub const fn days_in_year(year: i32) -> i64 {
        if is_leap_year(year) {
            366
        } else {
            365
        }
    }

    /// Returns the number of days between two dates (`to - from`).
    #[inline]
    pub fn days_between(from: NaiveDate, to: NaiveDate) -> i64 {
        (to - from).num_days()
    }

    /// Adds a calendar period (`interval` periods of `frequency_type`) to `date`.
    pub fn add_period(
        date: NaiveDate,
        frequency_type: CalendarFrequencyType,
        interval: i32,
        seed_day: Option<u32>,
    ) -> NaiveDate {
        match frequency_type {
            CalendarFrequencyType::Invalid => date,
            CalendarFrequencyType::Daily => {
                date.checked_add_signed(Duration::days(interval as i64)).unwrap_or(date)
            }
            CalendarFrequencyType::Weekly => {
                date.checked_add_signed(Duration::days(interval as i64 * 7)).unwrap_or(date)
            }
            CalendarFrequencyType::Monthly => {
                if let Some((target_year, target_month)) =
                    add_year_month(date.year(), date.month(), interval as i64)
                {
                    let max_day = days_in_year_month(target_year, target_month);
                    let target_day = seed_day.unwrap_or_else(|| date.day()).min(max_day);
                    NaiveDate::from_ymd_opt(target_year, target_month, target_day).unwrap_or(date)
                } else {
                    date
                }
            }
            CalendarFrequencyType::Yearly => {
                let target_year = date.year().saturating_add(interval);
                let target_month = date.month();
                let max_day = days_in_year_month(target_year, target_month);
                let target_day = seed_day.unwrap_or_else(|| date.day()).min(max_day);
                NaiveDate::from_ymd_opt(target_year, target_month, target_day).unwrap_or(date)
            }
        }
    }
    /// Generates up to `count` recurring dates on or after `start_date` (and on or after `seed_date`),
    /// preserving Fineract's end-of-month / 29th-30th-31st seed date anchoring.
    ///
    /// When `rrule.frequency == CalendarFrequencyType::Monthly` without `BYDAY`/`BYMONTHDAY` overrides,
    /// a `seed_date` on the 29th, 30th, or 31st clamps to the last day of shorter months (e.g. Feb 28 in a
    /// common year, Feb 29 in a leap year, Apr 30 in April) and snaps back to the `seed_date` anchor day
    /// in subsequent months that have enough days (e.g. Mar 31).
    pub fn generate_recurring_dates(
        rrule: &RecurrenceRule,
        seed_date: NaiveDate,
        start_date: NaiveDate,
        count: usize,
    ) -> Vec<NaiveDate> {
        if count == 0 {
            return Vec::new();
        }

        let effective_start = if start_date > seed_date {
            start_date
        } else {
            seed_date
        };

        let interval = rrule.interval.max(1);
        let max_total_occurrences = rrule.count.map(|c| c as usize);
        let mut results = Vec::with_capacity(count);
        let mut total_emitted_from_seed: usize = 0;

        let mut push_candidate = |candidate: NaiveDate| -> bool {
            if candidate < seed_date {
                return true;
            }
            if let Some(until) = rrule.until {
                if candidate > until {
                    return false;
                }
            }
            total_emitted_from_seed += 1;
            if candidate >= effective_start {
                results.push(candidate);
            }
            if results.len() >= count {
                return false;
            }
            if let Some(max_total) = max_total_occurrences {
                if total_emitted_from_seed >= max_total {
                    return false;
                }
            }
            true
        };

        match rrule.frequency {
            CalendarFrequencyType::Invalid => {}
            CalendarFrequencyType::Daily => {
                let step_days = i64::from(interval);
                let mut step_index: i64 = if max_total_occurrences.is_none() && effective_start > seed_date
                {
                    let diff = (effective_start - seed_date).num_days();
                    diff / step_days
                } else {
                    0
                };

                loop {
                    let Some(candidate) = seed_date.checked_add_signed(Duration::days(step_index * step_days))
                    else {
                        break;
                    };
                    if !push_candidate(candidate) {
                        break;
                    }
                    step_index += 1;
                }
            }
            CalendarFrequencyType::Weekly => {
                if rrule.by_day.is_empty() {
                    let step_days = i64::from(interval) * 7;
                    let mut step_index: i64 =
                        if max_total_occurrences.is_none() && effective_start > seed_date {
                            let diff = (effective_start - seed_date).num_days();
                            diff / step_days
                        } else {
                            0
                        };

                    loop {
                        let Some(candidate) =
                            seed_date.checked_add_signed(Duration::days(step_index * step_days))
                        else {
                            break;
                        };
                        if !push_candidate(candidate) {
                            break;
                        }
                        step_index += 1;
                    }
                } else {
                    // Sort weekdays Monday..Sunday within each active week
                    let mut sorted_days = rrule.by_day.clone();
                    sorted_days.sort_unstable();
                    sorted_days.dedup();

                    // Monday of the ISO week containing `seed_date`
                    let days_from_monday = i64::from(seed_date.weekday().num_days_from_monday());
                    let Some(seed_week_monday) =
                        seed_date.checked_sub_signed(Duration::days(days_from_monday))
                    else {
                        return results;
                    };

                    let step_weeks_days = i64::from(interval) * 7;
                    let mut week_index: i64 = 0;

                    'weekly: loop {
                        let Some(week_monday) = seed_week_monday
                            .checked_add_signed(Duration::days(week_index * step_weeks_days))
                        else {
                            break;
                        };

                        for day_type in &sorted_days {
                            let offset = i64::from(day_type.to_weekday().num_days_from_monday());
                            let Some(candidate) = week_monday.checked_add_signed(Duration::days(offset))
                            else {
                                break 'weekly;
                            };
                            if !push_candidate(candidate) {
                                break 'weekly;
                            }
                        }
                        week_index += 1;
                    }
                }
            }
            CalendarFrequencyType::Monthly => {
                let mut month_step: i64 = 0;
                let step_months = i64::from(interval);

                'monthly: loop {
                    let total_months_offset = month_step * step_months;
                    let Some((year, month)) = add_year_month(
                        seed_date.year(),
                        seed_date.month(),
                        total_months_offset,
                    ) else {
                        break;
                    };

                    if !rrule.by_day.is_empty() {
                        let matching_dates =
                            matching_weekdays_in_month(year, month, &rrule.by_day);
                        if let Some(pos) = rrule.by_set_pos {
                            let selected = if pos > 0 {
                                matching_dates.get((pos - 1) as usize).copied()
                            } else if pos < 0 {
                                let idx = matching_dates.len() as i32 + pos;
                                if idx >= 0 {
                                    matching_dates.get(idx as usize).copied()
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            if let Some(candidate) = selected {
                                if !push_candidate(candidate) {
                                    break 'monthly;
                                }
                            }
                        } else {
                            for candidate in matching_dates {
                                if !push_candidate(candidate) {
                                    break 'monthly;
                                }
                            }
                        }
                    } else if let Some(month_day) = rrule.by_month_day {
                        let max_day = days_in_year_month(year, month);
                        let target_day = if month_day > 0 {
                            (month_day as u32).min(max_day)
                        } else {
                            let resolved = max_day as i32 + 1 + month_day;
                            resolved.clamp(1, max_day as i32) as u32
                        };
                        if let Some(candidate) = NaiveDate::from_ymd_opt(year, month, target_day) {
                            let adjusted = Self::adjust_date(
                                candidate,
                                seed_date,
                                CalendarFrequencyType::Monthly,
                            );
                            if !push_candidate(adjusted) {
                                break 'monthly;
                            }
                        }
                    } else {
                        // Standard monthly recurrence anchored on `seed_date.day()`
                        let max_day = days_in_year_month(year, month);
                        let clamped_day = seed_date.day().min(max_day);
                        let Some(candidate) = NaiveDate::from_ymd_opt(year, month, clamped_day)
                        else {
                            break 'monthly;
                        };
                        let adjusted = Self::adjust_date(
                            candidate,
                            seed_date,
                            CalendarFrequencyType::Monthly,
                        );
                        if !push_candidate(adjusted) {
                            break 'monthly;
                        }
                    }

                    month_step += 1;
                    // Guard against rules whose BYSETPOS never matches any month
                    if month_step > 12_000 {
                        break;
                    }
                }
            }
            CalendarFrequencyType::Yearly => {
                let mut year_step: i32 = 0;
                let step_years = interval as i32;

                loop {
                    let Some(target_year) = seed_date
                        .year()
                        .checked_add(year_step.saturating_mul(step_years))
                    else {
                        break;
                    };
                    let target_month = seed_date.month();
                    let max_day = days_in_year_month(target_year, target_month);
                    let target_day = seed_date.day().min(max_day);
                    let Some(candidate) =
                        NaiveDate::from_ymd_opt(target_year, target_month, target_day)
                    else {
                        break;
                    };
                    if !push_candidate(candidate) {
                        break;
                    }
                    year_step += 1;
                }
            }
        }

        results
    }

    /// Returns the first recurring date on or after `start_date` (and on or after `seed_date`).
    pub fn next_recurring_date(
        rrule: &RecurrenceRule,
        seed_date: NaiveDate,
        start_date: NaiveDate,
    ) -> Option<NaiveDate> {
        Self::generate_recurring_dates(rrule, seed_date, start_date, 1)
            .into_iter()
            .next()
    }

    /// Checks whether `date` is a valid occurrence of `rrule` anchored at `seed_date`
    /// (`CalendarUtils.isValidRecurringDate`).
    pub fn is_valid_recurring_date(
        rrule: &RecurrenceRule,
        seed_date: NaiveDate,
        date: NaiveDate,
    ) -> bool {
        if date < seed_date {
            return false;
        }
        Self::generate_recurring_dates(rrule, seed_date, date, 1)
            .first()
            .copied()
            == Some(date)
    }

    /// Snaps a monthly `date` back onto the anchor day of `seed_date` using Fineract's default
    /// `MonthEndDueDateStrategy::LastDayOfMonth` clamp (`CalendarUtils.adjustDate`).
    pub fn adjust_date(
        date: NaiveDate,
        seed_date: NaiveDate,
        frequency: CalendarFrequencyType,
    ) -> NaiveDate {
        Self::adjust_date_with_strategy(
            date,
            seed_date,
            frequency,
            MonthEndDueDateStrategy::LastDayOfMonth,
        )
    }

    /// Snaps a monthly `date` onto the anchor day of `seed_date` under the specified
    /// [`MonthEndDueDateStrategy`] (`MonthlyDueDateResolver.adjustDate`).
    pub fn adjust_date_with_strategy(
        date: NaiveDate,
        seed_date: NaiveDate,
        frequency: CalendarFrequencyType,
        strategy: MonthEndDueDateStrategy,
    ) -> NaiveDate {
        const SHORTEST_MONTH_LENGTH: u32 = 28;
        if frequency.is_monthly()
            && seed_date.day() > SHORTEST_MONTH_LENGTH
            && date.day() >= SHORTEST_MONTH_LENGTH
        {
            let days_in_current_month = days_in_year_month(date.year(), date.month());
            let seed_day = seed_date.day();
            if strategy.rolls_forward() && days_in_current_month < seed_day {
                if let Some((next_year, next_month)) = add_year_month(date.year(), date.month(), 1)
                {
                    if let Some(rolled) = NaiveDate::from_ymd_opt(next_year, next_month, 1) {
                        return rolled;
                    }
                }
            }
            let adjusted_day = days_in_current_month.min(seed_day);
            if let Some(clamped) = NaiveDate::from_ymd_opt(date.year(), date.month(), adjusted_day)
            {
                return clamped;
            }
        }
        date
    }

    /// Turns a due date rolled forward to the 1st back into the last day of the month it stands for
    /// (`MonthlyDueDateResolver.unroll`).
    pub fn unroll_monthly_date(
        date: NaiveDate,
        seed_date: NaiveDate,
        frequency: CalendarFrequencyType,
        strategy: MonthEndDueDateStrategy,
    ) -> NaiveDate {
        if !strategy.rolls_forward() || !frequency.is_monthly() || date.day() != 1 {
            return date;
        }
        let Some(end_of_prev_month) = date.checked_sub_signed(Duration::days(1)) else {
            return date;
        };
        let prev_month_len =
            days_in_year_month(end_of_prev_month.year(), end_of_prev_month.month());
        if prev_month_len < seed_date.day() {
            end_of_prev_month
        } else {
            date
        }
    }

    /// Returns `true` if `date` falls on a working day according to `working_days.recurrence`
    /// (`WorkingDaysUtil.isWorkingDay`).
    pub fn is_working_day(working_days: &WorkingDaysConfig, date: NaiveDate) -> bool {
        let Ok(rrule) = RecurrenceRule::parse(&working_days.recurrence) else {
            return false;
        };
        Self::is_valid_recurring_date(&rrule, date, date)
    }

    /// Adjusts `date` if it falls on a non-working day according to `working_days.repayment_rescheduling_type`
    /// (`WorkingDaysUtil.getOffSetDateIfNonWorkingDay`).
    pub fn offset_date_if_non_working_day(
        date: NaiveDate,
        next_meeting_date: Option<NaiveDate>,
        working_days: &WorkingDaysConfig,
    ) -> NaiveDate {
        if Self::is_working_day(working_days, date) {
            return date;
        }

        match working_days.repayment_rescheduling_type {
            RepaymentRescheduleType::Invalid | RepaymentRescheduleType::SameDay => date,
            RepaymentRescheduleType::MoveToNextWorkingDay => {
                let mut cursor = date;
                for _ in 0..366 {
                    let Some(next) = cursor.checked_add_signed(Duration::days(1)) else {
                        break;
                    };
                    cursor = next;
                    if Self::is_working_day(working_days, cursor) {
                        return cursor;
                    }
                }
                date
            }
            RepaymentRescheduleType::MoveToPreviousWorkingDay => {
                let mut cursor = date;
                for _ in 0..366 {
                    let Some(prev) = cursor.checked_sub_signed(Duration::days(1)) else {
                        break;
                    };
                    cursor = prev;
                    if Self::is_working_day(working_days, cursor) {
                        return cursor;
                    }
                }
                date
            }
            RepaymentRescheduleType::MoveToNextRepaymentMeetingDay
            | RepaymentRescheduleType::MoveToNextMeetingDay => next_meeting_date.unwrap_or(date),
        }
    }

    /// Returns the first applicable holiday covering `date`, if any (`HolidayUtil.getApplicableHoliday`).
    pub fn applicable_holiday(date: NaiveDate, holidays: &[HolidayRule]) -> Option<&HolidayRule> {
        holidays.iter().find(|h| h.contains(date))
    }

    /// Returns `true` if `date` falls within any configured holiday (`HolidayUtil.isHoliday`).
    pub fn is_holiday(date: NaiveDate, holidays: &[HolidayRule]) -> bool {
        Self::applicable_holiday(date, holidays).is_some()
    }

    /// Recursively adjusts `date` for both non-working days and holidays until it lands on an eligible date,
    /// mirroring `DefaultScheduledDateGenerator.recursivelyCheckNonWorkingDaysAndHolidaysAndWorkingDaysExemptionToGenerateNextRepaymentPeriodDate`,
    /// `WorkingDaysUtil.getOffSetDateIfNonWorkingDay`, and `HolidayUtil`.
    pub fn adjust_date_for_working_days_and_holidays(
        date: NaiveDate,
        next_meeting_date: Option<NaiveDate>,
        working_days: &WorkingDaysConfig,
        holidays: &[HolidayRule],
    ) -> NaiveDate {
        let mut current = date;

        for _ in 0..366 {
            let before_step = current;

            // 1. Apply working-day rescheduling if `current` is a non-working day
            if !Self::is_working_day(working_days, current) {
                current =
                    Self::offset_date_if_non_working_day(current, next_meeting_date, working_days);
            }

            // 2. Apply holiday rescheduling if `current` falls within any active holiday range
            while let Some(holiday) = Self::applicable_holiday(current, holidays) {
                let next_after_holiday = match HolidayRescheduleType::from_u8(holiday.rescheduling_type) {
                    HolidayRescheduleType::RescheduleToSpecificDate => holiday
                        .repayments_rescheduled_to
                        .or(next_meeting_date)
                        .unwrap_or(current),
                    HolidayRescheduleType::RescheduleToNextRepaymentDate => next_meeting_date
                        .or(holiday.repayments_rescheduled_to)
                        .unwrap_or(current),
                    HolidayRescheduleType::Invalid => holiday
                        .repayments_rescheduled_to
                        .or(next_meeting_date)
                        .unwrap_or(current),
                };

                if next_after_holiday == current {
                    break;
                }
                current = next_after_holiday;
            }

            if current == before_step {
                break;
            }
        }

        current
    }
}

/// Returns `true` if `year` is a proleptic Gregorian leap year.
pub const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in `(year, month)` where `month` is in `1..=12`.
pub const fn days_in_year_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn add_year_month(year: i32, month: u32, delta_months: i64) -> Option<(i32, u32)> {
    let total = i64::from(year)
        .checked_mul(12)?
        .checked_add(i64::from(month) - 1)?
        .checked_add(delta_months)?;
    let new_year = i32::try_from(total.div_euclid(12)).ok()?;
    let new_month = (total.rem_euclid(12) + 1) as u32;
    Some((new_year, new_month))
}

fn matching_weekdays_in_month(
    year: i32,
    month: u32,
    weekdays: &[CalendarWeekDaysType],
) -> Vec<NaiveDate> {
    let max_day = days_in_year_month(year, month);
    let mut matches = Vec::with_capacity(5);
    for day in 1..=max_day {
        if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
            let wd = CalendarWeekDaysType::from_weekday(d.weekday());
            if weekdays.contains(&wd) {
                matches.push(d);
            }
        }
    }
    matches
}

fn parse_rrule_until_date(raw: &str) -> Result<NaiveDate, CalendarError> {
    let s = raw.trim();
    // Format 1: YYYYMMDD or YYYYMMDDTHHMMSSZ
    if s.len() >= 8 && s.as_bytes()[..8].iter().all(u8::is_ascii_digit) {
        let year = s[0..4]
            .parse::<i32>()
            .map_err(|_| CalendarError::InvalidUntil(raw.to_string()))?;
        let month = s[4..6]
            .parse::<u32>()
            .map_err(|_| CalendarError::InvalidUntil(raw.to_string()))?;
        let day = s[6..8]
            .parse::<u32>()
            .map_err(|_| CalendarError::InvalidUntil(raw.to_string()))?;
        return NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| CalendarError::InvalidUntil(raw.to_string()));
    }
    // Format 2: YYYY-MM-DD
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| CalendarError::InvalidUntil(raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid test date")
    }

    #[test]
    fn test_enum_discriminants_and_roundtrips() {
        assert_eq!(CalendarFrequencyType::Daily as u8, 1);
        assert_eq!(CalendarFrequencyType::Weekly as u8, 2);
        assert_eq!(CalendarFrequencyType::Monthly as u8, 3);
        assert_eq!(CalendarFrequencyType::Yearly as u8, 4);

        assert_eq!(CalendarWeekDaysType::MO.as_int(), 1);
        assert_eq!(CalendarWeekDaysType::TU.as_int(), 2);
        assert_eq!(CalendarWeekDaysType::WE.as_int(), 3);
        assert_eq!(CalendarWeekDaysType::TH.as_int(), 4);
        assert_eq!(CalendarWeekDaysType::FR.as_int(), 5);
        assert_eq!(CalendarWeekDaysType::SA.as_int(), 6);
        assert_eq!(CalendarWeekDaysType::SU.as_int(), 7);

        assert_eq!(RepaymentRescheduleType::Invalid as u8, 0);
        assert_eq!(RepaymentRescheduleType::SameDay as u8, 1);
        assert_eq!(RepaymentRescheduleType::MoveToNextWorkingDay as u8, 2);
        assert_eq!(RepaymentRescheduleType::MoveToNextRepaymentMeetingDay as u8, 3);
        assert_eq!(RepaymentRescheduleType::MoveToPreviousWorkingDay as u8, 4);
        assert_eq!(RepaymentRescheduleType::MoveToNextMeetingDay as u8, 5);
    }

    #[test]
    fn test_rrule_parse_and_format_canonical_strings() {
        let r1 = RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1").unwrap();
        assert_eq!(r1.frequency, CalendarFrequencyType::Monthly);
        assert_eq!(r1.interval, 1);
        assert!(r1.by_day.is_empty());
        assert_eq!(r1.by_set_pos, None);
        assert_eq!(r1.to_string(), "FREQ=MONTHLY;INTERVAL=1");

        let r2 = RecurrenceRule::parse("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO").unwrap();
        assert_eq!(r2.frequency, CalendarFrequencyType::Weekly);
        assert_eq!(r2.interval, 2);
        assert_eq!(r2.by_day, vec![CalendarWeekDaysType::MO]);
        assert_eq!(r2.to_string(), "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO");

        let r3 = RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1;BYSETPOS=2;BYDAY=TU").unwrap();
        assert_eq!(r3.frequency, CalendarFrequencyType::Monthly);
        assert_eq!(r3.interval, 1);
        assert_eq!(r3.by_set_pos, Some(2));
        assert_eq!(r3.by_day, vec![CalendarWeekDaysType::TU]);
        assert_eq!(r3.to_string(), "FREQ=MONTHLY;INTERVAL=1;BYSETPOS=2;BYDAY=TU");
        assert_eq!(
            r3.human_readable(ymd(2025, 1, 1)),
            "Monthly on second tuesday"
        );

        let r4 = RecurrenceRule::parse("RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR").unwrap();
        assert_eq!(r4.frequency, CalendarFrequencyType::Weekly);
        assert_eq!(r4.interval, 1);
        assert_eq!(
            r4.by_day,
            vec![
                CalendarWeekDaysType::MO,
                CalendarWeekDaysType::TU,
                CalendarWeekDaysType::WE,
                CalendarWeekDaysType::TH,
                CalendarWeekDaysType::FR,
            ]
        );
        assert_eq!(
            r4.to_string(),
            "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,TU,WE,TH,FR"
        );
    }

    #[test]
    fn test_monthly_seed_date_clamping_and_leap_year_anchoring_31st() {
        let rrule = RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1").unwrap();

        // Leap year 2024: Jan 31 -> Feb 29 -> Mar 31 -> Apr 30 -> May 31
        let leap_seed = ymd(2024, 1, 31);
        let leap_dates = CalendarUtils::generate_recurring_dates(&rrule, leap_seed, leap_seed, 5);
        assert_eq!(
            leap_dates,
            vec![
                ymd(2024, 1, 31),
                ymd(2024, 2, 29),
                ymd(2024, 3, 31),
                ymd(2024, 4, 30),
                ymd(2024, 5, 31),
            ]
        );

        // Common year 2025: Jan 31 -> Feb 28 -> Mar 31 -> Apr 30 -> May 31
        let common_seed = ymd(2025, 1, 31);
        let common_dates =
            CalendarUtils::generate_recurring_dates(&rrule, common_seed, common_seed, 5);
        assert_eq!(
            common_dates,
            vec![
                ymd(2025, 1, 31),
                ymd(2025, 2, 28),
                ymd(2025, 3, 31),
                ymd(2025, 4, 30),
                ymd(2025, 5, 31),
            ]
        );
    }

    #[test]
    fn test_monthly_seed_date_clamping_29th_and_30th_across_leap_and_common_february() {
        let rrule = RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1").unwrap();

        // Seeded on leap day 2024-02-29: stays on 29th for Mar..Jan, clamps to 2025-02-28 in common year,
        // and snaps back to 2025-03-29 in March!
        let leap_day_seed = ymd(2024, 2, 29);
        let dates_from_2025_jan = CalendarUtils::generate_recurring_dates(
            &rrule,
            leap_day_seed,
            ymd(2025, 1, 1),
            3,
        );
        assert_eq!(
            dates_from_2025_jan,
            vec![ymd(2025, 1, 29), ymd(2025, 2, 28), ymd(2025, 3, 29)]
        );

        // Seeded on Jan 30 in leap year 2024: Jan 30 -> Feb 29 -> Mar 30 -> Apr 30
        let seed_30 = ymd(2024, 1, 30);
        let dates_30 = CalendarUtils::generate_recurring_dates(&rrule, seed_30, seed_30, 4);
        assert_eq!(
            dates_30,
            vec![
                ymd(2024, 1, 30),
                ymd(2024, 2, 29),
                ymd(2024, 3, 30),
                ymd(2024, 4, 30),
            ]
        );
    }

    #[test]
    fn test_monthly_bysetpos_and_bymonthday_recurrence() {
        // 2nd Tuesday of every month starting Jan 2025
        let second_tuesday =
            RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1;BYSETPOS=2;BYDAY=TU").unwrap();
        let seed = ymd(2025, 1, 1);
        let dates = CalendarUtils::generate_recurring_dates(&second_tuesday, seed, seed, 4);
        assert_eq!(
            dates,
            vec![
                ymd(2025, 1, 14),
                ymd(2025, 2, 11),
                ymd(2025, 3, 11),
                ymd(2025, 4, 8),
            ]
        );

        // Last Friday of every month (BYSETPOS=-1;BYDAY=FR)
        let last_friday =
            RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1;BYSETPOS=-1;BYDAY=FR").unwrap();
        let lf_dates = CalendarUtils::generate_recurring_dates(&last_friday, seed, seed, 3);
        assert_eq!(
            lf_dates,
            vec![ymd(2025, 1, 31), ymd(2025, 2, 28), ymd(2025, 3, 28)]
        );

        // Last calendar day of every month (BYMONTHDAY=-1) across leap & common years
        let last_day = RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=1;BYMONTHDAY=-1").unwrap();
        let ld_dates =
            CalendarUtils::generate_recurring_dates(&last_day, ymd(2024, 1, 31), ymd(2024, 1, 31), 4);
        assert_eq!(
            ld_dates,
            vec![
                ymd(2024, 1, 31),
                ymd(2024, 2, 29),
                ymd(2024, 3, 31),
                ymd(2024, 4, 30),
            ]
        );
    }

    #[test]
    fn test_weekly_and_biweekly_intervals() {
        let biweekly_monday =
            RecurrenceRule::parse("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO").unwrap();
        // 2025-01-06 is a Monday
        let seed = ymd(2025, 1, 6);
        let dates = CalendarUtils::generate_recurring_dates(&biweekly_monday, seed, seed, 4);
        assert_eq!(
            dates,
            vec![
                ymd(2025, 1, 6),
                ymd(2025, 1, 20),
                ymd(2025, 2, 3),
                ymd(2025, 2, 17),
            ]
        );

        // Quarterly (every 3 months) anchored on May 31
        let quarterly = RecurrenceRule::parse("FREQ=MONTHLY;INTERVAL=3").unwrap();
        let q_seed = ymd(2024, 5, 31);
        let q_dates = CalendarUtils::generate_recurring_dates(&quarterly, q_seed, q_seed, 4);
        assert_eq!(
            q_dates,
            vec![
                ymd(2024, 5, 31),
                ymd(2024, 8, 31),
                ymd(2024, 11, 30),
                ymd(2025, 2, 28),
            ]
        );
    }

    #[test]
    fn test_working_days_detection_and_rescheduling_modes() {
        let mon_fri_next =
            WorkingDaysConfig::monday_to_friday(RepaymentRescheduleType::MoveToNextWorkingDay);

        // 2025-01-03 is Friday (working), 2025-01-04 is Saturday (non-working), 2025-01-05 is Sunday (non-working)
        assert!(CalendarUtils::is_working_day(&mon_fri_next, ymd(2025, 1, 3)));
        assert!(!CalendarUtils::is_working_day(&mon_fri_next, ymd(2025, 1, 4)));
        assert!(!CalendarUtils::is_working_day(&mon_fri_next, ymd(2025, 1, 5)));
        assert!(CalendarUtils::is_working_day(&mon_fri_next, ymd(2025, 1, 6)));

        // MoveToNextWorkingDay: Saturday Jan 4 -> Monday Jan 6
        assert_eq!(
            CalendarUtils::adjust_date_for_working_days_and_holidays(
                ymd(2025, 1, 4),
                None,
                &mon_fri_next,
                &[],
            ),
            ymd(2025, 1, 6)
        );

        // MoveToPreviousWorkingDay: Sunday Jan 5 -> Friday Jan 3
        let mon_fri_prev =
            WorkingDaysConfig::monday_to_friday(RepaymentRescheduleType::MoveToPreviousWorkingDay);
        assert_eq!(
            CalendarUtils::adjust_date_for_working_days_and_holidays(
                ymd(2025, 1, 5),
                None,
                &mon_fri_prev,
                &[],
            ),
            ymd(2025, 1, 3)
        );

        // SameDay: Saturday Jan 4 stays Saturday Jan 4
        let mon_fri_same =
            WorkingDaysConfig::monday_to_friday(RepaymentRescheduleType::SameDay);
        assert_eq!(
            CalendarUtils::adjust_date_for_working_days_and_holidays(
                ymd(2025, 1, 4),
                None,
                &mon_fri_same,
                &[],
            ),
            ymd(2025, 1, 4)
        );

        // MoveToNextRepaymentMeetingDay: Saturday Jan 4 -> next meeting date Jan 18
        let mon_fri_meeting = WorkingDaysConfig::monday_to_friday(
            RepaymentRescheduleType::MoveToNextRepaymentMeetingDay,
        );
        assert_eq!(
            CalendarUtils::adjust_date_for_working_days_and_holidays(
                ymd(2025, 1, 4),
                Some(ymd(2025, 1, 18)),
                &mon_fri_meeting,
                &[],
            ),
            ymd(2025, 1, 18)
        );
    }

    #[test]
    fn test_holiday_rescheduling_and_chained_working_day_interaction() {
        let working_days =
            WorkingDaysConfig::monday_to_friday(RepaymentRescheduleType::MoveToNextWorkingDay);

        // Case 1: Due date falls on Saturday Jan 4 -> moves to Monday Jan 6 (next working day),
        // which is a Holiday (Jan 6..=Jan 6) rescheduled to specific date Tuesday Jan 7.
        let ep_holiday =
            HolidayRule::to_specific_date(1, ymd(2025, 1, 6), ymd(2025, 1, 6), ymd(2025, 1, 7));
        let adjusted = CalendarUtils::adjust_date_for_working_days_and_holidays(
            ymd(2025, 1, 4),
            Some(ymd(2025, 2, 4)),
            &working_days,
            &[ep_holiday],
        );
        assert_eq!(adjusted, ymd(2025, 1, 7));

        // Case 2: Due date falls on Friday Jan 3, which is a holiday rescheduled to Saturday Jan 4;
        // Saturday Jan 4 is a non-working day, so it chains further to Monday Jan 6!
        let fri_holiday_to_sat =
            HolidayRule::to_specific_date(2, ymd(2025, 1, 3), ymd(2025, 1, 3), ymd(2025, 1, 4));
        let chained = CalendarUtils::adjust_date_for_working_days_and_holidays(
            ymd(2025, 1, 3),
            None,
            &working_days,
            &[fri_holiday_to_sat],
        );
        assert_eq!(chained, ymd(2025, 1, 6));

        // Case 3: Holiday configured with RescheduleToNextRepaymentDate (type = 1)
        let next_rep_holiday =
            HolidayRule::to_next_repayment_date(3, ymd(2025, 1, 15), ymd(2025, 1, 17));
        let moved_to_next_meeting = CalendarUtils::adjust_date_for_working_days_and_holidays(
            ymd(2025, 1, 15),
            Some(ymd(2025, 2, 17)),
            &working_days,
            &[next_rep_holiday],
        );
        assert_eq!(moved_to_next_meeting, ymd(2025, 2, 17));
    }

    #[test]
    fn test_monthly_due_date_resolver_roll_forward_and_unroll() {
        let seed_31 = ymd(2025, 1, 31);
        let feb_clamped = ymd(2025, 2, 28);

        let rolled = CalendarUtils::adjust_date_with_strategy(
            feb_clamped,
            seed_31,
            CalendarFrequencyType::Monthly,
            MonthEndDueDateStrategy::FirstDayOfNextMonth,
        );
        assert_eq!(rolled, ymd(2025, 3, 1));

        let unrolled = CalendarUtils::unroll_monthly_date(
            rolled,
            seed_31,
            CalendarFrequencyType::Monthly,
            MonthEndDueDateStrategy::FirstDayOfNextMonth,
        );
        assert_eq!(unrolled, ymd(2025, 2, 28));
    }
}
