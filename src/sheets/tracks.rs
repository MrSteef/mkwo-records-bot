use crate::sheets::{DataRanges, GSheet};
use anyhow::{Result, anyhow};
use serde_json::Value;

const SHEET_NAME: &str = "Tracks";
const FIRST_COLUMN: &str = "A";
const LAST_COLUMN: &str = "A";

pub struct Tracks<'a> {
    pub gsheet: &'a GSheet,
}

impl DataRanges for Tracks<'_> {
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

impl<'a> Tracks<'a> {
    pub async fn get_all(&self) -> Result<Vec<Track>> {
        let sheets = self.gsheet.sheets.lock().await;

        let tracks: Vec<Track> = sheets
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

        Ok(tracks)
    }
}

#[derive(Debug)]
pub struct Track {
    pub name: String,
}

impl TryFrom<Vec<Value>> for Track {
    type Error = anyhow::Error;

    fn try_from(values: Vec<Value>) -> Result<Self> {
        if values.len() < 1 {
            return Err(anyhow!("Not enough fields to constuct a Track instance"));
        }
        let name = match values.get(0).ok_or(anyhow!("Failed to get first value"))? {
            Value::Null => "",
            Value::String(name) => name,
            _ => {
                return Err(anyhow!("Failed to represent Current Track as a String"));
            }
        }
        .to_string();

        Ok(Track { name })
    }
}

impl Into<Vec<Value>> for Track {
    fn into(self) -> Vec<Value> {
        vec![Value::String(self.name)]
    }
}
