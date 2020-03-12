use actix_files as fs;
use actix_service::Service;
use actix_web::body::Body;
use actix_web::body::MessageBody;
use actix_web::body::ResponseBody;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Responder};
use futures::future::FutureExt;

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
            // .wrap_fn(|req, srv| {
            //     let fut = srv.call(req);
            //     async {
            //         let mut res = fut.await?;
            //         dbg!(&res);
            //         // res.headers_mut()
            //         //     .insert(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
            //         // Ok(res)
            //         let new_res = res.map_body(|_head, body| {
            //             println!("{:?}", body.wait());
            //             ResponseBody::Body(Body::Message(Box::new("fredbob")))
            //         });
            //         Ok(new_res)
            //     }
            // })
            .wrap_fn(|req, srv| {
                println!("Hi from start. You requested: {}", req.path());
                srv.call(req).map(|res| {
                    // dbg!(&res);
                    Ok(res.unwrap().map_body(move |_, body| {
                        match body {
                            ResponseBody::Body(b) => {
                                // println!("{:?}", &b);
                                match &b {
                                    Body::None => Body::None,
                                    Body::Empty => Body::Empty,
                                    Body::Bytes(raw) => {
                                        println!("Content {:?}", raw);
                                        dbg!(raw);
                                        Body::Empty
                                    }
                                    Body::Message(raw) => {
                                        // println!("message {:?}", raw);
                                        Body::Empty
                                    }
                                };
                                ResponseBody::Body(b)
                            }
                            ResponseBody::Other(b) => ResponseBody::Other(b),
                        };

                        ResponseBody::Body("test")
                    }))
                })
            })
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
