extern crate askama;
use askama::Template;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use actix_http::http::HeaderValue;
use actix_service::{Service, Transform};
use actix_web::body::{Body, BodySize, MessageBody, ResponseBody};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, http, Error};
use bytes::{Bytes, BytesMut};
use futures::future::{ok, Ready};
use pulldown_cmark::{html, Options, Parser};

#[derive(Template)]
#[template(path = "header.html")]

struct HeaderTemplate<'a> {
    title: &'a str,
}

#[derive(Template)]
#[template(path = "footer.html")]

struct FooterTemplate {}

pub struct Transformer;

impl<S: 'static, B> Transform<S> for Transformer
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    B: MessageBody + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = MarkdownTransformerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(MarkdownTransformerMiddleware { service })
    }
}

pub struct MarkdownTransformerMiddleware<S> {
    service: S,
}

impl<S, B> Service for MarkdownTransformerMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_owned();
        let fut = self.service.call(req);

        Box::pin(async move {
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
        })
    }
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
