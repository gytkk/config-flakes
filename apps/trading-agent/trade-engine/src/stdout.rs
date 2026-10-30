use std::fmt::{Display, Write as _};

use anstyle::{AnsiColor, Style};
use comfy_table::{
    Attribute, Cell, CellAlignment, Color, ContentArrangement, Table, presets::UTF8_BORDERS_ONLY,
};

const TICK_HEADING_PREFIX: &str = "Market tick #";
const TICK_SEPARATOR_WIDTH: usize = 72;
const POSITIONS_HEADING: &str = "Positions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputField {
    key: String,
    value: String,
}

impl OutputField {
    pub fn new(key: impl Into<String>, value: impl Display) -> Self {
        Self {
            key: key.into(),
            value: value.to_string(),
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Plain,
    Pretty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdoutRenderOptions {
    pub format: OutputFormat,
    pub color: ColorMode,
}

impl StdoutRenderOptions {
    pub const fn plain() -> Self {
        Self {
            format: OutputFormat::Plain,
            color: ColorMode::Auto,
        }
    }

    pub const fn pretty(color: ColorMode) -> Self {
        Self {
            format: OutputFormat::Pretty,
            color,
        }
    }

    fn emit_ansi(self) -> bool {
        self.color != ColorMode::Never
    }
}

impl Default for StdoutRenderOptions {
    fn default() -> Self {
        Self::pretty(ColorMode::Auto)
    }
}

pub fn render_stdout_chunk(chunk: &str, options: StdoutRenderOptions) -> String {
    if options.format == OutputFormat::Plain {
        return chunk.to_owned();
    }
    render_pretty_chunk(chunk, options)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StructuredOutput {
    buffer: String,
}

impl StructuredOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn heading(&mut self, title: impl Display) -> &mut Self {
        let _ = writeln!(self.buffer, "{title}");
        self
    }

    pub fn field(&mut self, key: impl Display, value: impl Display) -> &mut Self {
        let _ = writeln!(self.buffer, "  {key}: {value}");
        self
    }

    pub fn status_line(
        &mut self,
        name: impl Display,
        status: impl Display,
        fields: &[OutputField],
    ) -> &mut Self {
        let _ = write!(self.buffer, "  {name}: {status}");
        write_fields_suffix(&mut self.buffer, fields);
        let _ = writeln!(self.buffer);
        self
    }

    pub fn item(&mut self, label: impl Display, fields: &[OutputField]) -> &mut Self {
        let _ = write!(self.buffer, "  - {label}");
        for field in fields {
            let _ = write!(self.buffer, " {}={}", field.key, field.value);
        }
        let _ = writeln!(self.buffer);
        self
    }

    pub fn empty_item(&mut self) -> &mut Self {
        let _ = writeln!(self.buffer, "  none");
        self
    }

    pub fn blank(&mut self) -> &mut Self {
        let _ = writeln!(self.buffer);
        self
    }

    pub fn finish(self) -> String {
        self.buffer
    }
}

fn write_fields_suffix(buffer: &mut String, fields: &[OutputField]) {
    if fields.is_empty() {
        return;
    }

    let _ = write!(buffer, " (");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            let _ = write!(buffer, ", ");
        }
        let _ = write!(buffer, "{}={}", field.key, field.value);
    }
    let _ = write!(buffer, ")");
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrettyRow {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrettyItemRow {
    label: String,
    fields: Vec<PrettyItemField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrettyItemField {
    key: String,
    value: String,
}

fn render_pretty_chunk(chunk: &str, options: StdoutRenderOptions) -> String {
    if chunk.trim().is_empty() {
        return chunk.to_owned();
    }

    let mut rendered = String::new();
    let mut block = Vec::new();
    for line in chunk.lines() {
        if line.is_empty() {
            flush_pretty_block(&mut rendered, &mut block, options);
            rendered.push('\n');
        } else {
            block.push(line.to_owned());
        }
    }
    flush_pretty_block(&mut rendered, &mut block, options);
    rendered
}

fn flush_pretty_block(
    rendered: &mut String,
    block: &mut Vec<String>,
    options: StdoutRenderOptions,
) {
    if block.is_empty() {
        return;
    }

    let Some((heading, lines)) = block.split_first() else {
        return;
    };
    if heading.starts_with(' ') {
        rendered.push_str(&block.join("\n"));
        rendered.push('\n');
        block.clear();
        return;
    }

    rendered.push_str(&render_pretty_heading(heading, options));
    rendered.push('\n');

    if heading == POSITIONS_HEADING
        && let Some(table) = render_pretty_positions_table(lines, options)
    {
        rendered.push_str(&table);
        rendered.push('\n');
        block.clear();
        return;
    }

    let rows = parse_pretty_rows(lines);
    if !rows.is_empty() {
        rendered.push_str(&render_pretty_table(&rows, options));
        rendered.push('\n');
    }
    block.clear();
}

fn render_pretty_heading(heading: &str, options: StdoutRenderOptions) -> String {
    if is_tick_heading(heading) {
        return style_text(
            &tick_separator_heading(heading),
            tick_heading_style(),
            options.emit_ansi(),
        );
    }
    style_text(heading, heading_style(), options.emit_ansi())
}

fn is_tick_heading(heading: &str) -> bool {
    heading.starts_with(TICK_HEADING_PREFIX)
}

fn tick_separator_heading(heading: &str) -> String {
    let label = format!(" {heading} ");
    if label.len() >= TICK_SEPARATOR_WIDTH {
        return label;
    }

    let padding = TICK_SEPARATOR_WIDTH - label.len();
    let left = padding / 2;
    let right = padding - left;
    format!("{}{}{}", "=".repeat(left), label, "=".repeat(right))
}

fn parse_pretty_rows(lines: &[String]) -> Vec<PrettyRow> {
    lines
        .iter()
        .map(|line| {
            if let Some(item) = line.strip_prefix("  - ") {
                return PrettyRow {
                    key: "item".to_owned(),
                    value: item.to_owned(),
                };
            }
            if let Some(field) = line.strip_prefix("  ") {
                if let Some((key, value)) = field.split_once(": ") {
                    return PrettyRow {
                        key: key.to_owned(),
                        value: value.to_owned(),
                    };
                }
                return PrettyRow {
                    key: "detail".to_owned(),
                    value: field.to_owned(),
                };
            }
            PrettyRow {
                key: "detail".to_owned(),
                value: line.to_owned(),
            }
        })
        .collect()
}

fn render_pretty_table(rows: &[PrettyRow], options: StdoutRenderOptions) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic);
    for row in rows {
        table.add_row(vec![
            Cell::new(row.key.clone()).add_attribute(Attribute::Bold),
            value_cell(row, options),
        ]);
    }
    table.to_string()
}

fn render_pretty_positions_table(lines: &[String], options: StdoutRenderOptions) -> Option<String> {
    let rows = parse_pretty_item_rows(lines)?;
    if rows.is_empty() {
        return None;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            header_cell("symbol"),
            header_cell("quantity"),
            header_cell("average_price"),
            header_cell("current_price"),
            header_cell("return_pct"),
        ]);

    for row in &rows {
        table.add_row(vec![
            Cell::new(row.label.clone()),
            numeric_cell(row.field_value("quantity")?),
            numeric_cell(row.field_value("average_price")?),
            numeric_cell(row.field_value("current_price")?),
            return_pct_cell(row.field_value("unrealized_return_pct")?, options),
        ]);
    }

    Some(table.to_string())
}

fn parse_pretty_item_rows(lines: &[String]) -> Option<Vec<PrettyItemRow>> {
    lines
        .iter()
        .map(|line| parse_pretty_item_row(line))
        .collect()
}

fn parse_pretty_item_row(line: &str) -> Option<PrettyItemRow> {
    let item = line.strip_prefix("  - ")?;
    let mut parts = item.split_whitespace();
    let label = parts.next()?.to_owned();
    let fields = parts
        .map(|part| {
            let (key, value) = part.split_once('=')?;
            Some(PrettyItemField {
                key: key.to_owned(),
                value: value.to_owned(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if fields.is_empty() {
        return None;
    }
    Some(PrettyItemRow { label, fields })
}

impl PrettyItemRow {
    fn field_value(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|field| field.key == key)
            .map(|field| field.value.as_str())
    }
}

fn header_cell(value: &str) -> Cell {
    Cell::new(value).add_attribute(Attribute::Bold)
}

fn numeric_cell(value: &str) -> Cell {
    Cell::new(value).set_alignment(CellAlignment::Right)
}

fn return_pct_cell(value: &str, options: StdoutRenderOptions) -> Cell {
    let cell = numeric_cell(value);
    if !options.emit_ansi() {
        return cell;
    }

    if let Ok(return_pct) = value.parse::<f64>() {
        if return_pct > 0.0 {
            return cell.fg(Color::Green).add_attribute(Attribute::Bold);
        }
        if return_pct < 0.0 {
            return cell.fg(Color::Red).add_attribute(Attribute::Bold);
        }
    }
    cell
}

fn value_cell(row: &PrettyRow, options: StdoutRenderOptions) -> Cell {
    let cell = Cell::new(row.value.clone());
    if !options.emit_ansi() || row.key != "status" {
        return cell;
    }

    match status_color(&row.value) {
        Some(color) => cell.fg(color).add_attribute(Attribute::Bold),
        None => cell,
    }
}

fn status_color(value: &str) -> Option<Color> {
    let status = value.split_whitespace().next().unwrap_or(value);
    match status {
        "passed" | "parsed" | "completed" | "reconciled" | "submitted" => Some(Color::Green),
        "started" | "invoking" | "requested" => Some(Color::Cyan),
        "stopped" | "skipped" | "skipped_previous_tick_running" | "none" => Some(Color::Yellow),
        "failed" | "blocked" | "rejected" | "error" | "snapshot_failed" => Some(Color::Red),
        _ => None,
    }
}

fn heading_style() -> Style {
    Style::new().bold().fg_color(Some(AnsiColor::Cyan.into()))
}

fn tick_heading_style() -> Style {
    Style::new()
        .bold()
        .fg_color(Some(AnsiColor::Magenta.into()))
}

fn style_text(value: &str, style: Style, emit_ansi: bool) -> String {
    if emit_ansi {
        format!("{style}{value}{style:#}")
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ColorMode, OutputField, OutputFormat, StdoutRenderOptions, StructuredOutput,
        render_stdout_chunk,
    };

    #[test]
    fn renders_headings_fields_status_lines_and_items() {
        let mut output = StructuredOutput::new();
        output
            .heading("Dry-run loop")
            .field("status", "started")
            .status_line(
                "broker_ping",
                "passed",
                &[
                    OutputField::new("environment", "paper"),
                    OutputField::new("writable", false),
                ],
            )
            .blank()
            .heading("Quotes")
            .item("005930", &[OutputField::new("last_price", "73000")]);

        assert_eq!(
            output.finish(),
            "Dry-run loop\n  status: started\n  broker_ping: passed (environment=paper, writable=false)\n\nQuotes\n  - 005930 last_price=73000\n"
        );
    }

    #[test]
    fn plain_renderer_passes_chunks_through() {
        let chunk = "Market tick #1\n  status: completed\n\n";

        assert_eq!(
            render_stdout_chunk(chunk, StdoutRenderOptions::plain()),
            chunk
        );
    }

    #[test]
    fn default_render_options_use_pretty_auto_color() {
        assert_eq!(
            StdoutRenderOptions::default(),
            StdoutRenderOptions::pretty(ColorMode::Auto)
        );
    }

    #[test]
    fn pretty_renderer_turns_blocks_into_tables_without_color_when_disabled() {
        let chunk = "Market tick #1\n  status: completed\n  provider: command\n\n";
        let rendered = render_stdout_chunk(
            chunk,
            StdoutRenderOptions {
                format: OutputFormat::Pretty,
                color: ColorMode::Never,
            },
        );

        assert!(rendered.contains("Market tick #1"));
        assert!(rendered.contains("=== Market tick #1 ==="));
        assert!(rendered.contains("status"));
        assert!(rendered.contains("completed"));
        assert!(rendered.contains("provider"));
        assert!(!rendered.contains("\x1b["));
        assert_ne!(rendered, chunk);
    }

    #[test]
    fn pretty_renderer_only_adds_separators_to_market_tick_headings() {
        let chunk = "Universe discovery #1\n  status: parsed\n\n";
        let rendered = render_stdout_chunk(
            chunk,
            StdoutRenderOptions {
                format: OutputFormat::Pretty,
                color: ColorMode::Never,
            },
        );

        assert!(rendered.contains("Universe discovery #1"));
        assert!(!rendered.contains("==="));
    }

    #[test]
    fn pretty_renderer_emits_ansi_when_color_is_enabled() {
        let chunk = "Market tick #1\n  status: failed\n\n";
        let rendered = render_stdout_chunk(chunk, StdoutRenderOptions::pretty(ColorMode::Always));

        assert!(rendered.contains("\x1b["));
    }

    #[test]
    fn pretty_renderer_turns_positions_into_multicolumn_table() {
        let chunk = "Positions\n  - 005930 quantity=10 average_price=70000 current_price=73000 unrealized_return_pct=4.29\n\n";
        let rendered = render_stdout_chunk(
            chunk,
            StdoutRenderOptions {
                format: OutputFormat::Pretty,
                color: ColorMode::Never,
            },
        );

        assert!(rendered.contains("Positions"));
        assert!(rendered.contains("symbol"));
        assert!(rendered.contains("quantity"));
        assert!(rendered.contains("average_price"));
        assert!(rendered.contains("current_price"));
        assert!(rendered.contains("return_pct"));
        assert!(rendered.contains("005930"));
        assert!(rendered.contains("4.29"));
        assert!(!rendered.contains("unrealized_return_pct"));
    }
}
