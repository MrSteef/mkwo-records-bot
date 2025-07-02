use anyhow::Result;
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

impl GSheet {
    pub async fn try_new() -> Result<Self> {
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

    pub async fn write_cell(&self, cell: String, value: Value) -> Result<()> {
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

fn read_service_account_json(file_path: &str) -> Result<ServiceAccountKey> {
    let mut file = File::open(file_path).map_err(|e| serde_json::Error::io(e))?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| serde_json::Error::io(e))?;

    serde_json::from_str(&contents).map_err(|e| anyhow::anyhow!(e.to_string()))
}
