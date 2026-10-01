//! All Windows settings and custom session forms use the native WinUI companion.
use crate::state::{Request, Snapshot};
use std::sync::mpsc::Sender;

pub fn show(snapshot: Snapshot, sender: Sender<Request>) -> Result<(), String> {
    super::winui::show(snapshot, sender)
}
