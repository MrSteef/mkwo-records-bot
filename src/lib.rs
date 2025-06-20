pub mod discord;
pub mod ocr;

// optionally re-export run function
pub use ocr::run_pipeline_from_bytes;