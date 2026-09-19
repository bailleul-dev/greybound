//! Standard pedal enclosure formats and the constrained control grid.
//!
//! Every pedal is drawn at one shared physical scale
//! ([`PEDAL_LOGICAL_PX_PER_MM`]), so a compact pedal is visibly smaller than a
//! wide one on the board instead of being stretched to the same slot. Controls
//! on [`RenderLayoutPolicy::PedalGrid`] pedals are placed with the `grid_*`
//! builders below: their anchors come from the format's rows and columns and
//! their sizes come from a small set of tiers, so no pedal carries hand-tuned
//! coordinates or radii.
//!
//! [`validate_pedal_layout`] is the single geometry gate shared by the UI tests
//! and `cargo xtask pedal-assets`.

use crate::{
    ControlKind, KnobSkin, ModelRenderSpec, RenderControlAssetSpec, RenderControlRole,
    RenderControlSpec, RenderControlWidget, RenderLayoutPolicy, RenderSurfaceKind,
    RenderSurfaceSpec,
};

/// Logical pixels per millimetre of enclosure, shared by every pedal format.
pub const PEDAL_LOGICAL_PX_PER_MM: f32 = 4.0;
/// Source PNG pixels per logical pixel for pedal faceplates (`@4x`).
pub const PEDAL_SOURCE_SCALE: f32 = 4.0;
/// Transparent margin around the enclosure body, in logical pixels. It holds
/// the enclosure's own soft shadow and the side jacks.
pub const PEDAL_CANVAS_MARGIN: f32 = 18.0;

/// Label baseline distance below a control centre, in logical pixels.
pub const PEDAL_LABEL_BASELINE_OFFSET: f32 = 44.25;
/// Minimum gap between a control edge and its label baseline.
pub const PEDAL_LABEL_CLEARANCE: f32 = 20.0;
/// Cap height of an engraved control label, in logical pixels.
pub const PEDAL_LABEL_HEIGHT: f32 = 8.0;
const PEDAL_LABEL_ADVANCE: f32 = 7.5;
const PEDAL_LABEL_SIDE_PADDING: f32 = 4.5;
/// Minimum clearance between any two components, labels, and the body edge.
pub const PEDAL_COMPONENT_CLEARANCE: f32 = 3.0;

pub const PEDAL_FOOTSWITCH_RADIUS: f32 = 31.0;
pub const PEDAL_FOOTSWITCH_HIT_RADIUS: f32 = 50.0;
pub const PEDAL_LED_RADIUS: f32 = 16.0;

const ANCHOR_TOLERANCE: f32 = 0.0005;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedalFormat {
    /// Mini enclosure, Hammond 1590A class. One or two small knobs.
    Compact,
    /// Classic single-footswitch enclosure, Hammond 125B class.
    Standard,
    /// Wide enclosure, Hammond 1590BB class. Up to four knobs per row.
    Wide,
}

impl PedalFormat {
    pub const ALL: [PedalFormat; 3] = [Self::Compact, Self::Standard, Self::Wide];

    pub const fn spec(self) -> &'static PedalFormatSpec {
        match self {
            Self::Compact => &COMPACT_PEDAL_FORMAT,
            Self::Standard => &STANDARD_PEDAL_FORMAT,
            Self::Wide => &WIDE_PEDAL_FORMAT,
        }
    }

    pub const fn id(self) -> &'static str {
        self.spec().id
    }
}

/// Knob sizes. Diameters are 11, 15, and 19 mm at the shared scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobTier {
    Small,
    Medium,
    Large,
}

impl KnobTier {
    pub const ALL: [KnobTier; 3] = [Self::Small, Self::Medium, Self::Large];

    pub const fn radius(self) -> f32 {
        match self {
            Self::Small => 22.0,
            Self::Medium => 30.0,
            Self::Large => 38.0,
        }
    }

    pub const fn hit_radius(self) -> f32 {
        match self {
            Self::Small => 40.0,
            Self::Medium => 48.0,
            Self::Large => 56.0,
        }
    }
}

/// Horizontal control rows, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedalRow {
    A,
    B,
    C,
}

impl PedalRow {
    const fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
            Self::C => 2,
        }
    }
}

/// Mechanical description of one enclosure format.
///
/// Positions (`rows`, `led_y`, `footswitch_y`, `jack_y`) are logical pixels
/// measured from the top-left corner of the enclosure body, not the canvas.
#[derive(Debug, Clone, Copy)]
pub struct PedalFormatSpec {
    pub format: PedalFormat,
    pub id: &'static str,
    /// Real enclosure class the proportions are taken from.
    pub reference_enclosure: &'static str,
    pub body_width: f32,
    pub body_height: f32,
    pub surface: RenderSurfaceSpec,
    pub max_columns: u8,
    pub rows: &'static [f32],
    pub led_y: f32,
    pub footswitch_y: f32,
    /// Centre of the side input/output jacks.
    pub jack_y: f32,
}

impl PedalFormatSpec {
    pub const fn body_width_mm(&self) -> f32 {
        self.body_width / PEDAL_LOGICAL_PX_PER_MM
    }

    pub const fn body_height_mm(&self) -> f32 {
        self.body_height / PEDAL_LOGICAL_PX_PER_MM
    }

    /// Enclosure body bounds on the canvas: `(left, top, right, bottom)`.
    pub const fn body_bounds(&self) -> (f32, f32, f32, f32) {
        (
            PEDAL_CANVAS_MARGIN,
            PEDAL_CANVAS_MARGIN,
            PEDAL_CANVAS_MARGIN + self.body_width,
            PEDAL_CANVAS_MARGIN + self.body_height,
        )
    }

    /// Normalized canvas anchor of a body-relative point.
    pub const fn anchor(&self, body_x: f32, body_y: f32) -> (f32, f32) {
        (
            (PEDAL_CANVAS_MARGIN + body_x) / self.surface.logical_width,
            (PEDAL_CANVAS_MARGIN + body_y) / self.surface.logical_height,
        )
    }

    pub const fn slot_anchor(&self, row: PedalRow, columns: u8, index: u8) -> (f32, f32) {
        if columns == 0 || columns > self.max_columns {
            panic!("pedal grid: this format does not support that many columns");
        }
        if index >= columns {
            panic!("pedal grid: column index out of range");
        }
        if row.index() >= self.rows.len() {
            panic!("pedal grid: this format does not have that row");
        }
        self.anchor(
            self.body_width * column_fraction(columns, index),
            self.rows[row.index()],
        )
    }

    pub const fn led_anchor(&self) -> (f32, f32) {
        self.anchor(self.body_width * 0.5, self.led_y)
    }

    pub const fn footswitch_anchor(&self) -> (f32, f32) {
        self.anchor(self.body_width * 0.5, self.footswitch_y)
    }

    pub const fn jack_anchor_y(&self) -> f32 {
        self.anchor(0.0, self.jack_y).1
    }
}

/// Column centres as a fraction of the body width.
pub const fn column_fraction(columns: u8, index: u8) -> f32 {
    match (columns, index) {
        (1, 0) => 0.5,
        (2, 0) => 0.25,
        (2, 1) => 0.75,
        (3, 0) => 0.2,
        (3, 1) => 0.5,
        (3, 2) => 0.8,
        (4, 0) => 0.125,
        (4, 1) => 0.375,
        (4, 2) => 0.625,
        (4, 3) => 0.875,
        _ => panic!("pedal grid: unsupported column layout"),
    }
}

const fn pedal_surface(body_width: f32, body_height: f32) -> RenderSurfaceSpec {
    let logical_width = body_width + PEDAL_CANVAS_MARGIN * 2.0;
    let logical_height = body_height + PEDAL_CANVAS_MARGIN * 2.0;
    RenderSurfaceSpec {
        kind: RenderSurfaceKind::Pedal,
        logical_width,
        logical_height,
        recommended_pixel_width: (logical_width * PEDAL_SOURCE_SCALE) as u32,
        recommended_pixel_height: (logical_height * PEDAL_SOURCE_SCALE) as u32,
        transparent_background: true,
    }
}

/// 39 x 93 mm body; 192 x 408 logical canvas; 768 x 1632 source PNG.
pub const COMPACT_PEDAL_FORMAT: PedalFormatSpec = PedalFormatSpec {
    format: PedalFormat::Compact,
    id: "compact",
    reference_enclosure: "Hammond 1590A (38.5 x 92.6 mm)",
    body_width: 156.0,
    body_height: 372.0,
    surface: pedal_surface(156.0, 372.0),
    max_columns: 2,
    rows: &[56.0, 146.0],
    led_y: 218.0,
    footswitch_y: 292.0,
    jack_y: 217.0,
};

/// 66 x 126.75 mm body; 300 x 543 logical canvas; 1200 x 2172 source PNG.
///
/// Width follows the 125B; the length keeps the pre-existing Greybound canvas
/// so every legacy faceplate stays valid.
pub const STANDARD_PEDAL_FORMAT: PedalFormatSpec = PedalFormatSpec {
    format: PedalFormat::Standard,
    id: "standard",
    reference_enclosure: "Hammond 125B (66 x 122 mm)",
    body_width: 264.0,
    body_height: 507.0,
    surface: pedal_surface(264.0, 507.0),
    max_columns: 3,
    rows: &[70.0, 172.0, 274.0],
    led_y: 345.0,
    footswitch_y: 427.0,
    jack_y: 295.8,
};

/// 94 x 126.75 mm body; 412 x 543 logical canvas; 1648 x 2172 source PNG.
///
/// Width follows the 1590BB; the length matches Standard so footswitch rows
/// line up across the board.
pub const WIDE_PEDAL_FORMAT: PedalFormatSpec = PedalFormatSpec {
    format: PedalFormat::Wide,
    id: "wide",
    reference_enclosure: "Hammond 1590BB (94 x 119.5 mm)",
    body_width: 376.0,
    body_height: 507.0,
    surface: pedal_surface(376.0, 507.0),
    max_columns: 4,
    rows: &[70.0, 172.0, 274.0],
    led_y: 345.0,
    footswitch_y: 427.0,
    jack_y: 295.8,
};

/// A grid-placed control: row/column slot plus knob tier.
#[allow(clippy::too_many_arguments)]
pub const fn grid_control(
    format: PedalFormat,
    row: PedalRow,
    columns: u8,
    index: u8,
    tier: KnobTier,
    widget: RenderControlWidget,
    role: RenderControlRole,
    label: &'static str,
    skin: KnobSkin,
    asset: Option<RenderControlAssetSpec>,
) -> RenderControlSpec {
    let (anchor_x, anchor_y) = format.spec().slot_anchor(row, columns, index);
    RenderControlSpec {
        role,
        widget,
        label,
        anchor_x,
        anchor_y,
        radius: tier.radius(),
        hit_radius: tier.hit_radius(),
        skin,
        asset,
    }
}

/// A grid-placed rotary pot.
#[allow(clippy::too_many_arguments)]
pub const fn grid_pot(
    format: PedalFormat,
    row: PedalRow,
    columns: u8,
    index: u8,
    tier: KnobTier,
    kind: ControlKind,
    label: &'static str,
    skin: KnobSkin,
    asset: Option<RenderControlAssetSpec>,
) -> RenderControlSpec {
    grid_control(
        format,
        row,
        columns,
        index,
        tier,
        RenderControlWidget::Pot,
        RenderControlRole::Parameter(kind),
        label,
        skin,
        asset,
    )
}

/// The format's bypass status LED.
pub const fn grid_led(
    format: PedalFormat,
    skin: KnobSkin,
    asset: Option<RenderControlAssetSpec>,
) -> RenderControlSpec {
    let (anchor_x, anchor_y) = format.spec().led_anchor();
    RenderControlSpec {
        role: RenderControlRole::Bypass,
        widget: RenderControlWidget::Led,
        label: "Status",
        anchor_x,
        anchor_y,
        radius: PEDAL_LED_RADIUS,
        hit_radius: 0.0,
        skin,
        asset,
    }
}

/// The format's bypass footswitch.
pub const fn grid_footswitch(
    format: PedalFormat,
    skin: KnobSkin,
    asset: Option<RenderControlAssetSpec>,
) -> RenderControlSpec {
    let (anchor_x, anchor_y) = format.spec().footswitch_anchor();
    RenderControlSpec {
        role: RenderControlRole::Bypass,
        widget: RenderControlWidget::Footswitch,
        label: "Bypass",
        anchor_x,
        anchor_y,
        radius: PEDAL_FOOTSWITCH_RADIUS,
        hit_radius: PEDAL_FOOTSWITCH_HIT_RADIUS,
        skin,
        asset,
    }
}

/// Signed label baseline offset from the control centre, in logical pixels of
/// the pedal surface.
pub fn pedal_label_baseline_offset(control: &RenderControlSpec) -> f32 {
    match control.widget {
        // Vertical selectors sit just above the footswitch, so their label
        // goes above the travel instead of competing for the lower faceplate.
        RenderControlWidget::Slider => -PEDAL_LABEL_BASELINE_OFFSET,
        _ => PEDAL_LABEL_BASELINE_OFFSET.max(control.radius + PEDAL_LABEL_CLEARANCE),
    }
}

/// Conservative bounding width of an engraved all-caps label.
pub fn pedal_label_width(label: &str) -> f32 {
    label.chars().count() as f32 * PEDAL_LABEL_ADVANCE + PEDAL_LABEL_SIDE_PADDING * 2.0
}

/// Axis-aligned rectangle in logical canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBox {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Enclosure body on the canvas of any pedal surface. Freeform legacy pedals
/// use the same margin as the grid formats.
pub fn pedal_body_box(render: &ModelRenderSpec) -> LayoutBox {
    LayoutBox {
        left: PEDAL_CANVAS_MARGIN,
        top: PEDAL_CANVAS_MARGIN,
        right: render.surface.logical_width - PEDAL_CANVAS_MARGIN,
        bottom: render.surface.logical_height - PEDAL_CANVAS_MARGIN,
    }
}

pub fn pedal_control_center(render: &ModelRenderSpec, control: &RenderControlSpec) -> (f32, f32) {
    (
        control.anchor_x * render.surface.logical_width,
        control.anchor_y * render.surface.logical_height,
    )
}

/// Label bounds for parameter controls; bypass hardware carries no label.
pub fn pedal_label_box(render: &ModelRenderSpec, control: &RenderControlSpec) -> Option<LayoutBox> {
    if !matches!(control.role, RenderControlRole::Parameter(_)) {
        return None;
    }
    let (x, y) = pedal_control_center(render, control);
    let baseline = y + pedal_label_baseline_offset(control);
    let half_width = pedal_label_width(control.label) * 0.5;
    Some(LayoutBox {
        left: x - half_width,
        top: baseline - PEDAL_LABEL_HEIGHT,
        right: x + half_width,
        bottom: baseline,
    })
}

/// Geometry gate for every pedal surface.
///
/// All pedals: controls and labels stay inside the body, nothing overlaps.
/// Grid pedals additionally: the surface is the format's surface, and every
/// control sits on a grid slot, LED, or footswitch position with a tier size.
pub fn validate_pedal_layout(render: &ModelRenderSpec) -> Result<(), String> {
    if render.surface.kind != RenderSurfaceKind::Pedal {
        return Err(format!("{} is not a pedal surface", render.id));
    }
    if let RenderLayoutPolicy::PedalGrid(format) = render.layout {
        validate_grid_conformance(render, format)?;
    }

    let body = pedal_body_box(render);
    let clearance = PEDAL_COMPONENT_CLEARANCE;
    for (position, control) in render.controls.iter().enumerate() {
        let (x, y) = pedal_control_center(render, control);
        let r = control.radius;
        if x - r < body.left || x + r > body.right || y - r < body.top || y + r > body.bottom {
            return Err(format!(
                "{} control '{}' exceeds the pedal body",
                render.id, control.label
            ));
        }
        for other in &render.controls[position + 1..] {
            let (ox, oy) = pedal_control_center(render, other);
            let minimum = r + other.radius + clearance;
            if (x - ox).hypot(y - oy) < minimum {
                return Err(format!(
                    "{} controls '{}' and '{}' overlap (need {:.1}px centre clearance)",
                    render.id, control.label, other.label, minimum
                ));
            }
        }

        let Some(label) = pedal_label_box(render, control) else {
            continue;
        };
        if label.left < body.left + clearance
            || label.right > body.right - clearance
            || label.top < body.top + clearance
            || label.bottom > body.bottom - clearance
        {
            return Err(format!(
                "{} label '{}' exceeds the printable faceplate area",
                render.id, control.label
            ));
        }
        for other in render.controls {
            let (ox, oy) = pedal_control_center(render, other);
            if circle_intersects_box(ox, oy, other.radius + clearance, label) {
                return Err(format!(
                    "{} label '{}' overlaps the '{}' component",
                    render.id, control.label, other.label
                ));
            }
        }
        for other in &render.controls[position + 1..] {
            if let Some(other_label) = pedal_label_box(render, other) {
                if boxes_intersect(label, other_label) {
                    return Err(format!(
                        "{} labels '{}' and '{}' overlap",
                        render.id, control.label, other.label
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_grid_conformance(render: &ModelRenderSpec, format: PedalFormat) -> Result<(), String> {
    let spec = format.spec();
    let surface = spec.surface;
    if render.surface.logical_width != surface.logical_width
        || render.surface.logical_height != surface.logical_height
        || render.surface.recommended_pixel_width != surface.recommended_pixel_width
        || render.surface.recommended_pixel_height != surface.recommended_pixel_height
    {
        return Err(format!(
            "{} declares the {} grid but not its surface",
            render.id, spec.id
        ));
    }
    if let Some(asset) = render.asset {
        if asset.pixel_width != surface.recommended_pixel_width
            || asset.pixel_height != surface.recommended_pixel_height
        {
            return Err(format!(
                "{} faceplate {} is {}x{}, the {} format requires {}x{}",
                render.id,
                asset.path,
                asset.pixel_width,
                asset.pixel_height,
                spec.id,
                surface.recommended_pixel_width,
                surface.recommended_pixel_height
            ));
        }
    }

    let footswitches = render
        .controls
        .iter()
        .filter(|control| control.widget == RenderControlWidget::Footswitch)
        .count();
    if footswitches != 1 {
        return Err(format!(
            "{} must have exactly one footswitch, found {footswitches}",
            render.id
        ));
    }

    for control in render.controls {
        let anchor = (control.anchor_x, control.anchor_y);
        let conforms = match control.widget {
            RenderControlWidget::Footswitch => {
                same_anchor(anchor, spec.footswitch_anchor())
                    && control.radius == PEDAL_FOOTSWITCH_RADIUS
                    && control.hit_radius == PEDAL_FOOTSWITCH_HIT_RADIUS
            }
            RenderControlWidget::Led => {
                same_anchor(anchor, spec.led_anchor()) && control.radius == PEDAL_LED_RADIUS
            }
            RenderControlWidget::Pot
            | RenderControlWidget::Slider
            | RenderControlWidget::Toggle => {
                is_grid_slot(spec, anchor)
                    && KnobTier::ALL.iter().any(|tier| {
                        tier.radius() == control.radius && tier.hit_radius() == control.hit_radius
                    })
            }
        };
        if !conforms {
            return Err(format!(
                "{} control '{}' is not on the {} grid; build it with the grid_* helpers",
                render.id, control.label, spec.id
            ));
        }
    }
    Ok(())
}

fn is_grid_slot(spec: &PedalFormatSpec, anchor: (f32, f32)) -> bool {
    (1..=spec.max_columns).any(|columns| {
        (0..columns).any(|index| {
            spec.rows.iter().any(|row| {
                same_anchor(
                    anchor,
                    spec.anchor(spec.body_width * column_fraction(columns, index), *row),
                )
            })
        })
    })
}

fn same_anchor(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() <= ANCHOR_TOLERANCE && (a.1 - b.1).abs() <= ANCHOR_TOLERANCE
}

fn circle_intersects_box(x: f32, y: f32, radius: f32, bounds: LayoutBox) -> bool {
    let nearest_x = x.clamp(bounds.left, bounds.right);
    let nearest_y = y.clamp(bounds.top, bounds.bottom);
    (x - nearest_x).hypot(y - nearest_y) < radius
}

fn boxes_intersect(a: LayoutBox, b: LayoutBox) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_share_one_physical_scale() {
        for format in PedalFormat::ALL {
            let spec = format.spec();
            assert_eq!(
                spec.surface.logical_width,
                spec.body_width + 2.0 * PEDAL_CANVAS_MARGIN
            );
            assert_eq!(
                spec.surface.recommended_pixel_width as f32,
                spec.surface.logical_width * PEDAL_SOURCE_SCALE
            );
            assert_eq!(
                spec.surface.recommended_pixel_height as f32,
                spec.surface.logical_height * PEDAL_SOURCE_SCALE
            );
        }
        assert_eq!(STANDARD_PEDAL_FORMAT.surface.logical_width, 300.0);
        assert_eq!(STANDARD_PEDAL_FORMAT.surface.logical_height, 543.0);
        assert_eq!(STANDARD_PEDAL_FORMAT.surface.recommended_pixel_width, 1200);
        assert_eq!(STANDARD_PEDAL_FORMAT.surface.recommended_pixel_height, 2172);
        assert_eq!(COMPACT_PEDAL_FORMAT.body_width_mm(), 39.0);
        assert_eq!(STANDARD_PEDAL_FORMAT.body_width_mm(), 66.0);
        assert_eq!(WIDE_PEDAL_FORMAT.body_width_mm(), 94.0);
    }

    #[test]
    fn formats_order_their_hardware_top_to_bottom() {
        for format in PedalFormat::ALL {
            let spec = format.spec();
            let last_row = *spec.rows.last().unwrap();
            assert!(spec.rows.windows(2).all(|rows| rows[0] < rows[1]));
            assert!(last_row < spec.led_y, "{}", spec.id);
            assert!(spec.led_y < spec.footswitch_y, "{}", spec.id);
            assert!(
                spec.footswitch_y + PEDAL_FOOTSWITCH_RADIUS < spec.body_height,
                "{}",
                spec.id
            );
        }
    }
}
