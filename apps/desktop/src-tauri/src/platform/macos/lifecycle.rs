pub struct Registration;
impl Registration {
    pub fn new(_: std::sync::mpsc::Sender<crate::state::Request>) -> Result<Self, String> {
        Err(
            "macOS native suspend/resume observer pending; clock discontinuities clear sessions."
                .into(),
        )
    }
}
