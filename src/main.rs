use std::path::Path;

use grazing_incidence::cli::{self, Options};
use grazing_incidence::error::RenderError;
use grazing_incidence::renderer;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", cli::USAGE);
        return;
    }

    if let Err(e) = run(args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), RenderError> {
    let opts = Options::parse(args.into_iter())?;
    let image = renderer::render_scene(opts.width, opts.height)?;

    let path = Path::new(cli::DEFAULT_OUTPUT);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    image.write_ppm(path, renderer::GAMMA)?;

    println!("wrote {}", path.display());
    Ok(())
}
