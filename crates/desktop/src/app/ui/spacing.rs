//! Named spacing values shared across desktop screens (egui layout rhythm).

/// Tight line gap inside a section (labels, key/value rows).
pub const LINE: f32 = 6.0;

/// Gap after view toggles / before main scroll content (matches prior `INFO_SUBSECTION_SPACING`).
pub const SUBSECTION: f32 = 18.0;

/// Horizontal gap between dashboard columns and vertical gap between stacked dashboard boxes.
pub const DASHBOARD_COLUMN_GAP: f32 = 24.0;

/// Inner padding for `Frame::group` dashboard boxes.
pub const GROUP_INNER_MARGIN: f32 = 12.0;

/// Space between dashboard box title and the separator line under it.
pub const GROUP_TITLE_AFTER: f32 = 6.0;

/// Space between that separator and the box body (below the horizontal line).
pub const GROUP_AFTER_SEPARATOR: f32 = 8.0;

/// Vertical gap above and below a subsection label (e.g. Workers, backend name) before its body
/// (grid or wrapped list).
pub const SUBSECTION_HEADING_GAP: f32 = 8.0;

/// Space after a dashboard grid (or similar block) before the next distinct block of content.
pub const TABLE_BLOCK_AFTER: f32 = 18.0;

/// Space **above** a [`egui::CollapsingHeader`] so it reads as a new block (e.g. after a workers
/// table). The header row itself sits flush to prior content without this.
pub const COLLAPSING_HEADER_BEFORE: f32 = 12.0;

/// Vertical inset inside an expanded [`egui::CollapsingHeader`] body: below the clickable header
/// row and below nested content. Same value as [`COLLAPSING_HEADER_BEFORE`].
pub const COLLAPSING_BODY_INSET: f32 = COLLAPSING_HEADER_BEFORE;

/// Space after a key/value row.
pub const KV_AFTER: f32 = 6.0;

/// Horizontal gap between the key column and the value in [`super::dashboard::kv`].
pub const KV_KEY_VALUE_GAP: f32 = 8.0;

/// Column and row gaps for [`egui::Grid`] tables — same value on both axes.
pub const GRID_CELL_SPACING: f32 = 16.0;

/// Left/right padding inside each striped [`egui::Grid`] cell ([`super::dashboard::grid_cell`]) so
/// text is inset from the alternating row background edge.
pub const GRID_CELL_INNER_PAD_X: f32 = 8.0;

/// Minimum width for the key column in [`super::dashboard::kv`] so values align vertically.
/// Sized for long labels (e.g. status date field) at default body text.
pub const KV_LABEL_COLUMN_WIDTH: f32 = 260.0;

/// Horizontal padding for central panel content (matches sidebar/header rhythm).
pub const CENTRAL_PANEL_H_MARGIN: f32 = 24.0;
