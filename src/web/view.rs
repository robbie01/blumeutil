use std::fmt::Display;
use html::{scripting::Script, tables::{children::TableRowChild, TableCell, TableRow}};
use super::Row;

pub type CurrentUrlGenerator = Box<dyn Fn(&str, u32, u32) -> String + Send + Sync>;

pub struct View {
    current: CurrentUrlGenerator,
    edit_current: CurrentUrlGenerator
}

fn htmx() -> Script {
    Script::builder()
        .src("https://cdnjs.cloudflare.com/ajax/libs/htmx/2.0.2/htmx.min.js")
        .integrity("sha512-iRA+DJgDLUKPk+pqGpAM3+wSDeO5iKQkYXGtIv4+EA8oJWN1mewMJD7kWC9hHCyUTdey9bQypAhx6PASWShjOw==")
        .crossorigin("anonymous")
        .referrerpolicy("no-referrer")
        .build()
}

impl View {
    pub fn new(
        current: impl Fn(&str, u32, u32) -> String + Send + Sync + 'static,
        edit_current: impl Fn(&str, u32, u32) -> String + Send + Sync + 'static
    ) -> Self {
        Self { current: Box::new(current), edit_current: Box::new(edit_current) }
    }

    pub fn render_current(
        &self,
        session: &str,
        scriptid: u32,
        address: u32,
        current: String
    ) -> impl Display + Into<TableRowChild> {
        TableCell::builder()
            .division(|b| b
                .class("current")
                .division(|b| b.text(current))
                .button(|b| b
                    .data("hx-get", (self.edit_current)(session, scriptid, address))
                    .text("✏️")))
            .build()
    }

    pub fn render_current_edit(
        &self,
        session: &str,
        scriptid: u32,
        address: u32,
        current: String
    ) -> impl Display + Into<TableRowChild> {
        let url = (self.current)(session, scriptid, address);

        TableCell::builder()
            .division(|b| b
                .class("current")
                .input(|b| b.type_("text").name("current").value(current))
                .button(|b| b
                    .data("hx-put", url.clone())
                    .data("hx-include", "closest tr")
                    .text("💾"))
                .button(|b| b
                    .data("hx-get", url)
                    .text("❌")))
            .build()
    }

    pub fn render(
        &self,
        session: &str,
        scriptid: u32,
        rows: impl IntoIterator<Item = Row>
    ) -> impl Display {
        html::root::Html::builder()
            .lang("en")
            .head(|b| b
                .meta(|b| b.charset("utf-8"))
                .title(|b| b.text("Bruh"))
                .style(|b| b.text(include_str!("view.css"))))
            .body(|b| b
                .table(|b| b
                    .table_column_group(|b| b
                        .table_column(|b| b
                            .span("2")
                            .class("meta-cols"))
                        .table_column(|b| b
                            .span("3")
                            .class("text-cols")))
                    .table_head(|b| b
                        .table_row(|b| b
                            .table_header(|b| b.text("address"))
                            .table_header(|b| b.text("speaker"))
                            .table_header(|b| b.text("original"))
                            .table_header(|b| b.text("control"))
                            .table_header(|b| b.text("current"))))
                    .table_body(|b| b
                        .data("hx-target", "closest td")
                        .data("hx-swap", "outerHTML")
                        .extend(rows.into_iter().map(|Row { address, speaker, original, control, current }|
                            TableRow::builder()
                                .table_cell(|b| b.text(address.to_string()))
                                .table_cell(|b| b.lang("ja").text(speaker))
                                .table_cell(|b| b.lang("ja").text(original))
                                .table_cell(|b| b.text(control))
                                .push(self.render_current(session, scriptid, address, current))
                            .build()))))
                .push(htmx()))
            .build()
    }
}
