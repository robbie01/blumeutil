use std::fmt::Display;
use axum::http::StatusCode;

pub trait ResultExt<T, E> {
    fn with_ise(self) -> axum::response::Result<T> where E: Display;
    fn with_ise_msg(self, msg: &'static str) -> axum::response::Result<T>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn with_ise(self) -> axum::response::Result<T> where E: Display {
        self.map_err(move |err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into())
    }

    fn with_ise_msg(self, msg: &'static str) -> axum::response::Result<T> {
        self.or(Err((StatusCode::INTERNAL_SERVER_ERROR, msg).into()))
    }
}