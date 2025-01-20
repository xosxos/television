use std::env;

use color_eyre::Result;
use tracing::error;

pub fn init() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
        .panic_section(format!(
            "This is a bug. Consider reporting it at {}",
            env!("CARGO_PKG_REPOSITORY")
        ))
        .capture_span_trace_by_default(true)
        .display_env_section(false)
        .into_hooks();

    eyre_hook.install()?;

    std::panic::set_hook(Box::new(move |panic_info| {
        if let Ok(mut t) = crate::tui::Tui::new(std::io::stderr()) {
            if let Err(r) = t.exit() {
                error!("Unable to exit Terminal: {:?}", r);
            }
        }

        eprintln!("{}", panic_hook.panic_report(panic_info)); // prints color-eyre stack trace to stderr

        let msg = format!("{}", panic_hook.panic_report(panic_info));

        error!("Error: {}", msg);

        std::process::exit(1);
    }));
    Ok(())
}
