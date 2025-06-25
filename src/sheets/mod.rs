use anyhow::Result;
use google_sheets4::{
    Sheets,
    hyper_rustls::{self, HttpsConnector},
    hyper_util::{self, client::legacy::connect::HttpConnector},
    yup_oauth2::{ServiceAccountAuthenticator, ServiceAccountKey},
};
use std::{env, fs::File, io::Read, sync::Arc};
use tokio::sync::Mutex;

mod tracks;
use crate::sheets::tracks::Tracks;
mod players;
use crate::sheets::players::Players;
mod records;
use crate::sheets::records::Records;

pub trait DataRanges {
    fn sheet_name() -> &'static str;
    fn first_column() -> &'static str;
    fn last_column() -> &'static str;

    fn table_range() -> String {
        format!(
            "{}!{}:{}",
            Self::sheet_name(),
            Self::first_column(),
            Self::last_column()
        )
    }

    fn row_range(row: usize) -> String {
        format!(
            "{}!{}{}:{}{}",
            Self::sheet_name(),
            Self::first_column(),
            row,
            Self::last_column(),
            row
        )
    }

    fn rows_range(from: usize, to: usize) -> String {
        format!(
            "{}!{}{}:{}{}",
            Self::sheet_name(),
            Self::first_column(),
            from,
            Self::last_column(),
            to
        )
    }

    fn cell(row: usize, col: String) -> String {
        format!(
            "{}!{}{}:{}{}",
            Self::sheet_name(),
            col,
            row,
            col,
            row
        )
    }
}

pub struct GSheet {
    sheets: Arc<Mutex<Sheets<HttpsConnector<HttpConnector>>>>,
    document_id: String,
}

impl<'a> GSheet {
    pub async fn try_new() -> Result<Self> {
        let document_id = env::var("GOOGLE_SHEET_ID")?;
        let service_account_path = env::var("SERVICE_ACCOUNT_JSON")?;
        let service_account = read_service_account_json(&service_account_path)
            .expect("Expected valid service account JSON");
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

    pub fn tracks(&'a self) -> Tracks<'a> {
        Tracks { gsheet: self }
    }

    pub fn players(&'a self) -> Players<'a> {
        Players { gsheet: self }
    }
    
    pub fn records(&'a self) -> Records<'a> {
        Records { gsheet: self }
    }
}

fn read_service_account_json(file_path: &str) -> Result<ServiceAccountKey> {
    let mut file = File::open(file_path).map_err(|e| serde_json::Error::io(e))?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| serde_json::Error::io(e))?;

    serde_json::from_str(&contents).map_err(|e| anyhow::anyhow!(e.to_string()))
}
