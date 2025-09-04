use std::time::Duration;

use serde_json::Value;
use serenity::all::Timestamp;

use crate::sheets::{
    errors::{DataUploadError, DeserializeValueError},
    gsheet::GSheet,
    records::Records,
    utils::{
        duration_to_value, get_duration, get_string, get_timestamp, get_u64, timestamp_to_value, DataRanges
    },
};

#[derive(Debug)]
pub struct Record<'a> {
    gsheet: &'a GSheet,
    rownum: usize,
    pub user_message_id: u64,
    pub bot_message_id: u64,
    pub report_timestamp: Timestamp,
    pub driver_user_id: u64,
    pub track_name: String,
    pub race_duration: Duration,
}

impl<'a> Record<'a> {
    pub fn from_row(
        rownum: usize,
        values: Vec<Value>,
        gsheet: &'a GSheet,
    ) -> Result<Self, DeserializeValueError> {
        let user_message_id_value = values.get(0).ok_or(DeserializeValueError::MissingItem {
            missing_index: 0,
            expected_item_count: 6,
        })?;
        let bot_message_id_value = values.get(1).ok_or(DeserializeValueError::MissingItem {
            missing_index: 1,
            expected_item_count: 6,
        })?;
        let report_timestamp_value = values.get(2).ok_or(DeserializeValueError::MissingItem {
            missing_index: 2,
            expected_item_count: 6,
        })?;
        let driver_user_id_value = values.get(3).ok_or(DeserializeValueError::MissingItem {
            missing_index: 3,
            expected_item_count: 6,
        })?;
        let track_name_value = values.get(4).ok_or(DeserializeValueError::MissingItem {
            missing_index: 4,
            expected_item_count: 6,
        })?;
        let race_duration_value = values.get(5).ok_or(DeserializeValueError::MissingItem {
            missing_index: 5,
            expected_item_count: 6,
        })?;

        let user_message_id = get_u64(user_message_id_value)?;
        let bot_message_id = get_u64(bot_message_id_value)?;
        let report_timestamp = get_timestamp(report_timestamp_value)?;
        let driver_user_id = get_u64(driver_user_id_value)?;
        let track_name = get_string(track_name_value)?;
        let race_duration = get_duration(race_duration_value)?;

        Ok({
            Record {
                gsheet,
                rownum,
                user_message_id,
                bot_message_id,
                report_timestamp,
                driver_user_id,
                track_name,
                race_duration,
            }
        })
    }
}

impl Record<'_> {
    pub async fn set_driver_user_id(&mut self, user_id: u64) -> Result<(), DataUploadError> {
        let cell = Records::cell_range(self.rownum, Records::DRIVER_USER_ID_COLUMN);
        let value = Value::String(user_id.to_string());
        self.gsheet.write_cell(cell, value).await?;
        self.driver_user_id = user_id;
        Ok(())
    }

    pub async fn set_track_name(&mut self, track_name: String) -> Result<(), DataUploadError> {
        let cell = Records::cell_range(self.rownum, Records::TRACK_NAME_COLUMN);
        let value = Value::String(track_name.clone());
        self.gsheet.write_cell(cell, value).await?;
        self.track_name = track_name;
        Ok(())
    }

    pub async fn set_race_duration(&mut self, race_duration: Duration) -> Result<(), DataUploadError> {
        let cell = Records::cell_range(self.rownum, Records::RACE_DURATION_COLUMN);
        let value = duration_to_value(race_duration).unwrap(); // TODO: handle this unwrap properly
        self.gsheet.write_cell(cell, value).await?;
        self.race_duration = race_duration;
        Ok(())
    }
}

impl<'a> Into<Vec<Value>> for Record<'a> {
    fn into(self) -> Vec<Value> {
        let user_message_id = Value::String(self.user_message_id.to_string());
        let bot_message_id = Value::String(self.bot_message_id.to_string());
        let report_timestamp = timestamp_to_value(self.report_timestamp).unwrap(); // TODO: handle this unwrap properly
        let driver_user_id = Value::String(self.driver_user_id.to_string());
        let track_name = Value::String(self.track_name);
        let race_duration = duration_to_value(self.race_duration).unwrap(); // TODO: handle this unwrap properly

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
