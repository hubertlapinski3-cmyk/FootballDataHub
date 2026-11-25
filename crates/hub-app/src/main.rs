//! Desktop runner (same UI as Android), phone-sized window.
fn main() -> eframe::Result<()> {
    let root = std::env::var("DATAHUB_DIR").map(std::path::PathBuf::from).unwrap_or_else(|_| std::path::PathBuf::from("datahub_data"));
    eframe::run_native("Data Hub", hub_app::native_options(), Box::new(move |cc| Ok(Box::new(hub_app::App::new(&cc.egui_ctx, root)))))
}
