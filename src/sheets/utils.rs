use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Amsterdam;
use serde_json::{Number, Value};
use serenity::all::Timestamp;

use crate::sheets::errors::{DeserializeValueError, SerializeValueError};

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

pub fn get_u64(value: &Value) -> Result<u64, DeserializeValueError> {
    match value {
        Value::Number(number) => number.as_u64().ok_or(DeserializeValueError::ExtractValue {
            input_value: value.clone(),
            output_type: "u64",
        }),
        Value::String(text) => text
            .parse()
            .map_err(|_| DeserializeValueError::TypeConversion {
                input: text.to_string(),
                output_type: "u64",
            }),
        val => Err(DeserializeValueError::UnexpectedValueType {
            input_value: val.clone(),
            allowed_inputs: "Number, String",
            intended_output: "u64",
        }),
    }
}

pub fn get_string(value: &Value) -> Result<String, DeserializeValueError> {
    match value {
        Value::String(name) => Ok(name.to_owned()),
        val => Err(DeserializeValueError::UnexpectedValueType {
            input_value: val.clone(),
            allowed_inputs: "String",
            intended_output: "String",
        }),
    }
}

pub fn get_timestamp(value: &Value) -> Result<Timestamp, DeserializeValueError> {
    match value {
        Value::String(s) => {
            let naive = NaiveDateTime::parse_from_str(s, "%d-%m-%Y %H:%M:%S").map_err(|_| {
                DeserializeValueError::ExtractValue {
                    input_value: value.clone(),
                    output_type: "Timestamp",
                }
            })?;
            let datetime: DateTime<Utc> = TimeZone::from_utc_datetime(&Utc, &naive);
            Ok(Timestamp::from(datetime))
        }
        Value::Number(n) => {
            let serial_days = n.as_f64().ok_or(DeserializeValueError::ExtractValue {
                input_value: value.clone(),
                output_type: "f64",
            })?;
            let unix_seconds = (serial_days - SHEETS_EPOCH_UNIX_DAYS) * SECS_PER_DAY;
            let datetime = DateTime::from_timestamp(unix_seconds as i64, 0).ok_or_else(|| {
                DeserializeValueError::TypeConversion {
                    input: unix_seconds.to_string(),
                    output_type: "DateTime",
                }
            })?;
            // let datetime_utc: DateTime<Utc> = DateTime::from_utc(datetime, Utc);
            Ok(Timestamp::from(datetime))
        }
        val => Err(DeserializeValueError::UnexpectedValueType {
            input_value: val.clone(),
            allowed_inputs: "String, Number",
            intended_output: "Timestamp",
        }),
    }
}

pub fn get_duration(value: &Value) -> Result<Duration, DeserializeValueError> {
    match value {
        Value::Number(number) => {
            let time = number.as_f64().ok_or(DeserializeValueError::ExtractValue {
                input_value: value.clone(),
                output_type: "f64",
            })?;
            let seconds = time * SECS_PER_DAY;
            Ok(Duration::from_secs_f64(seconds))
        }
        Value::String(string) => {
            let parts: Vec<&str> = string.split(':').collect();
            if parts.len() != 2 {
                return Err(DeserializeValueError::InvalidFormat {
                    input: string.clone(),
                    output_type: "Duration",
                    message: "String must contain exactly one colon, between the minutes and seconds place".to_owned(),
                });
            }

            let minutes: u64 =
                parts[0]
                    .parse()
                    .map_err(|_| DeserializeValueError::InvalidFormat {
                        input: parts[0].to_owned(),
                        output_type: "u64",
                        message: "Minutes part must represent a valid number".to_owned(),
                    })?;

            let sec_parts: Vec<&str> = parts[1].split('.').collect();
            if sec_parts.len() != 2 {
                return Err(DeserializeValueError::InvalidFormat {
                    input: string.clone(),
                    output_type: "Duration",
                    message: "String must contain exactly one period, between the seconds and milliseconds place".to_owned(),
                });
            }

            let seconds: u64 =
                sec_parts[0]
                    .parse()
                    .map_err(|_| DeserializeValueError::InvalidFormat {
                        input: parts[0].to_owned(),
                        output_type: "u64",
                        message: "Seconds part must represent a valid number".to_owned(),
                    })?;

            let millis: u64 = if sec_parts.len() > 1 {
                sec_parts[1]
                    .parse::<u64>()
                    .map_err(|_| DeserializeValueError::InvalidFormat {
                        input: parts[0].to_owned(),
                        output_type: "u64",
                        message: "Milliseconds part must represent a valid number".to_owned(),
                    })?
            } else {
                0
            };

            Ok(Duration::from_secs(minutes * 60 + seconds) + Duration::from_millis(millis))
        }
        _ => Err(DeserializeValueError::UnexpectedValueType {
            input_value: value.clone(),
            allowed_inputs: "Number, String",
            intended_output: "Duration",
        }),
    }
}

const SHEETS_EPOCH_UNIX_DAYS: f64 = 25_569.0;
const SECS_PER_DAY: f64 = 86_400.0;

pub fn timestamp_to_value(timestamp: Timestamp) -> Result<Value, SerializeValueError> {
    let dt_am = timestamp.with_timezone(&Amsterdam);
    let naive_local = dt_am.naive_local();
    let local_secs = naive_local.and_utc().timestamp() as f64;
    let serial_days = local_secs / SECS_PER_DAY + SHEETS_EPOCH_UNIX_DAYS;
    let number = Number::from_f64(serial_days).ok_or(SerializeValueError::ParseError {
        input: serial_days.to_string(),
        message: "Number may not be NaN or Infinite".to_owned(),
    })?;
    Ok(Value::Number(number))
}

pub fn duration_to_value(duration: Duration) -> Result<Value, SerializeValueError> {
    let serial_days = duration.as_secs_f64() / SECS_PER_DAY;
    let number = Number::from_f64(serial_days).ok_or(SerializeValueError::ParseError {
        input: serial_days.to_string(),
        message: "Number may not be NaN or Infinite".to_owned(),
    })?;
    Ok(Value::Number(number))
}
