use rocket::routes;
use rocket_contrib::serve::StaticFiles;

fn main() {
    rocket::ignite().mount("/", StaticFiles::from(std::env::args().skip(1).next().expect("Must pass upload dir as first argument"))).launch();
}
