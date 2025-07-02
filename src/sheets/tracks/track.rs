use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::sheets::gsheet::GSheet;

#[derive(Debug)]
pub struct Track<'a> {
    _gsheet: &'a GSheet,
    _rownum: usize,
    pub name: String,
    pub icon_url: String,
}

impl<'a> Track<'a> {
    pub fn from_row(rownum: usize, values: Vec<Value>, gsheet: &'a GSheet) -> Result<Self> {
        let name = match values.get(0).ok_or(anyhow!("Failed to get name value"))? {
            Value::String(name) => name,
            _ => {
                return Err(anyhow!("Failed to represent track as a String"));
            }
        }
        .to_owned();

        let icon_url = match values
            .get(1)
            .ok_or(anyhow!("Failed to get icon url value"))?
        {
            Value::String(icon_url) => icon_url,
            _ => {
                return Err(anyhow!("Failed to represent icon url as a String"));
            }
        }
        .to_owned();

        Ok({
            Track {
                _gsheet: gsheet,
                _rownum: rownum,
                name,
                icon_url,
            }
        })
    }
}

impl Into<Vec<Value>> for Track<'_> {
    fn into(self) -> Vec<Value> {
        vec![Value::String(self.name), Value::String(self.icon_url)]
    }
}
