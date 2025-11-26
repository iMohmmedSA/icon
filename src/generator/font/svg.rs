use kurbo::{Affine, BezPath, CubicBez, PathEl, Point, Rect, Shape, Vec2};
use usvg::{
    Group, Node, Options, PaintOrder, Transform, Tree,
    tiny_skia_path::{self, PathStroker},
};

pub(crate) fn wrap_iconify_svg(body: &str, width: f64, height: f64) -> String {
    fn fmt(value: f64) -> String {
        let mut s = format!("{value:.6}");
        while s.contains('.') && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        if s.is_empty() {
            s.push('0');
        }
        s
    }

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}">{}</svg>"#,
        fmt(width),
        fmt(height),
        body.trim()
    )
}

fn wrap_svg_if_needed(svg_or_d: &str) -> String {
    let trimmed = svg_or_d.trim();
    if !trimmed.contains('<') {
        // Most common size 24x24
        return format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{}"/></svg>"#,
            trimmed
        );
    }

    let svg_formats = trimmed.starts_with("<svg")
        || trimmed.starts_with("<?xml")
        || trimmed.starts_with("<!DOCTYPE");
    if svg_formats {
        svg_or_d.to_string()
    } else {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{}</svg>"#,
            svg_or_d
        )
    }
}

/// Convert a tiny-skia point into a kurbo point.
fn skia_point_to_kurbo(p: tiny_skia_path::Point) -> Point {
    Point::new(p.x as f64, p.y as f64)
}

fn transform_to_affine(t: Transform) -> Affine {
    Affine::new([
        t.sx as f64,
        t.ky as f64,
        t.kx as f64,
        t.sy as f64,
        t.tx as f64,
        t.ty as f64,
    ])
}

fn tiny_path_to_bez(path: &tiny_skia_path::Path) -> BezPath {
    let mut bez = BezPath::new();

    for segment in path.segments() {
        use tiny_skia_path::PathSegment;
        match segment {
            PathSegment::MoveTo(p) => {
                let point = skia_point_to_kurbo(p);
                bez.push(PathEl::MoveTo(point));
            }
            PathSegment::LineTo(p) => bez.push(PathEl::LineTo(skia_point_to_kurbo(p))),
            PathSegment::QuadTo(c, p) => bez.push(PathEl::QuadTo(
                skia_point_to_kurbo(c),
                skia_point_to_kurbo(p),
            )),
            PathSegment::CubicTo(c1, c2, p) => bez.push(PathEl::CurveTo(
                skia_point_to_kurbo(c1),
                skia_point_to_kurbo(c2),
                skia_point_to_kurbo(p),
            )),
            PathSegment::Close => bez.push(PathEl::ClosePath),
        }
    }

    bez
}

fn append_path_node(path: &usvg::Path, out: &mut BezPath) {
    if !path.is_visible() {
        return;
    }

    let ts = path.abs_transform();
    let aff = (!ts.is_identity()).then(|| transform_to_affine(ts));

    let fill_path = path.fill().map(|_| {
        let mut local = tiny_path_to_bez(path.data());
        if let Some(aff) = aff {
            local.apply_affine(aff);
        }
        local
    });

    let stroke_path = path.stroke().and_then(|stroke| {
        let res_scale = PathStroker::compute_resolution_scale(&ts);
        let stroke = stroke.to_tiny_skia();

        let stroked = path.data().stroke(&stroke, res_scale)?;
        let mut local = tiny_path_to_bez(&stroked);
        if let Some(aff) = aff {
            local.apply_affine(aff);
        }
        Some(local)
    });

    match (fill_path, stroke_path) {
        (Some(fill), Some(stroke)) => match path.paint_order() {
            PaintOrder::FillAndStroke => {
                out.extend(fill);
                out.extend(stroke);
            }
            PaintOrder::StrokeAndFill => {
                out.extend(stroke);
                out.extend(fill);
            }
        },
        (Some(fill), None) => out.extend(fill),
        (None, Some(stroke)) => out.extend(stroke),
        (None, None) => {}
    }
}

fn collect_group_paths(group: &Group, out: &mut BezPath) {
    for node in group.children() {
        match node {
            Node::Group(child) => collect_group_paths(child, out),
            Node::Path(path) => append_path_node(path, out),
            _ => {}
        }
    }
}

pub(crate) struct ParsedSvg {
    pub outline: BezPath,
    pub view_box: Option<Rect>,
}

fn svg_to_bez(svg_or_d: &str) -> ParsedSvg {
    let svg = wrap_svg_if_needed(svg_or_d);
    let view_box = extract_view_box(&svg);

    let opt = Options::default();
    let tree = Tree::from_data(svg.as_bytes(), &opt).expect("usvg parse failed");

    let mut out = BezPath::new();
    collect_group_paths(tree.root(), &mut out);

    ParsedSvg {
        outline: out,
        view_box,
    }
}

fn bezpath_with_quadratics(path: &BezPath) -> BezPath {
    const TOLERANCE: f64 = 0.1;

    let mut out = BezPath::new();
    let mut subpath_start = Point::ZERO;
    let mut current_point = Point::ZERO;
    let mut current_point_set = false;

    for path in path.iter() {
        match path {
            PathEl::MoveTo(p) => {
                out.push(PathEl::MoveTo(p));
                subpath_start = p;
                current_point = p;
                current_point_set = true;
            }
            PathEl::LineTo(p) => {
                assert!(current_point_set, "LineTo before MoveTo in input path");
                out.push(PathEl::LineTo(p));
                current_point = p;
            }
            PathEl::QuadTo(c, p) => {
                assert!(current_point_set, "QuadTo before MoveTo in input path");
                out.push(PathEl::QuadTo(c, p));
                current_point = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                assert!(current_point_set, "CurveTo before MoveTo in input path");
                let cubic = CubicBez::new(current_point, c1, c2, p);
                for (_, _, quad) in cubic.to_quads(TOLERANCE) {
                    out.push(PathEl::QuadTo(quad.p1, quad.p2));
                    current_point = quad.p2;
                }
            }
            PathEl::ClosePath => {
                out.push(PathEl::ClosePath);
                current_point = subpath_start;
            }
        }
    }

    out
}

pub(crate) fn map_svg_to_em_space(
    parsed_svg: &mut ParsedSvg,
    units_per_em: u16,
    max_width: f64,
    max_height: f64,
) {
    const MIN_DIM: f64 = 1e-6;

    let svg_bbox = parsed_svg.outline.bounding_box();
    let svg_w = svg_bbox.width();
    let svg_h = svg_bbox.height();
    assert!(
        svg_w > MIN_DIM && svg_h > MIN_DIM,
        "SVG dimensions are too small"
    );

    if let Some(vb) = parsed_svg
        .view_box
        .filter(|r| r.width() > MIN_DIM && r.height() > MIN_DIM)
    {
        let scale = (units_per_em as f64) / vb.height();
        parsed_svg.outline.apply_affine(
            Affine::translate(Vec2::new(-vb.x0, -vb.y0))
                .then_scale_non_uniform(scale, -scale)
                .then_translate(Vec2::new(0.0, units_per_em as f64)),
        );
        return;
    }

    let scale = (max_width / svg_w).min(max_height / svg_h);
    assert!(
        scale.is_finite() && scale > MIN_DIM,
        "cannot scale to target box"
    );

    parsed_svg.outline.apply_affine(
        Affine::translate(Vec2::new(-svg_bbox.x0, -svg_bbox.y0))
            .then_scale_non_uniform(scale, -scale)
            .then_translate(Vec2::new(0.0, scale * svg_h)),
    );
}

fn extract_view_box(svg: &str) -> Option<Rect> {
    let (_, rest) = svg.split_once("viewBox=")?;
    let rest = rest.trim_start();
    let start = rest.chars().next()?;
    if start != '"' && start != '\'' {
        return None;
    }
    let end = rest[1..].find(start)? + 1;
    let content = &rest[1..end];
    let mut values = content
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok());
    let x0 = values.next()?;
    let y0 = values.next()?;
    let w = values.next()?;
    let h = values.next()?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some(Rect::new(x0, y0, x0 + w, y0 + h))
}

pub(crate) fn svg_to_quadratics(svg_or_d: &str) -> ParsedSvg {
    let mut parsed_svg = svg_to_bez(svg_or_d);
    parsed_svg.outline = bezpath_with_quadratics(&parsed_svg.outline);
    parsed_svg
}
