use std::path::Path;

use grazing_incidence::cli::{self, Options};
use grazing_incidence::error::RenderError;
use grazing_incidence::renderer;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match Options::parse(args.into_iter()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };

    if opts.help {
        print!("{}", cli::USAGE);
        return;
    }

    if let Err(e) = run(&opts) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(opts: &Options) -> Result<(), RenderError> {
    let image = renderer::render_scene(opts)?;

    let path = Path::new(&opts.out);
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }

    // The extension picks the encoder: `.png` gets the compressed one,
    // anything else the PPM that has always been there.
    let is_png = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));

    if is_png {
        image.write_png(path, renderer::GAMMA)?;
    } else {
        image.write_ppm(path, renderer::GAMMA)?;
    }

    println!("wrote {}", path.display());
    Ok(())
}
