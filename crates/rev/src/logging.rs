use std::path::PathBuf;

use directories::ProjectDirs;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};

pub fn initialize_logging() -> anyhow::Result<()> {
    let project = match ProjectDirs::from("com", "kjuulh", env!("CARGO_PKG_NAME")) {
        Some(p) => p.data_local_dir().to_path_buf(),
        None => PathBuf::from(".").join(".data"),
    };

    std::fs::create_dir_all(&project)?;
    let log_path = project.join("rev.log");

    println!("logging to: {}", log_path.display());

    let log_file = std::fs::File::create(log_path)?;
    std::env::set_var(
        "RUST_LOG",
        std::env::var("RUST_LOG")
            .or_else(|_| std::env::var("REV_LOG_LEVEL"))
            .unwrap_or_else(|_| format!("{}=info", env!("CARGO_CRATE_NAME"))),
    );
    let file_subscriber = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_filter(tracing_subscriber::filter::EnvFilter::from_default_env());
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .init();
    Ok(())
}

pub fn initialize_panic_handler() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Ok(mut t) = crate::tui::Tui::new() {
            if let Err(r) = t.exit() {
                tracing::error!("Unable to exit Terminal: {:?}", r);
            }
        }

        #[cfg(not(debug_assertions))]
        {
            use human_panic::{handle_dump, print_msg, Metadata};
            let meta = Metadata {
                version: env!("CARGO_PKG_VERSION").into(),
                name: env!("CARGO_PKG_NAME").into(),
                authors: env!("CARGO_PKG_AUTHORS").replace(':', ", ").into(),
                homepage: env!("CARGO_PKG_HOMEPAGE").into(),
            };

            let file_path = handle_dump(&meta, panic_info);
            // prints human-panic message
            print_msg(file_path, &meta)
                .expect("human-panic: printing error message to console failed");
            eprintln!("{}", panic_hook.panic_report(panic_info)); // prints color-eyre stack trace to stderr
        }
        let msg = format!("{}", panic_info);
        tracing::error!("Error: {}", msg);

        // #[cfg(debug_assertions)]
        // {
        //     // Better Panic stacktrace that is only enabled when debugging.
        //     better_panic::Settings::auto()
        //         .most_recent_first(false)
        //         .lineno_suffix(true)
        //         .verbosity(better_panic::Verbosity::Full)
        //         .create_panic_handler()(panic_info);
        // }
    }));
    Ok(())
}
