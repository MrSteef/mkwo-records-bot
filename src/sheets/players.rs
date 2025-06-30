use crate::sheets::{DataRanges, GSheet};
use anyhow::{Result, anyhow};
use google_sheets4::api::ValueRange;
use serde_json::Value;

const SHEET_NAME: &str = "Players";
const FIRST_COLUMN: &str = "A";
const LAST_COLUMN: &str = "C";

pub struct Players<'a> {
    pub gsheet: &'a GSheet,
}

impl DataRanges for Players<'_> {
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

impl<'a> Players<'a> {
    pub async fn get_all(&self) -> Result<Vec<Player>> {
        let sheets = self.gsheet.sheets.lock().await;

        let players = sheets
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

        Ok(players)
    }

    pub async fn get_by_id(&self, user_id: u64) -> Result<Option<Player>> {
        let sheets = self.gsheet.sheets.lock().await;

        let player = sheets
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
            .find(|p: &Player| p.user_id == user_id);

        Ok(player)
    }

    pub async fn get_row_by_id(&self, user_id: u64) -> Result<Option<usize>> {
        let sheets = self.gsheet.sheets.lock().await;

        let player_index = sheets
            .spreadsheets()
            .values_get(&self.gsheet.document_id, &Self::table_range())
            .doit()
            .await?
            .1
            .values
            .unwrap_or_default()
            .into_iter()
            .skip(1)
            .map(|row| TryInto::<Player>::try_into(row))
            .enumerate()
            .find(|(_, player)| {
                player
                    .as_ref()
                    .map_or_else(|_| false, |p| p.user_id == user_id)
            });

        let row_num = match player_index {
            None => None,

            // +1 to account for 0-indexed vec vs 1-indexed spreadsheet row numbering
            // +1 to account for header row in the spreadsheet
            Some(index) => Some(index.0 + 2),
        };

        Ok(row_num)
    }

    pub async fn select_track(&self, user_id: u64, track_name: String) -> Result<()> {
        let row_num = self
            .get_row_by_id(user_id)
            .await?
            .ok_or(anyhow!("Could not find player"))?;

        let cell = Self::cell(row_num, "C".to_string()); // Column C holds the current_track
        let values = vec![vec![Value::String(track_name)]];

        let request: ValueRange = ValueRange {
            major_dimension: Some("ROWS".to_string()),
            range: Some(cell.clone()),
            values: Some(values),
        };

        let sheets = self.gsheet.sheets.lock().await;
        sheets
            .spreadsheets()
            .values_update(request, &self.gsheet.document_id, &cell)
            .value_input_option("RAW")
            .doit()
            .await?
            .1;

        Ok(())
    }

    pub async fn create_player_if_not_exists(&self, user_id: u64) -> Result<()> {
        if let Some(_player) = self.get_by_id(user_id).await? {
            return Ok(());
        }

        let values = vec![vec![Value::String(user_id.to_string())]];

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
pub struct Player {
    pub user_id: u64,
    pub display_name: String,
    pub current_track: Option<String>,
}

impl TryFrom<Vec<Value>> for Player {
    type Error = anyhow::Error;

    fn try_from(values: Vec<Value>) -> Result<Self> {
        if values.len() < 1 {
            return Err(anyhow!("Not enough fields to constuct a Player instance"));
        }
        let user_id = match values.get(0).ok_or(anyhow!("Failed to get first value"))? {
            Value::Number(number) => number
                .as_u64()
                .ok_or(anyhow!("Failed to represent User ID as a u64")),
            Value::String(text) => text
                .parse()
                .map_err(|_| anyhow!("Failed to represent User ID as a u64")),
            _ => Err(anyhow!("Failed to represent User ID as a u64")),
        }?;

        let display_name = values.get(1).map_or("".to_string(), |val| val.to_string());

        let current_track = match values.get(2).unwrap_or(&Value::Null) {
            Value::Null => None,
            Value::String(track_name) => Some(track_name.clone()),
            _ => {
                return Err(anyhow!(
                    "Failed to represent Current Track as an Option<String>"
                ));
            }
        };

        Ok(Player {
            user_id,
            display_name,
            current_track,
        })
    }
}

impl Into<Vec<Value>> for Player {
    fn into(self) -> Vec<Value> {
        let user_id = Value::String(self.user_id.to_string());
        let display_name = Value::String(self.display_name);
        let current_track = match self.current_track {
            Some(track_name) => Value::String(track_name),
            None => Value::Null,
        };

        vec![user_id, display_name, current_track]
    }
}
