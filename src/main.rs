#![allow(dead_code, unused_variables)]
mod bars;
mod buffer;
mod common;
mod cursor;
mod editor;
mod error;
mod viewport;
use std::{fs::File, panic};

use buffer::VecBuffer;
use clap::Parser;
pub use common::*;
use editor::Editor;
pub use tracing::{error, info, span, warn, Instrument};
pub use tracing_subscriber::{filter::EnvFilter, fmt::Subscriber, prelude::*, Layer};
pub use tracing_tree::HierarchicalLayer;

#[derive(Parser, Debug)]
#[command(name = "neotext")]
struct Cli {
    #[arg(short, long)]
    debug: bool,

    // Open neotext on the the dedcicated testfile
    #[arg(short = 't', long)]
    test: bool,

    // Read File on given path, this argument is the default argument being passed
    #[arg(default_value = "")]
    file: String,
}

fn main() {
    let cli = Cli::parse();
    setup_tracing(cli.debug);

    // Capture Panics
    panic::set_hook(Box::new(|panic_info| {
        let (filename, line) = panic_info
            .location()
            .map(|loc| (loc.file(), loc.line()))
            .unwrap_or(("<unknown>", 0));

        let cause = panic_info
            .payload()
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .or_else(|| panic_info.payload().downcast_ref::<&str>().copied())
            .unwrap_or("<cause unknown>");

        error!(
            "Panic occurred in file '{}' at line {}: {}",
            filename, line, cause
        );
    }));

    let mut instance = initialize_editor(&cli);

    match instance.run_event_loop() {
        Err(Error::ExitCall) => info!("Quitting due to ExitCall"),
        otherwise => error!("Unexpected end to our journey: {:?}", otherwise),
    }
}

fn initialize_editor(cli: &Cli) -> Editor<VecBuffer> {
    if cli.test {
        return new_from_file(&"./test_file.neotext".into());
    }

    if cli.file.is_empty() {
        editor::Editor::new(VecBuffer::new(vec![" ".to_string()]), false)
    } else {
        new_from_file(&cli.file.clone().into())
    }
}

pub fn new_from_file(p: &std::path::PathBuf) -> Editor<VecBuffer> {
    let content = match std::fs::read(p) {
        Err(e) => panic!("Invalid path: {:?}, exception: {}", p, e),
        Ok(content) => content,
    };
    Editor::new(
        VecBuffer::new(
            String::from_utf8(content)
                .expect("Invalid utf8 file")
                .lines()
                .map(String::from)
                .collect(),
        ),
        false,
    )
}
fn setup_tracing(debug: bool) {
    let filter = EnvFilter::try_new("info, neotext = trace, crossterm = off")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stderr_layer = HierarchicalLayer::new(2)
        .with_writer(std::io::stderr)
        .with_targets(true)
        .with_bracketed_fields(true);

    let subscriber = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer);

    // Set debug to automatically output to a dbg file
    if debug {
        let file = File::create("dbg").expect("Failed to create debug log file");
        let file_layer = HierarchicalLayer::new(2)
            .with_writer(file)
            .with_targets(true)
            .with_bracketed_fields(true)
            .with_ansi(false);

        subscriber.with(file_layer).init();
    } else {
        subscriber.init();
    }
}
