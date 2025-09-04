use serde_json::Value;

use crate::sheets::{errors::DeserializeValueError, gsheet::GSheet};

#[derive(Debug)]
pub struct Track<'a> {
    _gsheet: &'a GSheet,
    _rownum: usize,
    pub name: String,
    pub icon_url: String,
}

impl<'a> Track<'a> {
    pub fn from_row(
        rownum: usize,
        values: Vec<Value>,
        gsheet: &'a GSheet,
    ) -> Result<Self, DeserializeValueError> {
        let name = match values.get(0).ok_or(DeserializeValueError::MissingItem {
            missing_index: 0,
            expected_item_count: 2,
        })? {
            Value::String(name) => name,
            val => {
                return Err(DeserializeValueError::UnexpectedValueType {
                    input_value: val.clone(),
                    allowed_inputs: "String",
                    intended_output: "String",
                });
            }
        }
        .to_owned();

        let icon_url = match values
            .get(1)
            .ok_or(DeserializeValueError::MissingItem {
            missing_index: 1,
            expected_item_count: 2,
        })?
        {
            Value::String(icon_url) => icon_url,
            val => {
                return Err(DeserializeValueError::UnexpectedValueType {
                    input_value: val.clone(),
                    allowed_inputs: "String",
                    intended_output: "String",
                });
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
