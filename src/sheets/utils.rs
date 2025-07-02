use std::time::Duration;

use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Amsterdam;
use serde_json::{Number, Value};
use serenity::all::Timestamp;

pub trait DataRanges {
    const SHEET_NAME: &'static str;
    const FIRST_COLUMN: &'static str;
    const LAST_COLUMN: &'static str;

    fn table_range() -> String {
        format!(
            "{}!{}:{}",
            Self::SHEET_NAME,
            Self::FIRST_COLUMN,
            Self::LAST_COLUMN
        )
    }

    fn row_range(row: usize) -> String {
        format!(
            "{}!{}{}:{}{}",
            Self::SHEET_NAME,
            Self::FIRST_COLUMN,
            row,
            Self::LAST_COLUMN,
            row
        )
    }

    fn rows_range(from: usize, to: usize) -> String {
        format!(
            "{}!{}{}:{}{}",
            Self::SHEET_NAME,
            Self::FIRST_COLUMN,
            from,
            Self::LAST_COLUMN,
            to
        )
    }

    fn cell_range(row: usize, col: &str) -> String {
        format!("{}!{}{}:{}{}", Self::SHEET_NAME, col, row, col, row)
    }

    fn extract_rows_from_range(range: &str) -> Option<(usize, usize)> {
        let pattern = regex::Regex::new(r"^[^!]+![A-Z]+(\d+):[A-Z]+(\d+)$").ok()?;
        let captures = pattern.captures(range)?;
        let start = captures.get(1)?.as_str().parse::<usize>().ok()?;
        let end = captures.get(2)?.as_str().parse::<usize>().ok()?;
        Some((start, end))
    }
}

pub fn get_u64(value: &Value) -> Result<u64> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .ok_or(anyhow!("Failed to represent User ID as a u64")),
        Value::String(text) => text
            .parse()
            .map_err(|_| anyhow!("Failed to represent User ID as a u64")),
        _ => Err(anyhow!("Failed to represent User ID as a u64")),
    }
}

pub fn get_string(value: &Value) -> Result<String> {
    match value {
        Value::String(name) => Ok(name.to_owned()),
        _ => Err(anyhow!("Failed to represent display name as a String")),
    }
}

pub fn get_timestamp(value: &Value) -> Result<Timestamp> {
    let s = value
        .as_str()
        .ok_or(anyhow!("Failed to represent value as a String"))?;
    let naive = NaiveDateTime::parse_from_str(s, "%d-%m-%Y %H:%M:%S")?;
    let datetime: DateTime<Utc> = TimeZone::from_utc_datetime(&Utc, &naive);
    Ok(Timestamp::from(datetime))
}

pub fn get_duration(value: &Value) -> Result<Duration> {
    match value {
        Value::Number(number) => Ok(Duration::from_secs(
            number
                .as_u64()
                .ok_or(anyhow!("Failed to represent value as a u64"))?,
        )),
        Value::String(string) => {
            let parts: Vec<&str> = string.split(':').collect();
            if parts.len() != 2 {
                panic!("Invalid duration string format: {:?}", string);
            }

            let minutes: u64 = parts[0].parse().expect("Invalid minutes");
            let sec_parts: Vec<&str> = parts[1].split('.').collect();

            let seconds: u64 = sec_parts[0].parse().expect("Invalid seconds");
            let millis: u64 = if sec_parts.len() > 1 {
                sec_parts[1].parse::<u64>().expect("Invalid milliseconds")
            } else {
                0
            };

            Ok(Duration::from_secs(minutes * 60 + seconds) + Duration::from_millis(millis))
        }
        _ => Err(anyhow!("Failed to represent value as a Duration")),
    }
}

const SHEETS_EPOCH_UNIX_DAYS: f64 = 25_569.0;
const SECS_PER_DAY: f64 = 86_400.0;

pub fn timestamp_to_value(timestamp: Timestamp) -> Value {
    let dt_am = timestamp.with_timezone(&Amsterdam);
    let naive_local = dt_am.naive_local();
    let local_secs = naive_local.and_utc().timestamp() as f64;
    let serial_days = local_secs / SECS_PER_DAY + SHEETS_EPOCH_UNIX_DAYS;
    Value::Number(
        Number::from_f64(serial_days).expect("timestamp_to_value: serial_days must be finite"),
    )
}

pub fn duration_to_value(duration: Duration) -> Value {
    let serial_days = duration.as_secs_f64() / SECS_PER_DAY;
    Value::Number(
        Number::from_f64(serial_days).expect("duration_to_value: serial_days must be finite"),
    )
}