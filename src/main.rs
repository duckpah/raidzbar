mod app;
mod backend;
mod core;
mod renderer;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("raidzbar starting");

    let config = core::config::Config::load();
    let bars = app::build_bars(&config);

    if bars.is_empty() {
        log::error!("No bars defined in config; exiting");
        std::process::exit(1);
    }

    backend::run(bars);
}
