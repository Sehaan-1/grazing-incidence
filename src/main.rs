use std::path::Path;

use grazing_incidence::cli::{self, Encoder, Options};
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

    // The extension picks the encoder, and only two extensions are known.
    // Anything else used to fall through to PPM, so `--out x.txt` wrote a
    // binary PPM under a name that promises text -- a silent mislabelling
    // rather than a mistake the caller could see. `cli::encoder_for` now
    // refuses it.
    let encoder = cli::encoder_for(path)?;

    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }

    match encoder {
        Encoder::Png => image.write_png(path, renderer::GAMMA)?,
        Encoder::Ppm => image.write_ppm(path, renderer::GAMMA)?,
    }

    println!("wrote {}", path.display());
    Ok(())
}
