use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::sheets::{
    gsheet::GSheet,
    players::Players,
    utils::{DataRanges, get_string, get_u64},
};

#[derive(Debug)]
pub struct Player<'a> {
    gsheet: &'a GSheet,
    rownum: usize,
    pub user_id: u64,
    pub display_name: String,
    pub current_track: Option<String>,
}

impl<'a> Player<'a> {
    pub fn from_row(rownum: usize, values: Vec<Value>, gsheet: &'a GSheet) -> Result<Self> {
        let user_id_value = values.get(0).ok_or(anyhow!("Failed to get first value"))?;
        let user_id = get_u64(user_id_value)?;

        let display_name_value = values
            .get(1)
            .ok_or(anyhow!("Failed to get display name value"))?;
        let display_name = get_string(display_name_value)?;

        let current_track_value = values.get(2).unwrap_or(&Value::Null);
        let current_track = get_string(current_track_value).ok();

        Ok({
            Player {
                gsheet,
                rownum,
                user_id,
                display_name,
                current_track,
            }
        })
    }
}

impl Player<'_> {
    pub async fn set_display_name(&mut self, display_name: String) -> Result<()> {
        let cell = Players::cell_range(self.rownum, Players::DISPLAY_NAME_COLUMN);
        let value = Value::String(display_name.clone());
        self.gsheet.write_cell(cell, value).await?;
        self.display_name = display_name;
        Ok(())
    }

    pub async fn set_current_track(&mut self, track_name: String) -> Result<()> {
        let cell = Players::cell_range(self.rownum, Players::CURRENT_TRACK_COLUMN);
        let value = Value::String(track_name.clone());
        self.gsheet.write_cell(cell, value).await?;
        self.current_track = Some(track_name);
        Ok(())
    }
}

impl<'a> Into<Vec<Value>> for Player<'a> {
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
