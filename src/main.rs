#![allow(dead_code, unused_variables)]
mod bars;
mod buffer;
mod common;
mod cursor;
mod editor;
mod error;
mod viewport;
use buffer::VecBuffer;
pub use common::*;
pub use tracing::{error, info, span, warn, Instrument};
use tracing_subscriber::layer::SubscriberExt;
pub use tracing_subscriber::{filter::EnvFilter, prelude::*, Layer};
pub use tracing_tree::HierarchicalLayer;

fn main() {
    setup_tracing();
    let mut instance = editor::Editor::new(VecBuffer::new(vec![" ".to_string()]), false);
    match instance.run_event_loop() {
        Err(Error::ExitCall) => info!("Quitting due to ExitCall"),
        _ => error!("Unexpected end to our journey"),
    }
}

fn setup_tracing() {
    let filter = EnvFilter::try_new("info, your_crate_name = trace, crossterm = off")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(
            HierarchicalLayer::new(2)
                .with_writer(std::io::stderr)
                .with_targets(true)
                .with_bracketed_fields(true),
        )
        .with(filter)
        .init();
}
