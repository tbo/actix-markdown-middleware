use actix_files as fs;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Responder};
mod markdown;

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

async fn index2() -> impl Responder {
    HttpResponse::Ok().body("Hello world again!")
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    env_logger::init();
    HttpServer::new(|| {
        App::new()
            .wrap(markdown::Transformer)
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .route("/", web::get().to(index))
            .route("/again", web::get().to(index2))
            .service(fs::Files::new("/files", ".").show_files_listing())
    })
    .bind("127.0.0.1:8088")?
    .run()
    .await
}
