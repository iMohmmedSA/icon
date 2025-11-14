use crate::{Collection, PackIcon, glyphs_in_order, module_leaf};
use kurbo::{Affine, BezPath, CubicBez, PathEl, Point, Rect, Shape, Vec2};
use std::{collections::BTreeMap, fs::File, io::Write, path};
use usvg::{Group, Node, Options, Transform, Tree, tiny_skia_path};
use write_fonts::{
    FontBuilder, OffsetMarker,
    tables::{
        cmap::Cmap,
        glyf::{GlyfLocaBuilder, Glyph, SimpleGlyph},
        head::{Flags, Head},
        hhea::Hhea,
        hmtx::Hmtx,
        loca::LocaFormat,
        maxp::Maxp,
        name::{Name, NameRecord},
        os2::{Os2, SelectionFlags},
        post::Post,
        vmtx::LongMetric,
    },
    types::{FWord, Fixed, GlyphId, NameId, UfWord, Version16Dot16},
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

    let mut local = tiny_path_to_bez(path.data());
    let ts = path.abs_transform();
    if !ts.is_identity() {
        let aff = transform_to_affine(ts);
        local.apply_affine(aff);
    }

    out.extend(local);
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

struct ParsedSvg {
    outline: BezPath,
    view_box: Option<Rect>,
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

fn make_postscript_name(base: &str) -> String {
    base.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' => c,
            _ => '-',
        })
        .collect()
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

fn map_svg_to_em_space(
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

fn generate_font_bytes(
    module_name: &str,
    glyphs: &mut BTreeMap<Collection, Vec<PackIcon>>,
) -> Vec<u8> {
    let units_per_em: u16 = 1000;
    let ascent: i16 = units_per_em as i16;
    let descent: i16 = 0;
    let advance_width: u16 = 1000;
    let max_width = advance_width as f64;
    let max_height = (ascent - descent) as f64;

    let mut gl = GlyfLocaBuilder::new();
    gl.add_glyph(&Glyph::Empty).expect(".notdef");

    // Private Area from 0xE000 to 0xF8FF
    let mut next_codepoint: u16 = 0xE000;
    let mut next_gid: u16 = 1;
    let mut codepoints: Vec<(char, GlyphId)> = Vec::new();

    let ordered_entries = glyphs_in_order(glyphs);
    for (collection, index) in ordered_entries {
        let pack = glyphs
            .get_mut(&collection)
            .and_then(|packs| packs.get_mut(index))
            .unwrap_or_else(|| panic!("glyph order mismatch for collection '{}'", collection.0));

        if pack.icon.trim().is_empty() {
            panic!("{} svg should not be empty", pack.enum_variant)
        }

        let mut parsed_svg = svg_to_bez(&pack.icon);
        // TTF only supports quadratic curves
        parsed_svg.outline = bezpath_with_quadratics(&parsed_svg.outline);
        debug_assert!(
            !parsed_svg.outline.is_empty(),
            "Unexpected empty outline after conversion"
        );

        map_svg_to_em_space(&mut parsed_svg, units_per_em, max_width, max_height);

        let mut sg = SimpleGlyph::from_bezpath(&parsed_svg.outline).expect("malformed outline");

        // Without the two lines below the glyph would be centered at the left middle
        sg.bbox.x_min = 0;
        sg.bbox.y_min = 0;

        gl.add_glyph(&sg).expect("add glyph");

        let ch = char::from_u32(next_codepoint as u32).expect("valid PUA codepoint");
        let gid = GlyphId::from(next_gid);
        codepoints.push((ch, gid));

        pack.icon = ch.to_string();

        next_codepoint = next_codepoint.wrapping_add(1);
        next_gid = next_gid.wrapping_add(1);
    }

    let total_glyphs = next_gid;
    let (glyf, loca, loca_fmt) = gl.build();
    let index_to_loc_format: i16 = match loca_fmt {
        LocaFormat::Short => 0,
        LocaFormat::Long => 1,
    };

    let head = Head {
        font_revision: Fixed::ONE,
        flags: Flags::empty(),
        units_per_em,
        x_min: 0,
        y_min: descent,
        x_max: units_per_em as i16,
        y_max: ascent,
        lowest_rec_ppem: 8,
        index_to_loc_format,
        ..Default::default()
    };

    let hhea = Hhea {
        ascender: FWord::from(ascent),
        descender: FWord::from(descent),
        line_gap: FWord::from(0),
        advance_width_max: UfWord::from(advance_width),
        x_max_extent: FWord::from(advance_width as i16),
        number_of_h_metrics: total_glyphs.max(1),
        ..Default::default()
    };

    let maxp = Maxp {
        num_glyphs: total_glyphs,
        ..Default::default()
    };

    let mut long_metrics = Vec::with_capacity(hhea.number_of_h_metrics as usize);
    for _ in 0..hhea.number_of_h_metrics {
        long_metrics.push(LongMetric {
            advance: advance_width,
            side_bearing: 0,
        });
    }
    let hmtx = Hmtx::new(long_metrics, Vec::new());

    let mut post = Post::new(
        Fixed::from_f64(0.0),
        FWord::from(10),
        FWord::from(0),
        0,
        0,
        0,
        0,
        0,
    );
    post.version = Version16Dot16::VERSION_3_0;

    let name = {
        let notice = "Contains third-party icons under their original licenses.";
        let family = module_name.to_string();
        let subfam = "Regular";
        let full = format!("{family} {subfam}");
        let ps = format!("{}-{}", make_postscript_name(module_name), subfam);
        let vers = "Version 1.000".to_string();
        let desc = "Auto generated icon collection".to_string();
        let vend = "https://github.com/iMohmmedSA".to_string();

        let recs = vec![
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::COPYRIGHT_NOTICE,
                OffsetMarker::new(notice.to_string()),
            ),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::FAMILY_NAME,
                OffsetMarker::new(family.clone()),
            ),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::SUBFAMILY_NAME,
                OffsetMarker::new(subfam.to_string()),
            ),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::FULL_NAME,
                OffsetMarker::new(full.clone()),
            ),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::VERSION_STRING,
                OffsetMarker::new(vers),
            ),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::POSTSCRIPT_NAME,
                OffsetMarker::new(ps.clone()),
            ),
            NameRecord::new(3, 1, 0x0409, NameId::DESCRIPTION, OffsetMarker::new(desc)),
            NameRecord::new(3, 1, 0x0409, NameId::VENDOR_URL, OffsetMarker::new(vend)),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::TYPOGRAPHIC_FAMILY_NAME,
                OffsetMarker::new(family.clone()),
            ),
            NameRecord::new(
                3,
                1,
                0x0409,
                NameId::TYPOGRAPHIC_SUBFAMILY_NAME,
                OffsetMarker::new(subfam.to_string()),
            ),
        ];
        Name::new(recs)
    };

    let last_char_index = if total_glyphs > 1 {
        0xE000 + (total_glyphs - 2)
    } else {
        0xE000
    };

    let os2 = Os2 {
        x_avg_char_width: advance_width as i16,
        us_weight_class: 400,
        us_width_class: 5,
        panose_10: [0; 10],
        fs_selection: SelectionFlags::REGULAR,
        us_first_char_index: 0xE000,
        us_last_char_index: last_char_index,
        s_typo_ascender: ascent,
        s_typo_descender: descent,
        s_typo_line_gap: 0,
        us_win_ascent: ascent as u16,
        us_win_descent: (-descent) as u16,
        ul_code_page_range_1: Some(0),
        ul_code_page_range_2: Some(0),
        sx_height: Some(0),
        s_cap_height: Some(0),
        us_default_char: Some(0),
        us_break_char: Some(0),
        us_max_context: Some(0),
        ..Default::default()
    };

    let cmap = Cmap::from_mappings(codepoints).expect("failed to build cmap from glyph mappings");

    let mut fb = FontBuilder::new();
    fb.add_table(&head).expect("add head");
    fb.add_table(&hhea).expect("add hhea");
    fb.add_table(&maxp).expect("add maxp");
    fb.add_table(&hmtx).expect("add hmtx");
    fb.add_table(&os2).expect("add os/2");
    fb.add_table(&post).expect("add post");
    fb.add_table(&name).expect("add name");
    fb.add_table(&cmap).expect("add cmap");
    fb.add_table(&glyf).expect("add glyf");
    fb.add_table(&loca).expect("add loca");

    fb.build()
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

pub(crate) fn font_path(
    path_hint: impl AsRef<path::Path>,
    module_path: impl AsRef<path::Path>,
) -> (path::PathBuf, String) {
    let module_input = module_path.as_ref().to_string_lossy();
    let module_name = module_leaf(&module_input);

    let out_path = path_hint
        .as_ref()
        .with_file_name(format!("{module_name}.ttf"));

    (out_path, module_name)
}

/// Build TTF "{module}.ttf"
pub fn generate_font(
    path_hint: impl AsRef<path::Path>,
    module_path: impl AsRef<path::Path>,
    glyphs: &mut BTreeMap<Collection, Vec<PackIcon>>,
) {
    let (font_path, module_basename) = font_path(path_hint, module_path);
    let bytes = generate_font_bytes(&module_basename, glyphs);
    let mut f = File::create(font_path).expect("cannot create output TTF");
    f.write_all(&bytes).expect("failed to write TTF");
}
