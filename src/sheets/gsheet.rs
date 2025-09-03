use google_sheets4::{
    Sheets,
    api::ValueRange,
    hyper_rustls::{self, HttpsConnector},
    hyper_util::{self, client::legacy::connect::HttpConnector},
    yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey},
};
use serde_json::Value;
use std::fmt;
use std::{
    env,
    fs::File,
    io::Read,
    sync::Arc,
};
use tokio::sync::Mutex;


use super::players::Players;
use super::tracks::Tracks;
use super::records::Records;

pub struct GSheet {
    pub sheets: Arc<Mutex<Sheets<HttpsConnector<HttpConnector>>>>,
    pub document_id: String,
}

impl fmt::Debug for GSheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GSheet")
            .field("document_id", &self.document_id)
            .field("sheets", &"<omitted>")
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GSheetError {
    #[error(transparent)]
    Env(#[from] std::env::VarError),

    #[error(transparent)]
    ServiceAccount(#[from] ServiceAccountError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl GSheet {
    pub async fn try_new() -> Result<Self, GSheetError> {
        let document_id = env::var("GOOGLE_SHEET_ID")?;
        let service_account_path = env::var("SERVICE_ACCOUNT_JSON")?;
        let service_account = read_service_account_json(&service_account_path)?;
        let builder = ServiceAccountAuthenticator::builder(service_account);
        let auth = builder.build().await?;
        let client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build(
                    hyper_rustls::HttpsConnectorBuilder::new()
                        .with_native_roots()
                        .unwrap()
                        .https_or_http()
                        .enable_http1()
                        .build(),
                );
        let sheets: Sheets<HttpsConnector<HttpConnector>> = Sheets::new(client, auth);

        sheets.spreadsheets();

        Ok(GSheet {
            sheets: Arc::new(Mutex::new(sheets)),
            document_id,
        })
    }

    pub async fn write_cell(&self, cell: String, value: Value) -> Result<(), google_sheets4::Error> {
        let values = vec![vec![value]];

        let request: ValueRange = ValueRange {
            major_dimension: Some("ROWS".to_owned()),
            range: Some(cell.clone()),
            values: Some(values),
        };

        let sheets = self
            .sheets
            .lock()
            .await;

        sheets
            .spreadsheets()
            .values_update(request, &self.document_id, &cell)
            .value_input_option("RAW")
            .doit()
            .await?;

        Ok(())
    }
}

impl<'a> GSheet {
    pub fn tracks(&'a self) -> Tracks<'a> {
        Tracks::new(self)
    }

    pub fn players(&'a self) -> Players<'a> {
        Players::new(self)
    }

    pub fn records(&'a self) -> Records<'a> {
        Records::new(self)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceAccountError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error)
}

fn read_service_account_json(file_path: &str) -> Result<ServiceAccountKey, ServiceAccountError> {
    let mut file = File::open(file_path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let acc: ServiceAccountKey = serde_json::from_str(&contents)?;

    Ok(acc)
}
