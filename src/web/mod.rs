mod err;
mod htmx;
mod model;
mod view;

use err::ResultExt as _;
use model::{Model, Row};
use view::View;

use std::sync::Arc;

use axum::{extract::{Path, State}, http::Response, middleware, response::{Html, IntoResponse}, routing::{get, on, MethodFilter}, Form, Router};
use clap::Parser;
use rusqlite::Connection;
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[derive(Parser)]
pub struct Args {
    #[arg(default_value_t = 9000)]
    port: u16
}

struct AppState {
    model: Model,
    view: View
}

pub async fn run(db: Connection, args: Args) -> anyhow::Result<()> {
    Ok(axum::serve(
        TcpListener::bind(("::", args.port)).await?,
        Router::new()
            .route("/:session/:scriptid", get(table))
            .route("/:session/:scriptid/:address", on(MethodFilter::GET.or(MethodFilter::PUT), table_row))
            .route("/:session/:scriptid/:address/edit", get(table_row_editor))
            .layer(middleware::map_response(|mut r: Response<_>| async {
                r.headers_mut().append("cache-control", "no-cache".parse().unwrap());
                r
            }))
            .layer(TraceLayer::new_for_http())
            .with_state(Arc::new(AppState {
                model: Model::new(db),
                view: View::new(
                    |session, scriptid, address|
                        format!("/{session}/{scriptid}/{address}"),
                    |session, scriptid, address|
                        format!("/{session}/{scriptid}/{address}/edit")
                )
            }))
    ).await?)
}

#[derive(Deserialize)]
struct ShowTableParams {
    session: String,
    scriptid: u32
}

async fn table(
    State(state): State<Arc<AppState>>,
    Path(ShowTableParams { session, scriptid }): Path<ShowTableParams>
) -> axum::response::Result<impl IntoResponse> {
    let rows = state.model.translations(&session, scriptid).with_ise()?;
    let res = state.view.render(&session, scriptid, rows).to_string();

    Ok(Html(res))
}

#[derive(Deserialize)]
struct TableRowParams {
    session: String,
    scriptid: u32,
    address: u32
}

#[derive(Deserialize)]
struct TableRowQuery {
    current: String
}

async fn table_row(
    State(state): State<Arc<AppState>>,
    Path(TableRowParams { session, scriptid, address }): Path<TableRowParams>,
    frm: Option<Form<TableRowQuery>>
) -> axum::response::Result<impl IntoResponse> {
    let current = state.model.translation(
        &session, scriptid, address,
        frm.as_ref().map(|f| f.0.current.as_str())
    ).with_ise()?;
    let res = state.view.render_current(&session, scriptid, address, current).to_string();

    Ok(Html(res))
}

async fn table_row_editor(
    State(state): State<Arc<AppState>>,
    Path(TableRowParams { session, scriptid, address }): Path<TableRowParams>
) -> axum::response::Result<impl IntoResponse> {
    let current = state.model.translation(&session, scriptid, address, None).with_ise()?;
    let res = state.view.render_current_edit(&session, scriptid, address, current).to_string();

    Ok(Html(res))
}