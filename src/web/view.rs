use std::fmt::Display;
use html::{forms::children::FormChild, scripting::Script, tables::TableRow, text_content::Division};
use super::Row;

pub type UpdateCurrent = Box<dyn Fn(&str, u32, u32) -> String + Send + Sync>;

pub struct View {
    update_current: UpdateCurrent
}

fn htmx() -> Script {
    Script::builder()
        .src("https://cdnjs.cloudflare.com/ajax/libs/htmx/1.9.10/htmx.min.js")
        .integrity("sha512-9qpauSP4+dDIldsrdNEZ2Z7JoyLZGfJsAP2wfXnc3drOh+5NXOBxjlq3sGXKdulmN9W+iwLxRt42zKMa8AHEeg==")
        .crossorigin("anonymous")
        .referrerpolicy("no-referrer")
        .build()
}

impl View {
    pub fn new(update_current: impl Fn(&str, u32, u32) -> String + Send + Sync + 'static) -> Self {
        Self { update_current: Box::new(update_current) }
    }

    pub fn render_current(
        &self,
        _session: &str,
        _scriptid: u32,
        _address: u32,
        current: String
    ) -> impl Display + Into<FormChild> {
        Division::builder().class("current").text(current).build()
    }

    pub fn render(
        &self,
        session: &str,
        scriptid: u32,
        rows: impl IntoIterator<Item = Row>
    ) -> impl Display {
        html::root::Html::builder()
            .head(|b| b
                .meta(|b| b.charset("utf-8"))
                .title(|b| b.text("Bruh"))
                .style(|b| b.text(include_str!("view.css"))))
            .body(|b| b
                .table(|b| b
                    .table_head(|b| b
                        .table_row(|b| b
                            .table_header(|b| b.text("address"))
                            .table_header(|b| b.text("speaker"))
                            .table_header(|b| b.text("original"))
                            .table_header(|b| b.text("control"))
                            .table_header(|b| b.text("current"))))
                    .table_body(|b| b
                        .data("hx-target", "find .current")
                        .data("hx-swap", "outerHTML")
                        .data("hx-on::before-request", "event.detail.elt.elements['current'].value = ''")
                        .extend(rows.into_iter().map(|Row { address, speaker, original, control, current }|
                            TableRow::builder()
                                .table_cell(|b| b.text(address.to_string()))
                                .table_cell(|b| b.text(speaker))
                                .table_cell(|b| b.text(original))
                                .table_cell(|b| b.text(control))
                                .table_cell(|b| b
                                    .form(|b| b
                                        .data("hx-patch", (self.update_current)(session, scriptid, address))
                                        .push(self.render_current(session, scriptid, address, current))
                                        .input(|b| b
                                            .type_("text")
                                            .name("current"))
                                        .button(|b| b
                                            .text("💾"))))
                            .build()))))
                .push(htmx()))
            .build()
    }
}