#[macro_export]
macro_rules! return_on_shutdown {
    ($shutdown:expr) => {
        if $shutdown {
            return;
        }
    };
}
