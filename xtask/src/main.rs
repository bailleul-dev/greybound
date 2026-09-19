use anyhow::{bail, Context};
use greybound_ui::pedal_format::{
    pedal_label_baseline_offset, pedal_label_width, validate_pedal_layout, PEDAL_CANVAS_MARGIN,
    PEDAL_LABEL_HEIGHT, PEDAL_SOURCE_SCALE,
};
use greybound_ui::{
    AppProfile, ModelRenderSpec, PedalFormat, RenderControlRole, RenderControlWidget,
    RenderLayoutPolicy, RenderSurfaceKind,
};
use image::GenericImageView;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const JACK_ZONE_HALF_HEIGHT: f32 = 68.0;

/// Source-pixel geometry of one pedal faceplate, derived from its format.
#[derive(Debug, Clone, Copy)]
struct SourceGeometry {
    format: PedalFormat,
    width: u32,
    height: u32,
    body_left: f32,
    body_top: f32,
    body_right: f32,
    body_bottom: f32,
    jack_y: f32,
}

impl SourceGeometry {
    fn of(render: &ModelRenderSpec) -> Self {
        let format = match render.layout {
            RenderLayoutPolicy::PedalGrid(format) => format,
            // Legacy freeform faceplates all live on the Standard canvas.
            RenderLayoutPolicy::Freeform => PedalFormat::Standard,
        };
        let spec = format.spec();
        let margin = PEDAL_CANVAS_MARGIN * PEDAL_SOURCE_SCALE;
        let width = spec.surface.recommended_pixel_width;
        let height = spec.surface.recommended_pixel_height;
        Self {
            format,
            width,
            height,
            body_left: margin,
            body_top: margin,
            body_right: width as f32 - margin,
            body_bottom: height as f32 - margin,
            jack_y: spec.jack_anchor_y() * height as f32,
        }
    }

    fn x(&self, anchor_x: f32) -> f32 {
        anchor_x * self.width as f32
    }

    fn y(&self, anchor_y: f32) -> f32 {
        anchor_y * self.height as f32
    }
}

fn main() -> nih_plug_xtask::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        anyhow::bail!("Usage: cargo xtask bundle <package> [cargo build args]");
    };

    match command.as_str() {
        "pedal-assets" => pedal_assets(args.collect()),
        "bundle" => {
            let package = args.next().ok_or_else(|| {
                anyhow::anyhow!("Usage: cargo xtask bundle <package> [cargo build args]")
            })?;
            let cargo_args = args.collect::<Vec<_>>();
            let packages = vec![package.clone()];
            let target_dir = std::env::current_dir()?.join("target");

            nih_plug_xtask::build(&packages, &cargo_args)?;
            nih_plug_xtask::bundle(&target_dir, &package, &cargo_args, false)
        }
        _ => bail!("Unknown command '{command}'. Usage: cargo xtask bundle <package> | cargo xtask pedal-assets <export|prompt|validate> ..."),
    }
}

fn pedal_assets(args: Vec<String>) -> anyhow::Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("Usage: cargo xtask pedal-assets <export <dir>|formats <dir>|prompt <model-id>|validate <model-id> <png>|preview <model-id> <png> <out.png>>");
    };

    match command {
        "export" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("docs/generated/pedal-asset-contracts"));
            export_pedal_contracts(&output)
        }
        "formats" => {
            let output = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("docs/templates"));
            export_format_templates(&output)
        }
        "prompt" => {
            let model_id = args
                .get(1)
                .context("Usage: cargo xtask pedal-assets prompt <model-id>")?;
            let (_, render) = pedal_render_spec(model_id)?;
            print!("{}", faceplate_prompt(render));
            Ok(())
        }
        "validate" => {
            let model_id = args
                .get(1)
                .context("Usage: cargo xtask pedal-assets validate <model-id> <png>")?;
            let png = args
                .get(2)
                .context("Usage: cargo xtask pedal-assets validate <model-id> <png>")?;
            let (_, render) = pedal_render_spec(model_id)?;
            validate_faceplate(render, Path::new(png))?;
            println!("PASS: {model_id} faceplate contract: {png}");
            Ok(())
        }
        "preview" => {
            let model_id = args
                .get(1)
                .context("Usage: cargo xtask pedal-assets preview <model-id> <png> <out.png>")?;
            let png = args
                .get(2)
                .context("Usage: cargo xtask pedal-assets preview <model-id> <png> <out.png>")?;
            let output = args
                .get(3)
                .context("Usage: cargo xtask pedal-assets preview <model-id> <png> <out.png>")?;
            let (_, render) = pedal_render_spec(model_id)?;
            write_preview(render, Path::new(png), Path::new(output))?;
            println!("Wrote deterministic layout overlay: {output}");
            Ok(())
        }
        _ => {
            bail!("Unknown pedal-assets command '{command}'. Expected export, formats, prompt, validate, or preview.")
        }
    }
}

fn pedal_specs() -> Vec<(&'static str, &'static ModelRenderSpec)> {
    let mut specs = Vec::new();
    let mut ids = BTreeSet::new();
    for profile in [AppProfile::greybound_free(), AppProfile::greybound_glass()] {
        for device in profile.devices {
            if device.render.surface.kind == RenderSurfaceKind::Pedal
                && device.render.generation.is_some()
                && ids.insert(device.id)
            {
                specs.push((device.id, device.render));
            }
        }
    }
    specs
}

fn pedal_render_spec(model_id: &str) -> anyhow::Result<(&'static str, &'static ModelRenderSpec)> {
    pedal_specs()
        .into_iter()
        .find(|(id, _)| *id == model_id)
        .with_context(|| format!("Unknown regenerable pedal '{model_id}'. Run `cargo xtask pedal-assets export` to see the catalogue."))
}

fn export_pedal_contracts(output: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(output).with_context(|| format!("Could not create {}", output.display()))?;

    for (model_id, render) in pedal_specs() {
        validate_control_layout(render)?;
        let contract_path = output.join(format!("{model_id}.json"));
        let guide_path = output.join(format!("{model_id}.construction.svg"));
        fs::write(
            &contract_path,
            serde_json::to_string_pretty(&pedal_contract(model_id, render))? + "\n",
        )
        .with_context(|| format!("Could not write {}", contract_path.display()))?;
        fs::write(&guide_path, construction_guide(model_id, render))
            .with_context(|| format!("Could not write {}", guide_path.display()))?;
    }

    println!(
        "Exported {} pedal contracts to {}",
        pedal_specs().len(),
        output.display()
    );
    Ok(())
}

/// Writes one mechanical template per enclosure format, generated from the same
/// `PedalFormatSpec` the UI and validator use.
fn export_format_templates(output: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(output).with_context(|| format!("Could not create {}", output.display()))?;
    for format in PedalFormat::ALL {
        let spec = format.spec();
        let path = output.join(format!(
            "pedal-{}-{}x{}.svg",
            spec.id, spec.surface.recommended_pixel_width, spec.surface.recommended_pixel_height
        ));
        fs::write(&path, format_template(format))
            .with_context(|| format!("Could not write {}", path.display()))?;
        println!("Wrote {}", path.display());
    }
    Ok(())
}

fn format_template(format: PedalFormat) -> String {
    let spec = format.spec();
    let px = |logical: f32| logical * PEDAL_SOURCE_SCALE;
    let width = spec.surface.recommended_pixel_width;
    let height = spec.surface.recommended_pixel_height;
    let margin = px(PEDAL_CANVAS_MARGIN);
    let body_width = px(spec.body_width);
    let body_height = px(spec.body_height);
    let jack_y = spec.jack_anchor_y() * height as f32;
    let jack_top = jack_y - JACK_ZONE_HALF_HEIGHT;
    let jack_height = JACK_ZONE_HALF_HEIGHT * 2.0;
    let right_jack = margin + body_width;
    let body_x = |fraction: f32| margin + body_width * fraction;
    let body_y = |logical: f32| margin + px(logical);

    let mut slots = String::new();
    for (row_index, row) in spec.rows.iter().enumerate() {
        let y = body_y(*row);
        let name = ["A", "B", "C"][row_index];
        slots.push_str(&format!(
            "    <line x1=\"{margin}\" y1=\"{y:.1}\" x2=\"{right_jack}\" y2=\"{y:.1}\"/>\n    <text x=\"{text_x}\" y=\"{text_y:.1}\">ROW {name}</text>\n",
            text_x = margin + 16.0,
            text_y = y - 12.0,
        ));
        for columns in 1..=spec.max_columns {
            for index in 0..columns {
                let x = body_x(greybound_ui::pedal_format::column_fraction(columns, index));
                slots.push_str(&format!(
                    "    <circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"6\"/>\n"
                ));
            }
        }
    }
    let mut tiers = String::new();
    for tier in greybound_ui::KnobTier::ALL {
        tiers.push_str(&format!("{:?} r={} px, ", tier, px(tier.radius())));
    }
    let center_x = body_x(0.5);
    let led_y = body_y(spec.led_y);
    let footswitch_y = body_y(spec.footswitch_y);
    let led_radius = px(greybound_ui::pedal_format::PEDAL_LED_RADIUS);
    let footswitch_radius = px(greybound_ui::pedal_format::PEDAL_FOOTSWITCH_RADIUS);

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <title>Greybound {id} pedal mechanical template</title>
  <desc>
    Generated by `cargo xtask pedal-assets formats` from PedalFormatSpec; do not edit by hand.
    Enclosure class: {enclosure}. Body {body_mm_w} x {body_mm_h} mm at 16 source px/mm.
    Control slots are the only permitted anchors: rows {rows}, 1..{max_columns} columns.
    Knob tiers: {tiers}footswitch r={footswitch_radius} px, LED r={led_radius} px.
    Guides must not appear in final artwork.
  </desc>
  <rect x="2" y="2" width="{inner_w}" height="{inner_h}" fill="none" stroke="#64748b" stroke-width="4"/>
  <rect x="{margin}" y="{margin}" width="{body_width}" height="{body_height}" rx="72" fill="none" stroke="#00a7ff" stroke-width="6" stroke-dasharray="24 18"/>
  <g fill="#ffb347" fill-opacity="0.14" stroke="#ffb347" stroke-width="5" stroke-dasharray="14 12">
    <rect x="0" y="{jack_top:.1}" width="{margin}" height="{jack_height}"/>
    <rect x="{right_jack}" y="{jack_top:.1}" width="{margin}" height="{jack_height}"/>
  </g>
  <g stroke="#ff4fd8" stroke-width="2" stroke-dasharray="8 10" fill="#ff4fd8" font-family="Arial" font-size="28">
{slots}  </g>
  <g fill="none" stroke="#62d26f" stroke-width="4" stroke-dasharray="12 10">
    <circle cx="{center_x:.1}" cy="{led_y:.1}" r="{led_radius}"/>
    <circle cx="{center_x:.1}" cy="{footswitch_y:.1}" r="{footswitch_radius}"/>
  </g>
</svg>
"##,
        id = spec.id,
        enclosure = spec.reference_enclosure,
        body_mm_w = spec.body_width_mm(),
        body_mm_h = spec.body_height_mm(),
        rows = spec.rows.len(),
        max_columns = spec.max_columns,
        inner_w = width - 4,
        inner_h = height - 4,
    )
}

fn pedal_contract(model_id: &str, render: &ModelRenderSpec) -> serde_json::Value {
    let generation = render.generation.expect("filtered to regenerable pedals");
    let geometry = SourceGeometry::of(render);
    let controls = render.controls.iter().map(|control| {
        let x = geometry.x(control.anchor_x);
        let y = geometry.y(control.anchor_y);
        json!({
            "role": control_role(control.role),
            "widget": control_widget(control.widget),
            "label": control.label,
            "anchor": { "normalized": { "x": control.anchor_x, "y": control.anchor_y }, "source_px": { "x": x, "y": y } },
            "radius": { "logical_px": control.radius, "source_px": control.radius * PEDAL_SOURCE_SCALE },
            "hit_radius_logical_px": control.hit_radius,
            "dynamic_asset": control.asset.map(control_asset_contract),
            "label_baseline_source_y": if matches!(control.role, RenderControlRole::Parameter(_)) { Some(label_baseline_source_y(&geometry, control)) } else { None },
        })
    }).collect::<Vec<_>>();

    json!({
        "schema": "greybound.pedal-asset-contract/v2",
        "model_id": model_id,
        "enclosure": enclosure_contract(render),
        "render_id": render.id,
        "recipe_version": generation.recipe_version,
        "output": {
            "faceplate_path": generation.faceplate_path,
            "format": "PNG RGBA",
            "width": geometry.width,
            "height": geometry.height,
            "transparent_background": true,
        },
        "faceplate": {
            "model_name": generation.model_name,
            "art_direction": generation.art_direction,
            "view": "straight-on orthographic front elevation",
            "body_bounds_source_px": { "x": geometry.body_left, "y": geometry.body_top, "width": geometry.body_right - geometry.body_left, "height": geometry.body_bottom - geometry.body_top, "outer_radius": 72, "bevel": 24 },
            "side_jack_center_source_y": geometry.jack_y,
            "must_not_contain": ["knobs", "knob holes", "control labels", "control ticks", "footswitch", "footswitch hole", "LED jewel", "LED socket", "toggle", "slider", "front screws", "front jacks", "drop shadow", "floor", "background"],
        },
        "deterministic_runtime_layer": {
            "labels": "Drawn by UI from this contract; never generated as pixels.",
            "jacks": "Standard input/output jacks are rendered by the UI at the fixed mechanical anchors.",
            "controls": "All interactive hardware is rendered from the declared control assets below.",
        },
        "preflight_preview": "cargo xtask pedal-assets preview <model-id> <candidate.png> <inspection.png>",
        "controls": controls,
        "image_model_prompt": faceplate_prompt(render),
    })
}

fn enclosure_contract(render: &ModelRenderSpec) -> serde_json::Value {
    let geometry = SourceGeometry::of(render);
    let spec = geometry.format.spec();
    json!({
        "format": spec.id,
        "layout": match render.layout {
            RenderLayoutPolicy::PedalGrid(_) => "grid",
            RenderLayoutPolicy::Freeform => "legacy-freeform",
        },
        "reference_enclosure": spec.reference_enclosure,
        "body_mm": { "width": spec.body_width_mm(), "height": spec.body_height_mm() },
    })
}

fn control_asset_contract(asset: greybound_ui::RenderControlAssetSpec) -> serde_json::Value {
    json!({
        "image": asset.image.path,
        "active_image": asset.active_image.map(|image| image.path),
        "pressed_image": asset.pressed_image.map(|image| image.path),
        "rotation": asset.rotation.map(|rotation| json!({
            "min_degrees": rotation.min_degrees,
            "max_degrees": rotation.max_degrees,
            "pivot": { "x": rotation.pivot_x, "y": rotation.pivot_y },
        })),
    })
}

fn control_role(role: RenderControlRole) -> &'static str {
    match role {
        RenderControlRole::Parameter(_) => "parameter",
        RenderControlRole::Bypass => "bypass",
    }
}

fn control_widget(widget: RenderControlWidget) -> &'static str {
    match widget {
        RenderControlWidget::Pot => "pot",
        RenderControlWidget::Slider => "slider",
        RenderControlWidget::Toggle => "toggle",
        RenderControlWidget::Footswitch => "footswitch",
        RenderControlWidget::Led => "led",
    }
}

fn faceplate_prompt(render: &ModelRenderSpec) -> String {
    let generation = render.generation.expect("filtered to regenerable pedals");
    let geometry = SourceGeometry::of(render);
    let spec = geometry.format.spec();
    format!(
        "Use case: product-mockup\nAsset type: Greybound pedal faceplate source art\n\
Create exactly one EMPTY boutique guitar-pedal enclosure faceplate.\n\n\
Canvas and camera:\n- enclosure format: {format} ({enclosure}); keep its proportions\n- exact canvas: {width} x {height} px\n- straight-on orthographic front elevation; no perspective, no rotation\n- transparent background; no floor, environment, reflection, or shadow outside the enclosure\n- enclosure outer bounds: x={left}..{right}, y={top}..{bottom}; outer corner radius 72 px; visible bevel 24 px\n\n\
Identity:\n- model wordmark (the only permitted text): \"{}\"\n- art direction: {}\n- premium photorealistic metal/enamel construction, crisp 4x detail\n\n\
The application will place every functional component deterministically. Leave the entire face clean and uninterrupted.\n\n\
Absolutely do not render: knobs, knob caps, knob holes, dial marks, labels, footswitch, footswitch hole, LED, LED socket, switches, sliders, jacks, screws, washers, nuts, bezels, UI, background, floor, or any cast/contact shadow.\n\n\
Output only this empty enclosure as a PNG. The supplied construction SVG is a guide only and must not be visible in the output.\n",
        generation.model_name,
        generation.art_direction,
        format = spec.id,
        enclosure = spec.reference_enclosure,
        width = geometry.width,
        height = geometry.height,
        left = geometry.body_left,
        right = geometry.body_right,
        top = geometry.body_top,
        bottom = geometry.body_bottom,
    )
}

fn construction_guide(model_id: &str, render: &ModelRenderSpec) -> String {
    let geometry = SourceGeometry::of(render);
    let (width, height) = (geometry.width, geometry.height);
    let (left, top) = (geometry.body_left, geometry.body_top);
    let body_width = geometry.body_right - geometry.body_left;
    let body_height = geometry.body_bottom - geometry.body_top;
    let jack_top = geometry.jack_y - JACK_ZONE_HALF_HEIGHT;
    let jack_height = JACK_ZONE_HALF_HEIGHT * 2.0;
    let jack_text_y = geometry.jack_y + 14.0;
    let right_jack_x = geometry.body_right;
    let in_x = left + 48.0;
    let out_x = geometry.body_right - 48.0;

    let mut controls = String::new();
    for control in render.controls {
        let x = geometry.x(control.anchor_x);
        let y = geometry.y(control.anchor_y);
        let radius = control.radius * PEDAL_SOURCE_SCALE;
        let label = xml_escape(control.label);
        controls.push_str(&format!(
            "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"{radius:.1}\" fill=\"#ff4fd8\" fill-opacity=\".12\" stroke=\"#ff4fd8\" stroke-width=\"4\" stroke-dasharray=\"16 12\"/><text x=\"{x:.1}\" y=\"{label_y:.1}\" fill=\"#ff4fd8\" font-family=\"Arial\" font-size=\"28\" text-anchor=\"middle\">{label}</text>\n",
            label_y = label_baseline_source_y(&geometry, control),
        ));
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><title>{title}</title><rect width=\"{width}\" height=\"{height}\" fill=\"none\" stroke=\"#64748b\" stroke-width=\"4\"/><rect x=\"{left}\" y=\"{top}\" width=\"{body_width}\" height=\"{body_height}\" rx=\"72\" fill=\"none\" stroke=\"#00a7ff\" stroke-width=\"6\" stroke-dasharray=\"24 18\"/><rect x=\"0\" y=\"{jack_top:.1}\" width=\"{left}\" height=\"{jack_height}\" fill=\"#ffb347\" fill-opacity=\".16\"/><rect x=\"{right_jack_x}\" y=\"{jack_top:.1}\" width=\"{left}\" height=\"{jack_height}\" fill=\"#ffb347\" fill-opacity=\".16\"/><text x=\"{in_x}\" y=\"{jack_text_y:.1}\" fill=\"#ffb347\" font-family=\"Arial\" font-size=\"44\">IN</text><text x=\"{out_x}\" y=\"{jack_text_y:.1}\" fill=\"#ffb347\" font-family=\"Arial\" font-size=\"44\" text-anchor=\"end\">OUT</text>{controls}</svg>\n",
        title = xml_escape(model_id),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn validate_control_layout(render: &ModelRenderSpec) -> anyhow::Result<()> {
    validate_pedal_layout(render).map_err(anyhow::Error::msg)
}

fn validate_faceplate(render: &ModelRenderSpec, path: &Path) -> anyhow::Result<()> {
    let geometry = SourceGeometry::of(render);
    let (width, height) = (geometry.width, geometry.height);
    let image =
        image::open(path).with_context(|| format!("Could not decode {}", path.display()))?;
    if image.dimensions() != (width, height) {
        bail!(
            "{} must be {width}x{height} for the {} format, got {}x{}",
            path.display(),
            geometry.format.id(),
            image.width(),
            image.height()
        );
    }
    let rgba = image.to_rgba8();
    for (x, y) in [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ] {
        if rgba.get_pixel(x, y)[3] != 0 {
            bail!("{} has opaque canvas corner at ({x}, {y}); faceplates require a transparent background", path.display());
        }
    }
    let alpha_bounds = alpha_bounds(&rgba).context("Faceplate has no visible enclosure pixels")?;
    if alpha_bounds.0 < geometry.body_left as u32 - 4
        || alpha_bounds.1 < geometry.body_top as u32 - 4
        || alpha_bounds.2 > geometry.body_right as u32 + 4
        || alpha_bounds.3 > geometry.body_bottom as u32 + 4
    {
        bail!("{} visible pixels {:?} exceed the {} enclosure bounds; do not include a background, floor, or external shadow", path.display(), alpha_bounds, geometry.format.id());
    }
    validate_control_layout(render)?;
    Ok(())
}

fn alpha_bounds(image: &image::RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] == 0 {
            continue;
        }
        bounds = Some(match bounds {
            Some((left, top, right, bottom)) => {
                (left.min(x), top.min(y), right.max(x), bottom.max(y))
            }
            None => (x, y, x, y),
        });
    }
    bounds
}

fn label_baseline_source_y(
    geometry: &SourceGeometry,
    control: &greybound_ui::RenderControlSpec,
) -> f32 {
    geometry.y(control.anchor_y) + pedal_label_baseline_offset(control) * PEDAL_SOURCE_SCALE
}

fn write_preview(render: &ModelRenderSpec, source: &Path, output: &Path) -> anyhow::Result<()> {
    validate_faceplate(render, source)?;
    let geometry = SourceGeometry::of(render);
    let mut image = image::open(source)
        .with_context(|| format!("Could not decode {}", source.display()))?
        .to_rgba8();

    stroke_rect(
        &mut image,
        geometry.body_left as i32,
        geometry.body_top as i32,
        geometry.body_right as i32,
        geometry.body_bottom as i32,
        [0, 167, 255, 230],
        4,
    );
    let jack_top = (geometry.jack_y - JACK_ZONE_HALF_HEIGHT).round() as i32;
    let jack_bottom = (geometry.jack_y + JACK_ZONE_HALF_HEIGHT).round() as i32;
    let margin = geometry.body_left as i32;
    let width = geometry.width as i32;
    stroke_rect(
        &mut image,
        0,
        jack_top,
        margin,
        jack_bottom,
        [255, 179, 71, 230],
        4,
    );
    stroke_rect(
        &mut image,
        width - margin,
        jack_top,
        width,
        jack_bottom,
        [255, 179, 71, 230],
        4,
    );

    for control in render.controls {
        let x = geometry.x(control.anchor_x).round() as i32;
        let y = geometry.y(control.anchor_y).round() as i32;
        let radius = (control.radius * PEDAL_SOURCE_SCALE).round() as i32;
        stroke_circle(&mut image, x, y, radius, [255, 79, 216, 230], 4);
        if matches!(control.role, RenderControlRole::Parameter(_)) {
            let label_width =
                (pedal_label_width(control.label) * PEDAL_SOURCE_SCALE).round() as i32;
            let baseline = label_baseline_source_y(&geometry, control).round() as i32;
            stroke_rect(
                &mut image,
                x - label_width / 2,
                baseline - (PEDAL_LABEL_HEIGHT * PEDAL_SOURCE_SCALE).round() as i32,
                x + label_width / 2,
                baseline,
                [167, 139, 250, 230],
                3,
            );
        }
    }
    image
        .save(output)
        .with_context(|| format!("Could not write {}", output.display()))
}

fn stroke_rect(
    image: &mut image::RgbaImage,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    color: [u8; 4],
    width: i32,
) {
    for offset in 0..width {
        for x in left + offset..=right - offset {
            blend_pixel(image, x, top + offset, color);
            blend_pixel(image, x, bottom - offset, color);
        }
        for y in top + offset..=bottom - offset {
            blend_pixel(image, left + offset, y, color);
            blend_pixel(image, right - offset, y, color);
        }
    }
}

fn stroke_circle(
    image: &mut image::RgbaImage,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: [u8; 4],
    width: i32,
) {
    let outer = radius * radius;
    let inner_radius = (radius - width).max(0);
    let inner = inner_radius * inner_radius;
    for y in center_y - radius..=center_y + radius {
        for x in center_x - radius..=center_x + radius {
            let dx = x - center_x;
            let dy = y - center_y;
            let distance_squared = dx * dx + dy * dy;
            if distance_squared <= outer && distance_squared >= inner {
                blend_pixel(image, x, y, color);
            }
        }
    }
}

fn blend_pixel(image: &mut image::RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return;
    }
    let destination = image.get_pixel_mut(x as u32, y as u32);
    let source_alpha = color[3] as f32 / 255.0;
    let destination_alpha = destination[3] as f32 / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    for channel in 0..3 {
        let source = color[channel] as f32 / 255.0;
        let destination_color = destination[channel] as f32 / 255.0;
        let output = if output_alpha == 0.0 {
            0.0
        } else {
            (source * source_alpha + destination_color * destination_alpha * (1.0 - source_alpha))
                / output_alpha
        };
        destination[channel] = (output * 255.0).round() as u8;
    }
    destination[3] = (output_alpha * 255.0).round() as u8;
}
