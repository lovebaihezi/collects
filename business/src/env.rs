use ustr::Ustr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppType {
    Prod,
    Test,
    Local,
}

pub struct AppEnv {
    app_type: AppType,
    worker_url: Ustr,
}
