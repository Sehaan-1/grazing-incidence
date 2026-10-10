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
    // Checked before anything is written, so `--out somewhere.png` fails
    // without having created the directory it names.
    // Until chapter 14 there is only a PPM encoder.
    let path = Path::new(&opts.out);
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        return Err(RenderError::UnsupportedFormat {
            extension: "png".to_string(),
        });
    }

    let image = renderer::render_scene(opts)?;

    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    image.write_ppm(path, renderer::GAMMA)?;

    println!("wrote {}", path.display());
    Ok(())
}
