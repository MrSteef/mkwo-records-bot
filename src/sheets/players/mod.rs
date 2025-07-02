use crate::sheets::gsheet::GSheet;
use anyhow::{Result, anyhow};
use google_sheets4::api::ValueRange;
use serde_json::Value;
mod player;
use super::utils::DataRanges;

use player::Player;

pub struct Players<'a> {
    gsheet: &'a GSheet,
}

impl DataRanges for Players<'_> {
    const SHEET_NAME: &'static str = "Players";
    const FIRST_COLUMN: &'static str = "A";
    const LAST_COLUMN: &'static str = "C";
}

impl<'a> Players<'a> {
    pub fn new(gsheet: &'a GSheet) -> Self {
        Players { gsheet }
    }
}

impl Players<'_> {
    pub const USER_ID_COLUMN: &'static str = "A";
    pub const DISPLAY_NAME_COLUMN: &'static str = "B";
    pub const CURRENT_TRACK_COLUMN: &'static str = "C";

    pub async fn get_all(&self) -> Result<Vec<Player>> {
        let sheets = self.gsheet.sheets.lock().await;
        let document_id = &self.gsheet.document_id;
        let table_range = &Players::table_range();

        let players: Vec<Player> = sheets
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
            .filter_map(|(index, row)| Player::from_row(index + 1, row, self.gsheet).ok())
            .collect();

        Ok(players)
    }

    pub async fn get_by_user_id(&self, user_id: u64) -> Result<Option<Player>> {
        let player_list = self.get_all().await?;
        let player = player_list
            .into_iter()
            .find(|p| p.user_id == user_id);
        Ok(player)
    }

    pub async fn create(&self, user_id: u64, display_name: impl Into<String>, track_name: Option<String>) -> Result<Player> {
        if let Some(_) = self.get_by_user_id(user_id).await? {
            return Err(anyhow!("Player already exists"));
        }

        let display_name: String = display_name.into();

        let row = vec![
            Value::String(user_id.to_string()),
            Value::String(display_name),
            Value::String(track_name.unwrap_or_default()),
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
        let rownum = Players::extract_rows_from_range(&result)
            .ok_or(anyhow!("Failed to determine row number"))?
            .0;
        let player = Player::from_row(rownum, row, self.gsheet);
        
        player
    }
}
