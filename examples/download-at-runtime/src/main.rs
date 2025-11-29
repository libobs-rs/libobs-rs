use std::{convert::Infallible, sync::Arc, time::Duration};

use indicatif::{ProgressBar, ProgressStyle};
use libobs_bootstrapper::{
    ObsBootstrapper, ObsBootstrapperOptions, ObsBootstrapperResult,
    status_handler::ObsBootstrapStatusHandler,
};
use libobs_wrapper::{context::ObsContext, utils::StartupInfo};

#[derive(Debug, Clone)]
struct ObsBootstrapProgress(Arc<ProgressBar>);

impl ObsBootstrapProgress {
    pub fn new() -> Self {
        let bar = ProgressBar::new(200).with_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap(),
        );

        bar.set_message("Initializing bootstrapper...");
        bar.enable_steady_tick(Duration::from_millis(50));

        Self(Arc::new(bar))
    }

    pub fn done(&self) {
        self.0.finish();
    }
}
impl ObsBootstrapStatusHandler for ObsBootstrapProgress {
    type Error = Infallible;

    fn handle_downloading(&mut self, prog: f32, msg: String) -> Result<(), Infallible> {
        self.0.set_message(msg);
        self.0.set_position((prog * 100.0) as u64);

        Ok(())
    }
    fn handle_extraction(&mut self, prog: f32, msg: String) -> Result<(), Infallible> {
        self.0.set_message(msg);
        self.0.set_position(100 + (prog * 100.0) as u64);

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    println!("Starting OBS bootstrapper...");
    let handler = ObsBootstrapProgress::new();

    let res = ObsBootstrapper::bootstrap_with_handler(
        &ObsBootstrapperOptions::default(),
        Box::new(handler.clone()),
    )
    .await
    .unwrap();
    if matches!(res, ObsBootstrapperResult::Restart) {
        println!("OBS has been downloaded and extracted. The application will now restart.");
        return;
    }

    let context = ObsContext::new(StartupInfo::default()).unwrap();
    handler.done();

    println!("Done");
    // Use the context here
    // For example creating new obs data
    context.data().unwrap();
}
