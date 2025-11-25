use icon::Icon;

fn main() {
    println!("cargo::rerun-if-changed=fonts/icons.toml");
    Icon::builder("fonts/icons.toml")
        .set_assets_path("assets")
        .set_gen_type(icon::GenType::Iced)
        .build();
}
