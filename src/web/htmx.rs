use std::{future, convert::Infallible};

use axum::extract::FromRequestParts;

pub struct IsHtmx(pub bool);

impl<S> FromRequestParts<S> for IsHtmx {
    type Rejection = Infallible;

    fn from_request_parts<'life0,'life1,'async_trait>(parts: &'life0 mut axum::http::request::Parts, _state: &'life1 S) ->  core::pin::Pin<Box<dyn core::future::Future<Output = Result<Self,Self::Rejection> > + core::marker::Send+'async_trait> >where 'life0:'async_trait,'life1:'async_trait,Self:'async_trait {
        Box::pin(future::ready(Ok(Self(parts.headers.get("HX-Request").map_or(false, |h| h == "true")))))
    }
}