use crate::sheets::gsheet::GSheet;
use anyhow::Result;
mod record;
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

impl Records<'_> {
    pub const USER_MESSAGE_ID_COLUMN: &'static str = "A";
    pub const BOT_MESSAGE_ID_COLUMN: &'static str = "B";
    pub const REPORT_TIMESTAMP_COLUMN: &'static str = "C";
    pub const DRIVER_USER_ID_COLUMN: &'static str = "D";
    pub const TRACK_NAME_COLUMN: &'static str = "E";
    pub const RACE_DURATION_COLUMN: &'static str = "F";

    pub async fn get_all(&self) -> Result<Vec<Record>> {
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
}
