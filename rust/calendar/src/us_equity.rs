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
        let month = date.month();
        if month > 3 && month < 11 {
            -4 // EDT
        } else if month == 3 {
            let second_sunday = (8..=14).find(|d| NaiveDate::from_ymd_opt(date.year(), 3, *d).map(|d| d.weekday() == Weekday::Sun).unwrap_or(false)).unwrap_or(8);
            if date.day() >= second_sunday { -4 } else { -5 }
        } else if month == 11 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekend_rejection() {
        let cal = UsEquityCalendar::default();
        // 2024-06-08 is Saturday, 2024-06-09 is Sunday
        let sat = NaiveDate::from_ymd_opt(2024, 6, 8).unwrap();
        let sun = NaiveDate::from_ymd_opt(2024, 6, 9).unwrap();
        assert!(!cal.is_trading_day(sat));
        assert!(!cal.is_trading_day(sun));
        assert!(cal.session(sat).is_none());
        assert!(cal.session(sun).is_none());
    }

    #[test]
    fn test_standard_us_holidays() {
        let cal = UsEquityCalendar::default();

        // 2024 MLK Day: Jan 15, 2024 (Monday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
        assert!(!cal.is_trading_day(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));

        // 2024 Presidents' Day: Feb 19, 2024 (Monday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 2, 19).unwrap()));

        // 2024 Good Friday: Mar 29, 2024
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 3, 29).unwrap()));

        // 2024 Memorial Day: May 27, 2024 (Monday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 5, 27).unwrap()));

        // 2024 Juneteenth: Jun 19, 2024 (Wednesday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 6, 19).unwrap()));

        // 2024 Independence Day: Jul 4, 2024 (Thursday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 7, 4).unwrap()));

        // 2024 Labor Day: Sep 2, 2024 (Monday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 9, 2).unwrap()));

        // 2024 Thanksgiving: Nov 28, 2024 (Thursday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 11, 28).unwrap()));

        // 2024 Christmas Day: Dec 25, 2024 (Wednesday)
        assert!(cal.is_holiday(NaiveDate::from_ymd_opt(2024, 12, 25).unwrap()));
    }

    #[test]
    fn test_early_close() {
        let cal = UsEquityCalendar::default();
        // 2024 Black Friday: Nov 29, 2024
        let black_friday = NaiveDate::from_ymd_opt(2024, 11, 29).unwrap();
        assert!(cal.is_trading_day(black_friday));
        assert!(cal.is_early_close(black_friday));

        let sess = cal.session(black_friday).unwrap();
        assert!(sess.is_early_close);
    }

    #[test]
    fn test_next_and_prev_trading_day() {
        let cal = UsEquityCalendar::default();
        // Friday before Memorial Day: 2024-05-24
        let fri = NaiveDate::from_ymd_opt(2024, 5, 24).unwrap();
        // Monday 2024-05-27 is Memorial Day, so next trading day is Tuesday 2024-05-28
        let next = cal.next_trading_day(fri);
        assert_eq!(next, NaiveDate::from_ymd_opt(2024, 5, 28).unwrap());

        // Previous trading day from Tuesday 2024-05-28 should be Friday 2024-05-24
        let prev = cal.prev_trading_day(next);
        assert_eq!(prev, fri);
    }

    #[test]
    fn test_is_market_open_utc() {
        let cal = UsEquityCalendar::default();
        // 2024-06-10 is a normal Monday in EDT (offset -4 hours).
        // 09:30 EDT = 13:30 UTC
        // 16:00 EDT = 20:00 UTC
        let d = NaiveDate::from_ymd_opt(2024, 6, 10).unwrap();
        let sess = cal.session(d).expect("must have session");

        let open_utc = sess.open_utc;
        let close_utc = sess.close_utc;

        assert_eq!(open_utc.time(), NaiveTime::from_hms_opt(13, 30, 0).unwrap());
        assert_eq!(close_utc.time(), NaiveTime::from_hms_opt(20, 0, 0).unwrap());

        // During session
        let mid_day = open_utc + Duration::hours(2);
        assert!(cal.is_market_open(mid_day));

        // Before open
        let before_open = open_utc - Duration::minutes(1);
        assert!(!cal.is_market_open(before_open));

        // After close
        let after_close = close_utc + Duration::minutes(1);
        assert!(!cal.is_market_open(after_close));
    }

    #[test]
    fn test_custom_holidays() {
        let national_day_of_mourning = NaiveDate::from_ymd_opt(2018, 12, 5).unwrap(); // George H.W. Bush mourning
        let cal = UsEquityCalendar::default().with_custom_holiday(national_day_of_mourning);
        assert!(!cal.is_trading_day(national_day_of_mourning));
    }
}
