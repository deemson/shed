use std::collections::HashSet;
use std::io::{self, IsTerminal, Stderr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{LineGauge, Paragraph, Widget, Wrap};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};

use crate::manifest::ReportLayout;
#[cfg(test)]
use crate::manifest::{ReportNode, ReportSection};
use crate::sync::{Direction, ItemStatus, Summary};

#[derive(Clone, Copy)]
pub enum Event<'a> {
    ScanStarted {
        item: usize,
    },
    ScanPath {
        item: usize,
        logical_path: &'a Path,
    },
    ScanFinished {
        item: usize,
        total_files: usize,
    },
    PlanFinished {
        total_files: usize,
    },
    ItemStarted {
        item: usize,
    },
    FileStarted {
        item: usize,
        logical_path: &'a Path,
        src: &'a Path,
        dst: &'a Path,
    },
    FileFinished {
        item: usize,
        copied: bool,
        bytes: u64,
    },
    ItemFinished {
        item: usize,
        status: ItemStatus,
    },
    Warning {
        item: Option<usize>,
        message: &'a str,
    },
    Error {
        item: Option<usize>,
        message: &'a str,
    },
    Finish(&'a Summary),
}

#[derive(Clone, Debug)]
pub enum OwnedEvent {
    ScanStarted {
        item: usize,
    },
    ScanPath {
        item: usize,
        logical_path: PathBuf,
    },
    ScanFinished {
        item: usize,
        total_files: usize,
    },
    PlanFinished {
        total_files: usize,
    },
    ItemStarted {
        item: usize,
    },
    FileStarted {
        item: usize,
        logical_path: PathBuf,
        src: PathBuf,
        dst: PathBuf,
    },
    FileFinished {
        item: usize,
        copied: bool,
        bytes: u64,
    },
    ItemFinished {
        item: usize,
        status: ItemStatus,
    },
    Warning {
        item: Option<usize>,
        message: String,
    },
    Error {
        item: Option<usize>,
        message: String,
    },
    Finish(Summary),
}

impl OwnedEvent {
    pub fn borrowed(&self) -> Event<'_> {
        match self {
            Self::ScanStarted { item } => Event::ScanStarted { item: *item },
            Self::ScanPath { item, logical_path } => Event::ScanPath {
                item: *item,
                logical_path,
            },
            Self::ScanFinished { item, total_files } => Event::ScanFinished {
                item: *item,
                total_files: *total_files,
            },
            Self::PlanFinished { total_files } => Event::PlanFinished {
                total_files: *total_files,
            },
            Self::ItemStarted { item } => Event::ItemStarted { item: *item },
            Self::FileStarted {
                item,
                logical_path,
                src,
                dst,
            } => Event::FileStarted {
                item: *item,
                logical_path,
                src,
                dst,
            },
            Self::FileFinished {
                item,
                copied,
                bytes,
            } => Event::FileFinished {
                item: *item,
                copied: *copied,
                bytes: *bytes,
            },
            Self::ItemFinished { item, status } => Event::ItemFinished {
                item: *item,
                status: *status,
            },
            Self::Warning { item, message } => Event::Warning {
                item: *item,
                message,
            },
            Self::Error { item, message } => Event::Error {
                item: *item,
                message,
            },
            Self::Finish(summary) => Event::Finish(summary),
        }
    }
}

pub trait Reporter {
    fn report(&mut self, event: Event<'_>);
}

pub fn reporter(
    direction: Direction,
    layout: &ReportLayout,
    dry: bool,
    verbose: bool,
    no_progress: bool,
) -> Box<dyn Reporter> {
    let detailed = dry || verbose;
    // Crossterm's cursor-position query is issued through stdout even when the
    // Ratatui backend writes to stderr. Requiring both streams to be terminals
    // prevents that control sequence from leaking into redirected stdout.
    let wants_inline = should_use_inline(
        dry,
        verbose,
        no_progress,
        io::stderr().is_terminal(),
        io::stdout().is_terminal(),
    );
    let plain = PlainReporter::new(layout.clone(), detailed);

    if !wants_inline {
        return Box::new(plain);
    }

    let (width, height) = match ratatui::crossterm::terminal::size() {
        Ok(size) => size,
        Err(error) => {
            eprintln!("note: progress display unavailable: {error}");
            return Box::new(plain);
        }
    };

    if width < 30 || height < 2 {
        eprintln!("note: progress display unavailable: terminal is too small ({width}x{height})");
        return Box::new(plain);
    }

    match InlineReporter::new(direction, layout.clone(), width, height) {
        Ok(inline) => Box::new(FallbackReporter {
            inline: Some(inline),
            plain,
            started_inline: true,
        }),
        Err(error) => {
            eprintln!("note: progress display unavailable: {error}");
            Box::new(plain)
        }
    }
}

fn should_use_inline(
    dry: bool,
    verbose: bool,
    no_progress: bool,
    stderr_tty: bool,
    stdout_tty: bool,
) -> bool {
    stderr_tty && stdout_tty && !dry && !verbose && !no_progress
}

struct PlainReporter {
    layout: ReportLayout,
    printed_sections: HashSet<usize>,
    detailed: bool,
    print_final_report: bool,
    unicode_report: bool,
}

impl PlainReporter {
    fn new(layout: ReportLayout, detailed: bool) -> Self {
        Self {
            layout,
            printed_sections: HashSet::new(),
            detailed,
            print_final_report: false,
            unicode_report: false,
        }
    }
}

fn section_containing(layout: &ReportLayout, wanted: usize) -> Option<usize> {
    layout.sections.iter().position(|section| {
        section
            .roots
            .iter()
            .any(|root| node_contains(layout, *root, wanted))
    })
}

fn node_contains(layout: &ReportLayout, node: usize, wanted: usize) -> bool {
    node == wanted
        || layout.nodes[node]
            .children
            .iter()
            .any(|child| node_contains(layout, *child, wanted))
}

impl Reporter for PlainReporter {
    fn report(&mut self, event: Event<'_>) {
        match event {
            Event::ItemStarted { item } if self.detailed => {
                if let Some(owner) = self.layout.leaf_owners.get(item).copied()
                    && let Some(section) = section_containing(&self.layout, owner)
                    && self.printed_sections.insert(section)
                    && let Some(heading) = &self.layout.sections[section].heading
                {
                    println!("{heading}");
                }
                if let Some((name, depth)) = self.layout.leaf_label(item) {
                    println!("{}syncing: {name}", "  ".repeat(depth));
                }
            }
            Event::FileStarted { src, dst, .. } if self.detailed => {
                println!("  copy: {:?} -> {:?}", src, dst);
            }
            Event::Warning { message, .. } => eprintln!("  warning: {message}"),
            Event::Error { message, .. } => eprintln!("  error: {message}"),
            Event::Finish(summary) if summary.cancelled => {
                eprintln!("{}", format_summary(summary, self.unicode_report));
            }
            Event::Finish(summary) if self.print_final_report => {
                eprintln!("{}", format_summary(summary, self.unicode_report));
            }
            _ => {}
        }
    }
}

struct FallbackReporter {
    inline: Option<InlineReporter>,
    plain: PlainReporter,
    started_inline: bool,
}

impl Reporter for FallbackReporter {
    fn report(&mut self, event: Event<'_>) {
        if let Some(inline) = self.inline.as_mut() {
            match inline.report_event(event) {
                Ok(()) => return,
                Err(error) => {
                    if let Some(mut failed) = self.inline.take() {
                        failed.cleanup_best_effort();
                    }
                    eprintln!("note: progress display unavailable: {error}");
                    self.plain.print_final_report = self.started_inline;
                    self.plain.unicode_report = self.started_inline;
                }
            }
        }

        self.plain.report(event);
    }
}

type StderrTerminal = Terminal<CrosstermBackend<Stderr>>;

struct InlineReporter {
    terminal: StderrTerminal,
    state: ViewState,
    color: bool,
    last_draw: Instant,
    finished: bool,
    width: u16,
}

impl InlineReporter {
    fn new(
        direction: Direction,
        layout: ReportLayout,
        width: u16,
        terminal_height: u16,
    ) -> io::Result<Self> {
        let row_count = report_row_count(&layout);
        let desired_height = 1 + row_count.min(10) as u16;
        let viewport_height = desired_height.max(2).min(terminal_height);
        let backend = CrosstermBackend::new(io::stderr());
        let terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(viewport_height),
            },
        )?;
        let color = std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty());
        let mut reporter = Self {
            terminal,
            state: ViewState::with_layout(direction, layout),
            color,
            last_draw: Instant::now() - Duration::from_secs(1),
            finished: false,
            width,
        };
        reporter.terminal.hide_cursor()?;
        reporter.draw(true)?;
        Ok(reporter)
    }

    fn report_event(&mut self, event: Event<'_>) -> io::Result<()> {
        match event {
            Event::ScanStarted { item } => {
                self.state.current = Some(item);
                if let Some(view) = self.state.items.get_mut(item) {
                    view.status = ItemStatus::Scanning;
                }
                self.draw(true)
            }
            Event::ScanPath { item, logical_path } => {
                self.state.current = Some(item);
                self.state.spinner = self.state.spinner.wrapping_add(1);
                if let Some(view) = self.state.items.get_mut(item) {
                    view.current_path = display_path(logical_path);
                }
                self.draw(false)
            }
            Event::ScanFinished { item, total_files } => {
                if let Some(view) = self.state.items.get_mut(item) {
                    view.total_files = total_files;
                    view.status = if view.had_error {
                        ItemStatus::Failed
                    } else if view.had_warning {
                        ItemStatus::Warning
                    } else {
                        ItemStatus::Ready
                    };
                    view.current_path.clear();
                }
                self.state.scanned_items += 1;
                if self.state.current == Some(item) {
                    self.state.current = self
                        .state
                        .items
                        .iter()
                        .position(|view| view.status == ItemStatus::Scanning);
                }
                self.draw(true)
            }
            Event::PlanFinished { total_files } => {
                self.state.plan_complete = true;
                self.state.total_files = total_files;
                self.state.current = None;
                self.draw(true)
            }
            Event::ItemStarted { item } => {
                self.state.current = Some(item);
                if let Some(view) = self.state.items.get_mut(item) {
                    view.status = ItemStatus::Copying;
                }
                self.draw(true)
            }
            Event::FileStarted {
                item, logical_path, ..
            } => {
                self.state.current = Some(item);
                if let Some(view) = self.state.items.get_mut(item) {
                    view.current_path = display_path(logical_path);
                }
                self.draw(false)
            }
            Event::FileFinished {
                item,
                copied,
                bytes,
            } => {
                self.state.processed_files += 1;
                if copied {
                    self.state.copied_files += 1;
                    self.state.copied_bytes += bytes;
                }
                if let Some(view) = self.state.items.get_mut(item) {
                    view.processed_files += 1;
                    if copied {
                        view.copied_files += 1;
                        view.copied_bytes += bytes;
                    }
                }
                self.draw(false)
            }
            Event::ItemFinished { item, status } => {
                if let Some(view) = self.state.items.get_mut(item) {
                    view.status = status;
                    view.current_path.clear();
                }
                if self.state.current == Some(item) {
                    self.state.current = self
                        .state
                        .items
                        .iter()
                        .position(|view| view.status == ItemStatus::Copying);
                }
                self.draw(true)
            }
            Event::Warning { item, message } => {
                if let Some(item) = item
                    && let Some(view) = self.state.items.get_mut(item)
                {
                    view.had_warning = true;
                    if !matches!(view.status, ItemStatus::Scanning) {
                        view.status = ItemStatus::Warning;
                    }
                }
                self.insert_diagnostic("warning", message, ItemStatus::Warning)?;
                self.draw(true)
            }
            Event::Error { item, message } => {
                if let Some(item) = item
                    && let Some(view) = self.state.items.get_mut(item)
                {
                    view.had_error = true;
                    view.status = ItemStatus::Failed;
                }
                self.insert_diagnostic("error", message, ItemStatus::Failed)?;
                self.draw(true)
            }
            Event::Finish(summary) => self.finish(summary),
        }
    }

    fn draw(&mut self, force: bool) -> io::Result<()> {
        let now = Instant::now();
        if !force && now.duration_since(self.last_draw) < Duration::from_millis(50) {
            return Ok(());
        }
        let state = &self.state;
        let color = self.color;
        self.terminal
            .draw(|frame| render_progress(frame, state, color))?;
        self.last_draw = now;
        Ok(())
    }

    fn insert_diagnostic(
        &mut self,
        level: &str,
        message: &str,
        status: ItemStatus,
    ) -> io::Result<()> {
        let text = format!("  {level}: {message}");
        let width = usize::from(self.width.max(1));
        let height = (text.chars().count().max(1).div_ceil(width)) as u16;
        let style = status_style(status, self.color);
        self.terminal.insert_before(height.max(1), |buffer| {
            Paragraph::new(text)
                .style(style)
                .wrap(Wrap { trim: false })
                .render(buffer.area, buffer);
        })?;
        Ok(())
    }

    fn finish(&mut self, summary: &Summary) -> io::Result<()> {
        if summary.cancelled {
            for view in &mut self.state.items {
                if matches!(view.status, ItemStatus::Scanning | ItemStatus::Copying) {
                    view.status = ItemStatus::Cancelled;
                    view.current_path.clear();
                }
            }
        }
        self.draw(true)?;

        let item_lines: Vec<_> = report_rows(&self.state)
            .into_iter()
            .map(|row| match row {
                RenderRow::Heading(heading) => (heading, active_style(self.color)),
                RenderRow::Item { view, .. } => (
                    format_item_report(&view, self.width),
                    status_style(view.status, self.color),
                ),
            })
            .collect();
        let summary_text = format_summary(summary, true);
        let summary_style = summary_style(summary, self.color);
        let width = usize::from(self.width.max(1));
        let summary_height = summary_text.chars().count().max(1).div_ceil(width) as u16;
        let item_count = u16::try_from(item_lines.len()).unwrap_or(u16::MAX);
        let report_height = item_count.saturating_add(summary_height.max(1));

        self.terminal.insert_before(report_height, |buffer| {
            let area = buffer.area;
            for (row, (text, style)) in item_lines.into_iter().enumerate() {
                if row as u16 >= area.height.saturating_sub(summary_height) {
                    break;
                }
                Paragraph::new(text).style(style).render(
                    Rect::new(area.x, area.y.saturating_add(row as u16), area.width, 1),
                    buffer,
                );
            }
            Paragraph::new(summary_text)
                .style(summary_style)
                .wrap(Wrap { trim: false })
                .render(
                    Rect::new(
                        area.x,
                        area.y.saturating_add(
                            item_count.min(area.height.saturating_sub(summary_height)),
                        ),
                        area.width,
                        summary_height.min(area.height),
                    ),
                    buffer,
                );
        })?;

        self.cleanup()?;
        Ok(())
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.terminal.clear()?;
        let area = self.terminal.get_frame().area();
        self.terminal
            .set_cursor_position(Position::new(area.x, area.y))?;
        self.terminal.show_cursor()?;
        self.terminal.backend_mut().flush()?;
        self.finished = true;
        Ok(())
    }

    fn cleanup_best_effort(&mut self) {
        let _ = self.cleanup();
    }
}

impl Drop for InlineReporter {
    fn drop(&mut self) {
        self.cleanup_best_effort();
    }
}

#[derive(Clone, Debug)]
struct ViewState {
    direction: Direction,
    layout: ReportLayout,
    items: Vec<ItemView>,
    current: Option<usize>,
    spinner: usize,
    scanned_items: usize,
    plan_complete: bool,
    total_files: usize,
    processed_files: usize,
    copied_files: usize,
    copied_bytes: u64,
    name_width: u16,
}

impl ViewState {
    #[cfg(test)]
    fn new(direction: Direction, names: Vec<String>) -> Self {
        let mut layout = ReportLayout::default();
        let mut roots = Vec::new();
        for (leaf, name) in names.into_iter().enumerate() {
            let node = layout.nodes.len();
            layout.nodes.push(ReportNode {
                origin: name.clone(),
                label: name,
                depth: 0,
                children: Vec::new(),
                leaf: Some(leaf),
                duplicate_of: None,
            });
            layout.leaf_owners.push(node);
            roots.push(node);
        }
        layout.sections.push(ReportSection {
            heading: None,
            roots,
        });
        Self::with_layout(direction, layout)
    }

    fn with_layout(direction: Direction, layout: ReportLayout) -> Self {
        let item_count = layout.leaf_owners.len();
        let name_width = layout
            .nodes
            .iter()
            .map(|node| node.label.chars().count() + node.depth * 2)
            .max()
            .unwrap_or(10)
            .clamp(10, 30) as u16;
        Self {
            direction,
            layout,
            items: (0..item_count)
                .map(|index| ItemView::new(format!("item-{index}")))
                .collect(),
            current: None,
            spinner: 0,
            scanned_items: 0,
            plan_complete: false,
            total_files: 0,
            processed_files: 0,
            copied_files: 0,
            copied_bytes: 0,
            name_width,
        }
    }
}

#[derive(Clone, Debug)]
struct ItemView {
    name: String,
    status: ItemStatus,
    total_files: usize,
    processed_files: usize,
    copied_files: usize,
    copied_bytes: u64,
    current_path: String,
    had_warning: bool,
    had_error: bool,
    total_items: usize,
    completed_items: usize,
    branch: bool,
    duplicate: bool,
}

impl ItemView {
    fn new(name: String) -> Self {
        Self {
            name,
            status: ItemStatus::Pending,
            total_files: 0,
            processed_files: 0,
            copied_files: 0,
            copied_bytes: 0,
            current_path: String::new(),
            had_warning: false,
            had_error: false,
            total_items: 0,
            completed_items: 0,
            branch: false,
            duplicate: false,
        }
    }
}

#[derive(Clone, Debug)]
enum RenderRow {
    Heading(String),
    Item { node: usize, view: ItemView },
}

fn report_row_count(layout: &ReportLayout) -> usize {
    layout
        .sections
        .iter()
        .map(|section| {
            usize::from(section.heading.is_some())
                + section
                    .roots
                    .iter()
                    .map(|root| node_count(layout, *root))
                    .sum::<usize>()
        })
        .sum()
}

fn node_count(layout: &ReportLayout, node: usize) -> usize {
    1 + layout.nodes[node]
        .children
        .iter()
        .map(|child| node_count(layout, *child))
        .sum::<usize>()
}

fn report_rows(state: &ViewState) -> Vec<RenderRow> {
    let mut rows = Vec::new();
    for section in &state.layout.sections {
        if let Some(heading) = &section.heading {
            rows.push(RenderRow::Heading(heading.clone()));
        }
        for root in &section.roots {
            push_node_rows(state, *root, &mut rows);
        }
    }
    rows
}

fn push_node_rows(state: &ViewState, node: usize, rows: &mut Vec<RenderRow>) {
    rows.push(RenderRow::Item {
        node,
        view: node_view(state, node),
    });
    for child in &state.layout.nodes[node].children {
        push_node_rows(state, *child, rows);
    }
}

fn node_view(state: &ViewState, node_index: usize) -> ItemView {
    let node = &state.layout.nodes[node_index];
    let indent = "  ".repeat(node.depth);
    if let Some(leaf) = node.leaf {
        let mut view = state
            .items
            .get(leaf)
            .cloned()
            .unwrap_or_else(|| ItemView::new(node.label.clone()));
        view.name = if let Some(owner) = node.duplicate_of {
            view.total_files = 0;
            view.processed_files = 0;
            view.copied_files = 0;
            view.copied_bytes = 0;
            view.duplicate = true;
            format!(
                "{indent}{}  duplicate → {}",
                node.label, state.layout.nodes[owner].origin
            )
        } else {
            format!("{indent}{}", node.label)
        };
        return view;
    }

    let children: Vec<ItemView> = node
        .children
        .iter()
        .map(|child| node_view(state, *child))
        .collect();
    let mut view = ItemView::new(format!("{indent}{}", node.label));
    view.branch = true;
    view.status = aggregate_status(&children);
    for child in &children {
        if child.duplicate {
            continue;
        }
        view.total_items += 1 + child.total_items;
        view.completed_items += usize::from(is_terminal(child.status)) + child.completed_items;
        view.total_files += child.total_files;
        view.processed_files += child.processed_files;
        view.copied_files += child.copied_files;
        view.copied_bytes += child.copied_bytes;
        view.had_warning |= child.had_warning;
        view.had_error |= child.had_error;
        if view.current_path.is_empty() && !child.current_path.is_empty() {
            view.current_path = child.current_path.clone();
        }
    }
    view
}

fn aggregate_status(children: &[ItemView]) -> ItemStatus {
    for status in [
        ItemStatus::Scanning,
        ItemStatus::Copying,
        ItemStatus::Pending,
        ItemStatus::Ready,
        ItemStatus::Failed,
        ItemStatus::Warning,
        ItemStatus::Skipped,
        ItemStatus::Cancelled,
        ItemStatus::Done,
    ] {
        if children.iter().any(|child| child.status == status) {
            return status;
        }
    }
    ItemStatus::Done
}

fn is_terminal(status: ItemStatus) -> bool {
    matches!(
        status,
        ItemStatus::Done
            | ItemStatus::Warning
            | ItemStatus::Skipped
            | ItemStatus::Failed
            | ItemStatus::Cancelled
    )
}

fn render_progress(frame: &mut Frame<'_>, state: &ViewState, color: bool) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }

    let rows = report_rows(state);
    let visible_rows = usize::from(area.height.saturating_sub(1)).min(10);
    render_overall(
        frame,
        Rect::new(area.x, area.y, area.width, 1),
        state,
        visible_rows.min(rows.len()),
        color,
    );

    if visible_rows == 0 || rows.is_empty() {
        return;
    }
    let current = state.current.and_then(|leaf| {
        let owner = *state.layout.leaf_owners.get(leaf)?;
        rows.iter()
            .position(|row| matches!(row, RenderRow::Item { node, .. } if *node == owner))
    });
    let (start, end) = visible_window(rows.len(), visible_rows, current);
    for (row, rendered) in rows[start..end].iter().enumerate() {
        let rect = Rect::new(area.x, area.y.saturating_add(1 + row as u16), area.width, 1);
        match rendered {
            RenderRow::Heading(heading) => {
                frame.render_widget(
                    Paragraph::new(heading.as_str()).style(active_style(color)),
                    rect,
                );
            }
            RenderRow::Item { view, .. } => {
                render_item(frame, rect, view, state.name_width, state.spinner, color)
            }
        }
    }
}

fn render_overall(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &ViewState,
    visible_rows: usize,
    color: bool,
) {
    let hidden = report_row_count(&state.layout).saturating_sub(visible_rows);
    if !state.plan_complete {
        let spinner = spinner(state.spinner);
        let suffix = if hidden > 0 {
            format!(" · {hidden} hidden")
        } else {
            String::new()
        };
        let text = format!(
            "{spinner} {} scanning {}/{} items{suffix}",
            state.direction.label(),
            state.scanned_items,
            state.items.len()
        );
        frame.render_widget(
            Paragraph::new(text).style(status_style(ItemStatus::Scanning, color)),
            area,
        );
        return;
    }

    if area.width < 55 {
        let text = format!(
            "{} {}/{} files {}",
            state.direction.label(),
            state.copied_files,
            state.total_files,
            format_bytes(state.copied_bytes)
        );
        frame.render_widget(Paragraph::new(text), area);
        return;
    }

    let chunks = Layout::horizontal([
        Constraint::Length(6),
        Constraint::Min(8),
        Constraint::Length(11),
        Constraint::Length(10),
        Constraint::Length(if hidden > 0 { 12 } else { 0 }),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(state.direction.label()).style(active_style(color)),
        chunks[0],
    );
    let ratio = progress_ratio(state.processed_files, state.total_files);
    frame.render_widget(line_gauge(ratio, ItemStatus::Copying, color), chunks[1]);
    frame.render_widget(
        Paragraph::new(format!(
            "{}/{} files",
            state.copied_files, state.total_files
        )),
        chunks[2],
    );
    frame.render_widget(Paragraph::new(format_bytes(state.copied_bytes)), chunks[3]);
    if hidden > 0 {
        frame.render_widget(Paragraph::new(format!("{hidden} hidden")), chunks[4]);
    }
}

fn render_item(
    frame: &mut Frame<'_>,
    area: Rect,
    item: &ItemView,
    name_width: u16,
    spinner_index: usize,
    color: bool,
) {
    let symbol = status_symbol(item.status, spinner_index);
    let count = if item.branch {
        format!(
            "{}/{} items · {}/{} files",
            item.completed_items, item.total_items, item.copied_files, item.total_files
        )
    } else if item.duplicate {
        "duplicate".to_string()
    } else {
        format!("{}/{}", item.copied_files, item.total_files)
    };
    let status = item.status.label();
    let style = status_style(item.status, color);

    if area.width < 55 {
        let fixed = 2 + count.len() + status.len() + 3;
        let name_width = usize::from(area.width).saturating_sub(fixed).max(1);
        let text = format!(
            "{symbol} {} {count} {status}",
            truncate(&item.name, name_width)
        );
        frame.render_widget(Paragraph::new(text).style(style), area);
        return;
    }

    let show_bytes = area.width >= 80;
    let show_path = area.width >= 100;
    let mut constraints = vec![
        Constraint::Length(2),
        Constraint::Length(name_width),
        Constraint::Min(8),
        Constraint::Length(u16::try_from(count.len()).unwrap_or(u16::MAX).max(8)),
    ];
    if show_bytes {
        constraints.push(Constraint::Length(10));
    }
    constraints.push(Constraint::Length(10));
    if show_path {
        constraints.push(Constraint::Length(24));
    }
    let chunks = Layout::horizontal(constraints).split(area);
    let mut chunk = 0;
    frame.render_widget(Paragraph::new(symbol).style(style), chunks[chunk]);
    chunk += 1;
    frame.render_widget(
        Paragraph::new(item.name.as_str()).style(style),
        chunks[chunk],
    );
    chunk += 1;
    frame.render_widget(
        line_gauge(
            progress_ratio(item.processed_files, item.total_files),
            item.status,
            color,
        ),
        chunks[chunk],
    );
    chunk += 1;
    frame.render_widget(Paragraph::new(count), chunks[chunk]);
    chunk += 1;
    if show_bytes {
        frame.render_widget(
            Paragraph::new(format_bytes(item.copied_bytes)),
            chunks[chunk],
        );
        chunk += 1;
    }
    frame.render_widget(Paragraph::new(status).style(style), chunks[chunk]);
    chunk += 1;
    if show_path {
        frame.render_widget(
            Paragraph::new(truncate(&item.current_path, 24)),
            chunks[chunk],
        );
    }
}

fn line_gauge(ratio: f64, status: ItemStatus, color: bool) -> LineGauge<'static> {
    LineGauge::default()
        .ratio(ratio)
        .filled_style(status_style(status, color))
        .unfilled_style(if color {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        })
        .filled_symbol("━")
        .unfilled_symbol("─")
        .label("")
}

fn progress_ratio(processed: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (processed as f64 / total as f64).clamp(0.0, 1.0)
    }
}

fn visible_window(total: usize, visible: usize, current: Option<usize>) -> (usize, usize) {
    if total <= visible {
        return (0, total);
    }
    let current = current.unwrap_or(0).min(total - 1);
    let start = current
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible));
    (start, start + visible)
}

fn status_symbol(status: ItemStatus, spinner_index: usize) -> &'static str {
    match status {
        ItemStatus::Pending | ItemStatus::Ready => "·",
        ItemStatus::Scanning => spinner(spinner_index),
        ItemStatus::Copying => "›",
        ItemStatus::Done => "✓",
        ItemStatus::Warning | ItemStatus::Skipped => "⚠",
        ItemStatus::Failed => "✗",
        ItemStatus::Cancelled => "⨯",
    }
}

fn spinner(index: usize) -> &'static str {
    const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[index % FRAMES.len()]
}

fn active_style(color: bool) -> Style {
    if color {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

fn status_style(status: ItemStatus, color: bool) -> Style {
    if !color {
        return Style::default();
    }
    let color = match status {
        ItemStatus::Done => Color::Green,
        ItemStatus::Warning | ItemStatus::Skipped => Color::Yellow,
        ItemStatus::Failed | ItemStatus::Cancelled => Color::Red,
        ItemStatus::Scanning | ItemStatus::Copying => Color::Cyan,
        ItemStatus::Pending | ItemStatus::Ready => Color::DarkGray,
    };
    Style::default().fg(color)
}

fn summary_style(summary: &Summary, color: bool) -> Style {
    let status = if summary.cancelled || summary.errors > 0 {
        ItemStatus::Failed
    } else if summary.warnings > 0 {
        ItemStatus::Warning
    } else {
        ItemStatus::Done
    };
    status_style(status, color)
}

fn format_item_report(item: &ItemView, width: u16) -> String {
    let symbol = status_symbol(item.status, 0);
    let suffix = if item.duplicate {
        format!("  {}", item.status.label())
    } else if item.branch {
        format!(
            "  {}/{} items  {}/{} files  {}  {}",
            item.completed_items,
            item.total_items,
            item.copied_files,
            item.total_files,
            format_bytes(item.copied_bytes),
            item.status.label()
        )
    } else if width >= 55 {
        format!(
            "  {}/{} files  {}  {}",
            item.copied_files,
            item.total_files,
            format_bytes(item.copied_bytes),
            item.status.label()
        )
    } else {
        format!(
            "  {}/{}  {}",
            item.copied_files,
            item.total_files,
            item.status.label()
        )
    };
    let fixed_width = symbol.chars().count() + 1 + suffix.chars().count();
    let name_width = usize::from(width).saturating_sub(fixed_width).max(1);
    format!("{symbol} {}{suffix}", truncate(&item.name, name_width))
}

pub fn format_summary(summary: &Summary, unicode: bool) -> String {
    let symbol = if summary.cancelled {
        if unicode {
            "⨯ cancelled"
        } else {
            "cancelled"
        }
    } else if summary.errors > 0 {
        if unicode { "✗" } else { "error:" }
    } else if summary.warnings > 0 {
        if unicode { "⚠" } else { "warning:" }
    } else if unicode {
        "✓"
    } else {
        "ok:"
    };
    let elapsed = format_duration(summary.elapsed);

    if summary.cancelled_during_planning {
        return format!(
            "{symbol} {} during planning: {}/{} items scanned, {} files discovered in {elapsed}{}",
            summary.direction.label(),
            summary.scanned_items,
            summary.total_items,
            summary.discovered_files,
            diagnostic_suffix(summary)
        );
    }

    if summary.cancelled {
        return format!(
            "{symbol} {}: {}/{} items, {}/{} files, {} in {elapsed}{}",
            summary.direction.label(),
            summary.completed_items,
            summary.total_items,
            summary.copied_files,
            summary.planned_files,
            format_bytes(summary.copied_bytes),
            diagnostic_suffix(summary)
        );
    }

    if summary.warnings == 0 && summary.errors == 0 {
        return format!(
            "{symbol} {}: {} {}, {} {}, {} in {elapsed}",
            summary.direction.label(),
            summary.total_items,
            plural(summary.total_items, "item", "items"),
            summary.copied_files,
            plural(summary.copied_files, "file", "files"),
            format_bytes(summary.copied_bytes)
        );
    }

    let mut states = vec![format!("{} done", summary.done_items)];
    if summary.warning_items > 0 {
        states.push(format!("{} warning", summary.warning_items));
    }
    if summary.skipped_items > 0 {
        states.push(format!("{} skipped", summary.skipped_items));
    }
    if summary.failed_items > 0 {
        states.push(format!("{} failed", summary.failed_items));
    }

    format!(
        "{symbol} {}: {} items ({}), {}/{} files, {} in {elapsed}{}",
        summary.direction.label(),
        summary.total_items,
        states.join(", "),
        summary.copied_files,
        summary.planned_files,
        format_bytes(summary.copied_bytes),
        diagnostic_suffix(summary)
    )
}

fn diagnostic_suffix(summary: &Summary) -> String {
    if summary.warnings == 0 && summary.errors == 0 {
        return String::new();
    }
    format!(
        " — {} {}, {} {}",
        summary.warnings,
        plural(summary.warnings, "warning", "warnings"),
        summary.errors,
        plural(summary.errors, "error", "errors")
    )
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() >= 10 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{:.1}s", duration.as_secs_f64())
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn truncate(value: &str, width: usize) -> String {
    let count = value.chars().count();
    if count <= width {
        return value.to_string();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let mut output: String = value.chars().take(width - 1).collect();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> Summary {
        Summary {
            direction: Direction::Put,
            total_items: 2,
            completed_items: 2,
            done_items: 2,
            warning_items: 0,
            skipped_items: 0,
            failed_items: 0,
            copied_files: 3,
            planned_files: 3,
            copied_bytes: 1536,
            warnings: 0,
            errors: 0,
            elapsed: Duration::from_millis(200),
            cancelled: false,
            cancelled_during_planning: false,
            scanned_items: 2,
            discovered_files: 3,
        }
    }

    #[test]
    fn mode_selection_requires_a_normal_tty_sync() {
        assert!(should_use_inline(false, false, false, true, true));
        assert!(!should_use_inline(true, false, false, true, true));
        assert!(!should_use_inline(false, true, false, true, true));
        assert!(!should_use_inline(false, false, true, true, true));
        assert!(!should_use_inline(false, false, false, false, true));
        assert!(!should_use_inline(false, false, false, true, false));
    }

    #[test]
    fn formats_completed_item_report() {
        let mut item = ItemView::new("editors/neovim".into());
        item.status = ItemStatus::Done;
        item.total_files = 3;
        item.processed_files = 3;
        item.copied_files = 3;
        item.copied_bytes = 1536;
        assert_eq!(
            format_item_report(&item, 100),
            "✓ editors/neovim  3/3 files  1.5 KiB  done"
        );
    }

    #[test]
    fn formats_clean_summary() {
        assert_eq!(
            format_summary(&summary(), true),
            "✓ put: 2 items, 3 files, 1.5 KiB in 0.2s"
        );
    }

    #[test]
    fn formats_non_clean_summary_with_item_breakdown() {
        let mut summary = summary();
        summary.done_items = 0;
        summary.warning_items = 1;
        summary.failed_items = 1;
        summary.copied_files = 2;
        summary.planned_files = 3;
        summary.warnings = 1;
        summary.errors = 1;
        assert_eq!(
            format_summary(&summary, true),
            "✗ put: 2 items (0 done, 1 warning, 1 failed), 2/3 files, 1.5 KiB in 0.2s — 1 warning, 1 error"
        );
    }

    #[test]
    fn formats_cancelled_planning_summary() {
        let mut summary = summary();
        summary.cancelled = true;
        summary.cancelled_during_planning = true;
        summary.scanned_items = 1;
        summary.discovered_files = 2;
        assert_eq!(
            format_summary(&summary, false),
            "cancelled put during planning: 1/2 items scanned, 2 files discovered in 0.2s"
        );
    }

    #[test]
    fn visible_window_keeps_current_item_in_view() {
        assert_eq!(visible_window(20, 10, Some(0)), (0, 10));
        assert_eq!(visible_window(20, 10, Some(10)), (5, 15));
        assert_eq!(visible_window(20, 10, Some(19)), (10, 20));
    }

    #[test]
    fn reports_rows_hidden_by_the_available_height() {
        let backend = ratatui::backend::TestBackend::new(70, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = ViewState::new(
            Direction::Put,
            (0..10).map(|index| format!("item-{index}")).collect(),
        );
        terminal
            .draw(|frame| render_progress(frame, &state, false))
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("7 hidden"));
    }

    #[test]
    fn aggregates_authored_tree_rows() {
        let layout = ReportLayout {
            sections: vec![ReportSection {
                heading: Some("nested.yaml".into()),
                roots: vec![0],
            }],
            nodes: vec![
                ReportNode {
                    label: "$HOME/.config/app → stored/app".into(),
                    origin: "nested.yaml:$HOME/.config/app".into(),
                    depth: 0,
                    children: vec![1, 2],
                    leaf: None,
                    duplicate_of: None,
                },
                ReportNode {
                    label: "one".into(),
                    origin: "nested.yaml:one".into(),
                    depth: 1,
                    children: vec![],
                    leaf: Some(0),
                    duplicate_of: None,
                },
                ReportNode {
                    label: "two".into(),
                    origin: "nested.yaml:two".into(),
                    depth: 1,
                    children: vec![],
                    leaf: Some(1),
                    duplicate_of: None,
                },
            ],
            leaf_owners: vec![1, 2],
        };
        let mut state = ViewState::with_layout(Direction::Put, layout);
        state.items[0].status = ItemStatus::Done;
        state.items[0].total_files = 2;
        state.items[0].copied_files = 2;
        state.items[1].status = ItemStatus::Warning;
        state.items[1].total_files = 1;

        let rows = report_rows(&state);
        assert_eq!(rows.len(), 4);
        let RenderRow::Item { view, .. } = &rows[1] else {
            panic!("expected branch row");
        };
        assert!(view.branch);
        assert_eq!(view.total_items, 2);
        assert_eq!(view.completed_items, 2);
        assert_eq!(view.total_files, 3);
        assert_eq!(view.copied_files, 2);
        assert_eq!(view.status, ItemStatus::Warning);
    }

    #[test]
    fn duplicate_rows_share_status_without_metrics() {
        let layout = ReportLayout {
            sections: vec![ReportSection {
                heading: Some("duplicates.yaml".into()),
                roots: vec![0, 1],
            }],
            nodes: vec![
                ReportNode {
                    label: "first".into(),
                    origin: "duplicates.yaml:first".into(),
                    depth: 0,
                    children: vec![],
                    leaf: Some(0),
                    duplicate_of: None,
                },
                ReportNode {
                    label: "second".into(),
                    origin: "duplicates.yaml:second".into(),
                    depth: 0,
                    children: vec![],
                    leaf: Some(0),
                    duplicate_of: Some(0),
                },
            ],
            leaf_owners: vec![0],
        };
        let mut state = ViewState::with_layout(Direction::Put, layout);
        state.items[0].status = ItemStatus::Done;
        state.items[0].total_files = 3;
        state.items[0].copied_files = 3;

        let rows = report_rows(&state);
        let RenderRow::Item { view, .. } = &rows[2] else {
            panic!("expected duplicate row");
        };
        assert!(view.duplicate);
        assert_eq!(view.status, ItemStatus::Done);
        assert_eq!(view.total_files, 0);
        assert!(view.name.contains("duplicate → duplicates.yaml:first"));
    }

    #[test]
    fn renders_into_a_narrow_test_backend() {
        let backend = ratatui::backend::TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = ViewState::new(Direction::Get, vec!["editors/neovim".into()]);
        state.plan_complete = true;
        state.total_files = 4;
        state.items[0].status = ItemStatus::Copying;
        state.items[0].total_files = 4;
        state.items[0].processed_files = 2;
        state.items[0].copied_files = 2;
        terminal
            .draw(|frame| render_progress(frame, &state, false))
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("get 0/4 files"));
        assert!(rendered.contains("editors/neovim"));
        assert!(rendered.contains("2/4"));
        assert!(rendered.contains("copying"));
    }
}
