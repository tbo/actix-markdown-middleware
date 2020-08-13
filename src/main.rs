use actix_files as fs;
use actix_http::http::{self, HeaderValue};
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Responder};
extern crate askama;
use askama::Template;
use std::task::{Context, Poll};

use actix_service::Service;
use actix_web::body::{Body, BodySize, MessageBody, ResponseBody};
use actix_web::Error;
use bytes::{Bytes, BytesMut};
use pulldown_cmark::{html, Options, Parser};
// mod markdown;

#[derive(Template)]
#[template(path = "header.html")]

struct HeaderTemplate<'a> {
    title: &'a str,
}

#[derive(Template)]
#[template(path = "footer.html")]

struct FooterTemplate {}

pub struct Transformer;

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
            .service(web::scope("/files").wrap_fn(|req, service| {
                let path = req.path().to_owned();
                let fut = service.call(req);

                async move {
                    let mut res = fut.await?;

                    if res
                        .headers()
                        .get("content-type")
                        .map(|header| header.eq(&HeaderValue::from_static("text/markdown")))
                        .unwrap_or(false)
                    {
                        res.headers_mut().insert(
                            http::header::CONTENT_TYPE,
                            HeaderValue::from_static("text/html"),
                        );
                        return Ok(res.map_body(move |_, body| {
                            let size = body.size();
                            ResponseBody::Other(Body::from_message(MarkdownBody {
                                body,
                                path,
                                buffer: get_buffer_with_capacity(size),
                            }))
                        }));
                    }
                    Ok(res)
                }
            }))
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

fn get_buffer_with_capacity(capacity: BodySize) -> BytesMut {
    use BodySize::*;
    match capacity {
        Sized(capacity) => BytesMut::with_capacity(capacity),
        Sized64(capacity) => BytesMut::with_capacity(capacity as usize),
        _ => BytesMut::new(),
    }
}

pub struct MarkdownBody<B> {
    body: ResponseBody<B>,
    path: String,
    buffer: BytesMut,
}

impl<B: MessageBody> MarkdownBody<B> {
    fn is_complete(&self) -> bool {
        use BodySize::*;
        match self.body.size() {
            None | Empty => true,
            Sized(size) => size <= self.buffer.len(),
            Sized64(size) => size <= self.buffer.len() as u64,
            _ => false,
        }
    }
}

impl<B: MessageBody> MessageBody for MarkdownBody<B> {
    fn size(&self) -> BodySize {
        BodySize::Stream
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Bytes, Error>>> {
        let next = self.body.poll_next(cx);
        if let Poll::Ready(Some(Ok(chunk))) = next {
            self.buffer.extend_from_slice(&chunk);
            if !self.is_complete() {
                cx.waker().clone().wake();
                return Poll::Pending;
            }
            let s = &String::from_utf8_lossy(&self.buffer);
            let parser = Parser::new_ext(s, Options::empty());
            let mut html_output: String = String::with_capacity(s.len() * 3 / 2);
            let header = HeaderTemplate { title: &self.path };
            let footer = FooterTemplate {};
            html_output.push_str(&header.render().unwrap());
            html::push_html(&mut html_output, parser);
            html_output.push_str(&footer.render().unwrap());
            return Poll::Ready(Some(Ok(Bytes::from(html_output))));
        }
        return next;
    }
}
