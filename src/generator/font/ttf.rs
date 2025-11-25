use std::{collections::BTreeMap, fs::File, io::Write, path};

use crate::{
    model::{Collection, PackIcon},
    utils::{glyphs_in_order, module_leaf},
};
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

use super::svg::{map_svg_to_em_space, svg_to_quadratics};

fn make_postscript_name(base: &str) -> String {
    base.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' => c,
            _ => '-',
        })
        .collect()
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

pub(crate) fn generate_font_bytes(
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
            .unwrap_or_else(|| panic!("glyph order mismatch for collection '{}'", collection.name));

        if pack.icon.trim().is_empty() {
            panic!("{} svg should not be empty", pack.enum_variant)
        }

        let mut parsed_svg = svg_to_quadratics(&pack.icon);

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
