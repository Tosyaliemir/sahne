use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("no media player is connected")]
    NoPlayer,
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("D-Bus error: {0}")]
    Fdo(#[from] zbus::fdo::Error),
}

// Tauri commands send errors to the frontend; a message string is all it needs.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
