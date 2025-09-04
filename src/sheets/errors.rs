use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum DeserializeValueError {
    #[error("Item at index {missing_index} was missing, expected {expected_item_count} items")]
    MissingItem {
        missing_index: usize,
        expected_item_count: usize,
    },

    #[error("Failed to represent {input_value} as {output_type}")]
    ExtractValue {
        input_value: Value,
        output_type: &'static str,
    },

    #[error(
        "Found an unexpected value while trying to produce a {intended_output}: {input_value}, allowed value type(s) is/are: {allowed_inputs}"
    )]
    UnexpectedValueType {
        input_value: Value,
        allowed_inputs: &'static str,
        intended_output: &'static str,
    },

    #[error("Failed to convert {input} into {output_type}")]
    TypeConversion {
        input: String,
        output_type: &'static str,
    },

    #[error("Failed to format {input} as {output_type}: {message}")]
    InvalidFormat {
        input: String,
        output_type: &'static str,
        message: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SerializeValueError {
    #[error("Failed to turn {input} into a Value: {message}")]
    ParseError { input: String, message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum DataFetchError {
    #[error(transparent)]
    GoogleSheet(#[from] google_sheets4::Error),

    #[error(transparent)]
    DeserializeValue(#[from] DeserializeValueError),
}

#[derive(Debug, thiserror::Error)]
pub enum DataUploadError {
    #[error(transparent)]
    GoogleSheet(#[from] google_sheets4::Error),
    
    #[error(transparent)]
    DataFetchError(#[from] DataFetchError),

    #[error("Google Sheets did not return the expected data")]
    MissingOrUnexpectedResponse,

    #[error("Upload would create a duplicate key")]
    UniqueConstraint,

    #[error(transparent)]
    SerializeValue(#[from] SerializeValueError),

    #[error(transparent)]
    DeserializeValue(#[from] DeserializeValueError),
}
