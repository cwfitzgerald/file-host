#![feature(decl_macro, proc_macro_hygiene)]

use once_cell::sync::Lazy;
use rocket::request::Form;
use rocket::{post, routes, Data, FromForm};
use rocket_contrib::serve::StaticFiles;
use std::path::{Path, PathBuf};

static UPLOAD_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let string = std::env::args()
        .nth(1)
        .expect("Must pass upload dir as first argument");
    let absolute = Path::new(&string)
        .canonicalize()
        .expect("Directory must exist");
    absolute
});

#[derive(FromForm)]
struct UploadData {
    extant: String,
    data: String,
}

#[post("/upload", data = "<data>")]
fn upload(data: Form<UploadData>) {
    std::fs::write(
        UPLOAD_DIR.join(String::from("test1.") + &data.extant),
        &data.data,
    );
}

fn main() {
    rocket::ignite()
        .mount("/", StaticFiles::from(&*UPLOAD_DIR))
        .mount("/", routes![upload])
        .launch();
}
