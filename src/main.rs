#![feature(decl_macro, proc_macro_hygiene)]

use chrono::{DateTime, Utc};
use humansize::{file_size_opts::BINARY, FileSize};
use itertools::Itertools;
use once_cell::sync::{Lazy, OnceCell};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rocket::{
    fairing::AdHoc,
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
use serde::Serialize;
use std::path::{Path, PathBuf};

static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let string = std::env::args()
        .nth(1)
        .expect("Must pass data dir as first argument");
    Path::new(&string)
        .canonicalize()
        .expect("Directory must exist")
});

static UPLOAD_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let name = DATA_DIR.join("upload");
    std::fs::create_dir_all(&name).expect("Could not create upload dir");
    name
});

static API_KEY: Lazy<String> = Lazy::new(|| {
    std::fs::read_to_string(DATA_DIR.join("api-key"))
        .expect("cannot find api key")
        .trim()
        .to_string()
});

static WEBSITE_URL: OnceCell<String> = OnceCell::new();

fn get_website() -> &'static str {
    WEBSITE_URL.get().unwrap().as_str()
}

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

#[derive(Serialize)]
struct MangeContext {
    files: Vec<(String, String, String, String, String)>,
    total_size: String,
    total: usize,
}

#[get("/manage")]
fn manage(_admin: AdminKey) -> Template {
    let tmp = UPLOAD_DIR
        .read_dir()
        .expect("could not read dir")
        .filter_map(|f| {
            let file = f.ok()?;
            let file_name_os = file.file_name();
            let file_name = file_name_os.to_string_lossy();
            let metadata = file.metadata().ok()?;
            if !file.file_type().ok()?.is_dir() {
                Some((
                    file_name.to_string(),
                    format!("{}/{}", get_website(), file_name),
                    DateTime::<Utc>::from(metadata.created().ok()?)
                        .format("%Y-%m-%d")
                        .to_string(),
                    metadata.len(),
                    format!("{}/delete/{}?key={}", get_website(), file_name, *API_KEY),
                ))
            } else {
                None
            }
        })
        .sorted_by_key(|&(_, _, _, len, _)| 0 - len as i64)
        .collect_vec();
    let total_size: u64 = tmp.iter().map(|(_, _, _, len, _)| *len).sum();
    let files = tmp
        .into_iter()
        .filter_map(|(file_name, url, date, len, delete)| {
            Some((file_name, url, date, len.file_size(BINARY).ok()?, delete))
        })
        .collect_vec();
    Template::render("manage", MangeContext {
        total: files.len(),
        total_size: total_size.file_size(BINARY).unwrap(),
        files,
    })
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
        .map(|_| format!("{}/{}", get_website(), name))
        .map_err(|e| e.to_string())
}

fn main() {
    rocket::ignite()
        .attach(Template::fairing())
        .attach(AdHoc::on_attach("URL Config", |rocket| {
            WEBSITE_URL.get_or_init(|| {
                rocket
                    .config()
                    .get_string("url")
                    .unwrap_or_else(|_| String::from("localhost:8000"))
            });
            Ok(rocket)
        }))
        .mount("/", StaticFiles::from(&*UPLOAD_DIR))
        .mount("/", routes![upload, manage, delete])
        .launch();
}
