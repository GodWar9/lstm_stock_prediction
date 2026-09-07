//! US Equity Calendar implementation for NYSE / NASDAQ exchanges.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc, Weekday};
use std::collections::HashSet;
use crate::{Session, TradingCalendar};

/// Calendar representing NYSE / NASDAQ market schedules, rules, and holidays.
#[derive(Debug, Clone)]
pub struct UsEquityCalendar {
    exchange: String,
    custom_holidays: HashSet<NaiveDate>,
}

impl Default for UsEquityCalendar {
    fn default() -> Self {
        Self::new("NYSE")
    }
}

impl UsEquityCalendar {
    pub fn new(exchange: impl Into<String>) -> Self {
        Self {
            exchange: exchange.into(),
            custom_holidays: HashSet::new(),
        }
    }

    /// Adds custom holiday dates (for one-off market closures, national days of mourning, etc.).
    pub fn with_custom_holiday(mut self, date: NaiveDate) -> Self {
        self.custom_holidays.insert(date);
        self
    }

    /// Checks whether a date is a recognized US federal/market holiday.
    pub fn is_holiday(&self, date: NaiveDate) -> bool {
        if self.custom_holidays.contains(&date) {
            return true;
        }

        let year = date.year();
        let month = date.month();
        let day = date.day();
        let weekday = date.weekday();

        // 1. New Year's Day (Jan 1) with weekend observance rule
        if (month == 1 && day == 1 && weekday != Weekday::Sat && weekday != Weekday::Sun)
            || (month == 1 && day == 2 && weekday == Weekday::Mon)
            || (month == 12 && day == 31 && weekday == Weekday::Fri)
        {
            return true;
        }

        // 2. Martin Luther King Jr. Day (Third Monday of January)
        if month == 1 && weekday == Weekday::Mon && (15..=21).contains(&day) {
            return true;
        }

        // 3. Washington's Birthday / Presidents' Day (Third Monday of February)
        if month == 2 && weekday == Weekday::Mon && (15..=21).contains(&day) {
            return true;
        }

        // 4. Good Friday (Easter - 2 days)
        if let Some(good_friday) = easter_date(year).map(|e| e - Duration::days(2)) {
            if date == good_friday {
                return true;
            }
        }

        // 5. Memorial Day (Last Monday of May)
        if month == 5 && weekday == Weekday::Mon && day >= 25 {
            return true;
        }

        // 6. Juneteenth National Independence Day (June 19, observed since 2021)
        if year >= 2021 {
            if (month == 6 && day == 19 && weekday != Weekday::Sat && weekday != Weekday::Sun)
                || (month == 6 && day == 20 && weekday == Weekday::Mon)
                || (month == 6 && day == 18 && weekday == Weekday::Fri)
            {
                return true;
            }
        }

        // 7. Independence Day (July 4)
        if (month == 7 && day == 4 && weekday != Weekday::Sat && weekday != Weekday::Sun)
            || (month == 7 && day == 5 && weekday == Weekday::Mon)
            || (month == 7 && day == 3 && weekday == Weekday::Fri)
        {
            return true;
        }

        // 8. Labor Day (First Monday of September)
        if month == 9 && weekday == Weekday::Mon && day <= 7 {
            return true;
        }

        // 9. Thanksgiving Day (Fourth Thursday of November)
        if month == 11 && weekday == Weekday::Thu && (22..=28).contains(&day) {
            return true;
        }

        // 10. Christmas Day (Dec 25)
        if (month == 12 && day == 25 && weekday != Weekday::Sat && weekday != Weekday::Sun)
            || (month == 12 && day == 26 && weekday == Weekday::Mon)
            || (month == 12 && day == 24 && weekday == Weekday::Fri)
        {
            return true;
        }

        false
    }

    /// Checks whether a date is scheduled for an early close (13:00 Eastern).
    pub fn is_early_close(&self, date: NaiveDate) -> bool {
        let month = date.month();
        let day = date.day();
        let weekday = date.weekday();

        // Day after Thanksgiving (Black Friday)
        if month == 11 && weekday == Weekday::Fri && (23..=29).contains(&day) {
            return true;
        }

        // Christmas Eve (Dec 24) when it falls on a weekday and is not a market holiday
        if month == 12 && day == 24 && weekday != Weekday::Sat && weekday != Weekday::Sun {
            return true;
        }

        // July 3rd when July 4th falls on a weekday
        if month == 7 && day == 3 && weekday != Weekday::Sat && weekday != Weekday::Sun && weekday != Weekday::Fri {
            return true;
        }

        false
    }

    /// Rough UTC offset for US Eastern Time (simplifying Daylight Saving for session boundaries: UTC-4 for summer, UTC-5 for winter).
    fn eastern_offset_hours(date: NaiveDate) -> i64 {
        // DST in US: starts 2nd Sunday in March, ends 1st Sunday in November
        let month = date.month();
        if month > 3 && month < 11 {
            -4 // EDT
        } else if month == 3 {
            // Check second Sunday
            let second_sunday = (8..=14).find(|d| NaiveDate::from_ymd_opt(date.year(), 3, *d).map(|d| d.weekday() == Weekday::Sun).unwrap_or(false)).unwrap_or(8);
            if date.day() >= second_sunday { -4 } else { -5 }
        } else if month == 11 {
            // Check first Sunday
            let first_sunday = (1..=7).find(|d| NaiveDate::from_ymd_opt(date.year(), 11, *d).map(|d| d.weekday() == Weekday::Sun).unwrap_or(false)).unwrap_or(1);
            if date.day() < first_sunday { -4 } else { -5 }
        } else {
            -5 // EST
        }
    }
}

impl TradingCalendar for UsEquityCalendar {
    fn exchange(&self) -> &str {
        &self.exchange
    }

    fn is_trading_day(&self, date: NaiveDate) -> bool {
        let wd = date.weekday();
        if wd == Weekday::Sat || wd == Weekday::Sun {
            return false;
        }
        !self.is_holiday(date)
    }

    fn session(&self, date: NaiveDate) -> Option<Session> {
        if !self.is_trading_day(date) {
            return None;
        }

        let is_early = self.is_early_close(date);
        let offset = Self::eastern_offset_hours(date);

        let open_time = NaiveTime::from_hms_opt(9, 30, 0)?;
        let close_time = if is_early {
            NaiveTime::from_hms_opt(13, 0, 0)?
        } else {
            NaiveTime::from_hms_opt(16, 0, 0)?
        };

        let open_dt = NaiveDateTime::new(date, open_time);
        let close_dt = NaiveDateTime::new(date, close_time);

        // Convert Eastern to UTC: UTC = Local - Offset (offset is -4 or -5, so - offset is +4 or +5)
        let open_utc = Utc.from_utc_datetime(&(open_dt - Duration::hours(offset)));
        let close_utc = Utc.from_utc_datetime(&(close_dt - Duration::hours(offset)));

        Some(Session::new(date, open_utc, close_utc, is_early))
    }

    fn next_trading_day(&self, mut date: NaiveDate) -> NaiveDate {
        loop {
            date += Duration::days(1);
            if self.is_trading_day(date) {
                return date;
            }
        }
    }

    fn prev_trading_day(&self, mut date: NaiveDate) -> NaiveDate {
        loop {
            date -= Duration::days(1);
            if self.is_trading_day(date) {
                return date;
            }
        }
    }

    fn is_market_open(&self, dt: DateTime<Utc>) -> bool {
        let date = dt.date_naive();
        if let Some(sess) = self.session(date) {
            sess.contains(dt)
        } else {
            false
        }
    }
}

/// Anonymous Gregorian Easter algorithm.
fn easter_date(year: i32) -> Option<NaiveDate> {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;

    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}
