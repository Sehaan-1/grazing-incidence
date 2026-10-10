use grazing_incidence::renderer;
use grazing_incidence::scene;

fn main() {
    println!("{}", renderer::about());
    let _ = scene::Scene::FinalBook;
}