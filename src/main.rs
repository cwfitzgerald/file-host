#![feature(decl_macro, proc_macro_hygiene)]

use humansize::{file_size_opts::BINARY, FileSize};
use itertools::Itertools;
use maplit::hashmap;
use once_cell::sync::Lazy;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rocket::{
    get,
    http::{ContentType, Status},
    post,
    request::{FromRequest, Outcome},
    routes, Data, Request,
};
use rocket_contrib::{serve::StaticFiles, templates::Template};
use rocket_multipart_form_data::{
    MultipartFormData, MultipartFormDataField, MultipartFormDataOptions,
};
use std::path::{Path, PathBuf};

static UPLOAD_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let string = std::env::args()
        .nth(1)
        .expect("Must pass upload dir as first argument");
    Path::new(&string)
        .canonicalize()
        .expect("Directory must exist")
});

static API_KEY: Lazy<String> =
    Lazy::new(|| std::fs::read_to_string(UPLOAD_DIR.join("api-key")).expect("cannot find api key"));

struct AdminKey;

impl<'a, 'r> FromRequest<'a, 'r> for AdminKey {
    type Error = String;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        if let Some(true) = request
            .headers()
            .get_one("x-api-key")
            .map(|key| key == *API_KEY)
            .or_else(|| Some(request.get_query_value::<String>("key")?.ok()? == *API_KEY))
        {
            Outcome::Success(AdminKey)
        } else {
            Outcome::Failure((Status::Forbidden, String::from("invalid api key")))
        }
    }
}

fn gen_name(extant: impl AsRef<str>) -> String {
    let name: String = thread_rng().sample_iter(Alphanumeric).take(6).collect();
    let name_ext = name + "." + extant.as_ref();
    if UPLOAD_DIR.join(&name_ext).exists() {
        gen_name(extant)
    } else {
        name_ext
    }
}

#[get("/delete/<file>")]
fn delete(_admin: AdminKey, file: String) -> &'static str {
    let _ = std::fs::remove_file(UPLOAD_DIR.join(file));
    "Deleted!"
}

#[get("/manage")]
fn manage(_admin: AdminKey) -> Template {
    let files = UPLOAD_DIR
        .read_dir()
        .expect("could not read dir")
        .filter_map(|f| {
            let file = f.ok()?;
            let file_name_os = file.file_name();
            let file_name = file_name_os.to_string_lossy();
            if !file.file_type().ok()?.is_dir() && file_name != "api-key" {
                Some((
                    file_name.to_string(),
                    format!("http://localhost:8000/{}", file_name),
                    file.metadata().ok()?.len(),
                    format!(
                        "http://localhost:8000/delete/{}?key={}",
                        file_name, *API_KEY
                    ),
                ))
            } else {
                None
            }
        })
        .sorted_by_key(|&(_, _, len, _)| 0 - len as i64)
        .filter_map(|(file_name, url, len, delete)| {
            Some((file_name, url, len.file_size(BINARY).ok()?, delete))
        })
        .collect_vec();
    Template::render("manage", hashmap!["files" => files])
}

#[post("/upload", format = "multipart/form-data", data = "<data>")]
fn upload(_admin: AdminKey, content_type: &ContentType, data: Data) -> Result<String, String> {
    let options = MultipartFormDataOptions::with_multipart_form_data_fields(vec![
        MultipartFormDataField::bytes("file").size_limit(1 << 30),
        MultipartFormDataField::text("filename"),
    ]);

    let mut form =
        MultipartFormData::parse(content_type, data, options).map_err(|v| v.to_string())?;

    let data = if let Some(files) = form.raw.remove("file") {
        files.into_iter().next().unwrap().raw
    } else {
        return Err(String::from("No file field!"));
    };

    let filename = if let Some(texts) = form.texts.remove("filename") {
        texts.into_iter().next().unwrap().text
    } else {
        return Err(String::from("No filename field"));
    };

    let extant = filename
        .rfind('.')
        .map(|i| &filename[(i + 1)..])
        .unwrap_or("");

    let name = gen_name(&extant);
    let full_file = UPLOAD_DIR.join(name.clone());
    std::fs::write(&full_file, &data)
        .map(|_| format!("http://localhost:8000/{}", name))
        .map_err(|e| e.to_string())
}

fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        .mount("/", StaticFiles::from(&*UPLOAD_DIR))
        .mount("/", routes![upload, manage, delete])
        .launch();
}
