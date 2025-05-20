#[macro_export]
macro_rules! return_on_shutdown {
    ($run:expr) => {
        if !$run.load(Ordering::Relaxed) {
            return;
        }
    };
}
