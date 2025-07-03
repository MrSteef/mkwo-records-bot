use std::time::Duration;

use crate::sheets::{gsheet::GSheet, utils::{duration_to_value, timestamp_to_value}};
use anyhow::{Result, anyhow};
use google_sheets4::api::ValueRange;
use serenity::{all::Timestamp, json::Value};
pub mod record;
use super::utils::DataRanges;
use record::Record;

pub struct Records<'a> {
    gsheet: &'a GSheet,
}

impl DataRanges for Records<'_> {
    const SHEET_NAME: &'static str = "Records";
    const FIRST_COLUMN: &'static str = "A";
    const LAST_COLUMN: &'static str = "F";
}

impl<'a> Records<'a> {
    pub fn new(gsheet: &'a GSheet) -> Self {
        Records { gsheet }
    }
}

impl<'a> Records<'a> {
    pub const USER_MESSAGE_ID_COLUMN: &'static str = "A";
    pub const BOT_MESSAGE_ID_COLUMN: &'static str = "B";
    pub const REPORT_TIMESTAMP_COLUMN: &'static str = "C";
    pub const DRIVER_USER_ID_COLUMN: &'static str = "D";
    pub const TRACK_NAME_COLUMN: &'static str = "E";
    pub const RACE_DURATION_COLUMN: &'static str = "F";

    pub async fn get_all(&self) -> Result<Vec<Record<'a>>> {
        let sheets = self.gsheet.sheets.lock().await;
        let document_id = &self.gsheet.document_id;
        let table_range = &Records::table_range();

        let records: Vec<Record> = sheets
            .spreadsheets()
            .values_get(document_id, table_range)
            .doit()
            .await?
            .1
            .values
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .skip(1)
            .filter_map(|(index, row)| Record::from_row(index + 1, row, self.gsheet).ok())
            .collect();

        Ok(records)
    }

    pub async fn get_by_bot_message_id(&self, bot_message_id: u64) -> Result<Option<Record>> {
        let player_list = self.get_all().await?;
        let player = player_list
            .into_iter()
            .find(|r| r.bot_message_id == bot_message_id);
        Ok(player)
    }

    pub async fn create(
        &self,
        user_message_id: u64,
        bot_message_id: u64,
        report_timestamp: Timestamp,
        driver_user_id: u64,
        track_name: String,
        race_duration: Duration,
    ) -> Result<Record<'a>> {
        let user_message_id_value = Value::String(user_message_id.to_string());
        let bot_message_id_value = Value::String(bot_message_id.to_string());
        let report_timestamp_value = timestamp_to_value(report_timestamp);
        let driver_user_id_value = Value::String(driver_user_id.to_string());
        let track_name_value = Value::String(track_name);
        let race_duration_value = duration_to_value(race_duration);

        let row = vec![
            user_message_id_value,
            bot_message_id_value,
            report_timestamp_value,
            driver_user_id_value,
            track_name_value,
            race_duration_value,
        ];

        let values = vec![row.clone()];

        let request: ValueRange = ValueRange {
            major_dimension: Some("ROWS".to_string()),
            range: Some(Self::table_range()),
            values: Some(values),
        };

        let sheets = self.gsheet.sheets.lock().await;
        let result = sheets
            .spreadsheets()
            .values_append(request, &self.gsheet.document_id, &Self::table_range())
            .value_input_option("RAW")
            .doit()
            .await?
            .1
            .updates
            .ok_or(anyhow!("Failed to obtain Google Sheets return"))?
            .updated_range
            .ok_or(anyhow!("Failed to obtain Google Sheets return"))?;
        let rownum = Records::extract_rows_from_range(&result)
            .ok_or(anyhow!("Failed to determine row number"))?
            .0;
        let record = Record::from_row(rownum, row, self.gsheet);
        
        record
    }
}
