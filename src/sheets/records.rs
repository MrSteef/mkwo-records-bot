use anyhow::{Result, anyhow};
use google_sheets4::api::ValueRange;
use serde_json::Number;
use std::time::Duration;
use chrono_tz::Europe::Amsterdam;
use chrono::{TimeZone};

use serenity::{all::Timestamp, json::Value};

use crate::sheets::{DataRanges, GSheet};

const SHEET_NAME: &str = "Records";
const FIRST_COLUMN: &str = "A";
const LAST_COLUMN: &str = "F";

pub struct Records<'a> {
    pub gsheet: &'a GSheet,
}

impl DataRanges for Records<'_> {
    fn sheet_name() -> &'static str {
        SHEET_NAME
    }

    fn first_column() -> &'static str {
        FIRST_COLUMN
    }

    fn last_column() -> &'static str {
        LAST_COLUMN
    }
}

impl<'a> Records<'a> {
    pub async fn get_all(&self) -> Result<Vec<Record>> {
        let sheets = self.gsheet.sheets.lock().await;

        let records = sheets
            .spreadsheets()
            .values_get(&self.gsheet.document_id, &Self::table_range())
            .doit()
            .await?
            .1
            .values
            .unwrap_or_default()
            .into_iter()
            .skip(1)
            .filter_map(|row| row.try_into().ok())
            .collect();

        Ok(records)
    }

    // get_by_user_msg_id
    // get_row_by_user_msg_id
    // get_by_bot_msg_id
    // get_row_by_bot_msg_id

    pub async fn create_record(
        &self,
        user_message_id: u64,
        bot_message_id: u64,
        report_timestamp: Timestamp,
        driver_user_id: u64,
        track_name: String,
        race_duration: Duration,
    ) -> Result<()> {
        let record = Record {
            user_message_id,
            bot_message_id,
            report_timestamp,
            driver_user_id,
            track_name,
            race_duration,
        };

        let values = vec![record.into()];

        let request: ValueRange = ValueRange {
            major_dimension: Some("ROWS".to_string()),
            range: Some(Self::table_range()),
            values: Some(values),
        };

        let sheets = self.gsheet.sheets.lock().await;
        sheets
            .spreadsheets()
            .values_append(request, &self.gsheet.document_id, &Self::table_range())
            .value_input_option("RAW")
            .doit()
            .await?
            .1;

        Ok(())
    }
}

#[derive(Debug)]
pub struct Record {
    pub user_message_id: u64,
    pub bot_message_id: u64,
    pub report_timestamp: Timestamp,
    pub driver_user_id: u64,
    pub track_name: String,
    pub race_duration: Duration,
}

// impl TryFrom<Vec<Value>> for Record {
//     type Error = anyhow::Error;

// fn try_from(values: Vec<Value>) -> Result<Self> {
//     if values.len() < 6 {
//         return Err(anyhow!("Not enough field to construct a Record instance"));
//     }

//     let user_message_id = match values.get(0).ok_or(anyhow!("Failed to get first value"))? {
//         Value::Number(number) => number
//             .as_u64()
//             .ok_or(anyhow!("Failed to represent User Message ID as a u64")),
//         Value::String(text) => text
//             .parse()
//             .map_err(|_| anyhow!("Failed to represent User Message ID as a u64")),
//         _ => Err(anyhow!("Failed to represent User Message ID as a u64")),
//     }?;

//     let bot_message_id = match values.get(0).ok_or(anyhow!("Failed to get second value"))? {
//         Value::Number(number) => number
//             .as_u64()
//             .ok_or(anyhow!("Failed to represent Bot Message ID as a u64")),
//         Value::String(text) => text
//             .parse()
//             .map_err(|_| anyhow!("Failed to represent Bot Message ID as a u64")),
//         _ => Err(anyhow!("Failed to represent Bot Message ID as a u64")),
//     }?;

//     let
// }
//

impl TryFrom<Vec<Value>> for Record {
    type Error = anyhow::Error;

    fn try_from(values: Vec<Value>) -> Result<Self> {
        if values.len() != 6 {
            return Err(anyhow!("Expected 6 values, got {}", values.len()));
        }

        let user_message_id = values[0]
            .as_u64()
            .ok_or_else(|| anyhow!("Invalid user_message_id"))?;
        let bot_message_id = values[1]
            .as_u64()
            .ok_or_else(|| anyhow!("Invalid bot_message_id"))?;
        let report_timestamp = value_to_timestamp(values[2].clone());
        let driver_user_id = values[3]
            .as_u64()
            .ok_or_else(|| anyhow!("Invalid driver_user_id"))?;
        let track_name = values[4]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid track_name"))?
            .to_string();
        let race_duration = value_to_duration(values[5].clone());

        Ok(Record {
            user_message_id,
            bot_message_id,
            report_timestamp,
            driver_user_id,
            track_name,
            race_duration,
        })
    }
}

impl Into<Vec<Value>> for Record {
    fn into(self) -> Vec<Value> {
        let user_message_id = Value::String(self.user_message_id.to_string());
        let bot_message_id = Value::String(self.bot_message_id.to_string());
        let report_timestamp = timestamp_to_value(self.report_timestamp);
        let driver_user_id = Value::String(self.driver_user_id.to_string());
        let track_name = Value::String(self.track_name);
        let race_duration = duration_to_value(self.race_duration);

        vec![
            user_message_id,
            bot_message_id,
            report_timestamp,
            driver_user_id,
            track_name,
            race_duration,
        ]
    }
}

/// Google-Sheets “zero” is 1899-12-30, which is 25 569 days before UNIX epoch.
const SHEETS_EPOCH_UNIX_DAYS: f64 = 25_569.0;
const SECS_PER_DAY:       f64 = 86_400.0;

fn timestamp_to_value(timestamp: Timestamp) -> Value {
    // 1) interpret the UTC instant in Amsterdam time
    let dt_am = timestamp.with_timezone(&Amsterdam);
    // 2) strip off its timezone to get the local wall‐clock NaiveDateTime
    let naive_local = dt_am.naive_local();
    // 3) treat that naive as UTC to get “local seconds since UNIX”
    let local_secs = naive_local.and_utc().timestamp() as f64;
    // 4) turn into days since 1899-12-30
    let serial_days = local_secs / SECS_PER_DAY + SHEETS_EPOCH_UNIX_DAYS;
    // 5) emit as JSON number
    Value::Number(
        Number::from_f64(serial_days)
            .expect("timestamp_to_value: serial_days must be finite"),
    )
}

fn duration_to_value(duration: Duration) -> Value {
    // Durations in Sheets are also fractional days
    let serial_days = duration.as_secs_f64() / SECS_PER_DAY;
    Value::Number(
        Number::from_f64(serial_days)
            .expect("duration_to_value: serial_days must be finite"),
    )
}

fn value_to_timestamp(value: Value) -> Timestamp {
    // Expect the JSON value to be a string in RFC3339 format
    if let Some(s) = value.as_str() {
        Timestamp::parse(s)
            .expect("Failed to parse RFC3339 timestamp string")
    } else {
        panic!("Expected JSON string for timestamp, got {:?}", value);
    }
}

fn value_to_duration(value: Value) -> Duration {
    // Expect the JSON value to be a u64 number of seconds
    if let Some(secs) = value.as_u64() {
        Duration::from_secs(secs)
    } else {
        panic!("Expected JSON number for duration, got {:?}", value);
    }
}