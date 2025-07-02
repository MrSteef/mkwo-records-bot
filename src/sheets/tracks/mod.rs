use crate::sheets::gsheet::GSheet;
use anyhow::Result;
mod track;
use super::utils::DataRanges;
use track::Track;

pub struct Tracks<'a> {
    gsheet: &'a GSheet,
}


impl DataRanges for Tracks<'_> {
    const SHEET_NAME: &'static str = "Tracks";
    const FIRST_COLUMN: &'static str = "A";
    const LAST_COLUMN: &'static str = "B";
}

impl<'a> Tracks<'a> {
    pub fn new(gsheet: &'a GSheet) -> Self {
        Tracks { gsheet }
    }
}

impl Tracks<'_> {
    pub const NAME_COLUMN: &'static str = "A";
    pub const ICON_FILE_URL_COLUMN: &'static str = "B";

    pub async fn get_all(&self) -> Result<Vec<Track>> {
        let sheets = self
            .gsheet
            .sheets
            .lock()
            .await;
        let document_id = &self.gsheet.document_id;
        let table_range = &Tracks::table_range();

        let tracks: Vec<Track> = sheets
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
            .filter_map(|(index, row)| Track::from_row(index + 1, row, self.gsheet).ok())
            .collect();

        Ok(tracks)
    }
}