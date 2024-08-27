mod entities;
mod service;

use std::thread;
use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
// use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
// use service::{db, hk};

use rayon::ThreadPoolBuilder;

use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

#[derive(RustEmbed)]
#[folder = "web\\dist"]
pub struct FrontendAssets;

#[tokio::main]
async fn main() {
    // hk::call_dll();
    // let result = db::run().await;
    // println!("result: {:?}", result);
    info();
    // test().await;
}

fn info() {
    let logical_core_count = num_cpus::get();
    println!("逻辑CPU核心数: {}", logical_core_count);

    // 获取当前线程池中的线程数
    let thread_pool = ThreadPoolBuilder::new().build().unwrap();
    println!("线程数: {}", thread_pool.current_num_threads());

    let s = Instant::now();
    let mut v = vec![];
    for _i in 0..10 {
        v.push(thread::spawn(|| {
            let mut c = String::new();
            for i in 0..9999999 {
                c = c + &format!("-{}", i);
            }
            return c.len() > 0;
        }));
    }

    for ele in v {
        let _ = ele.join();
    }
    println!("执行完成, 耗时：{:?} ms", s.elapsed().as_millis())
}

async fn test() {
    let app = Router::new()
        .route("/", get(to_web))
        // .route("/web/", get(to_web))
        .route("/*path", get(serve_static_file))
        .route("/api/version", get(get_version));

    match tokio::net::TcpListener::bind("0.0.0.0:8080").await {
        Ok(m) => {
            println!("http service ready, addr: {}", m.local_addr().unwrap());
            match axum::serve(m, app).await {
                Ok(m) => {
                    println!("2 -- 启动成功:{:?}", m)
                }
                Err(e) => {
                    println!("2 -- 启动失败:{}", e)
                }
            };
        }
        Err(e) => {
            println!("1 -- 启动失败:{}", e);
        }
    };
}

async fn to_web() -> Response {
    Redirect::to("/index.html").into_response()
}

async fn serve_static_file(path: Path<String>) -> Result<Response, StatusCode> {
    match FrontendAssets::get(path.as_str()) {
        Some(bytes) => {
            let mime_type = mime_guess::from_path(path.as_str()).first_or_octet_stream();
            Ok(axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime_type.as_ref())
                .body(Body::from(bytes.data))
                .unwrap())
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_version(Query(params): Query<GetVersionParams>) -> Json<R<String>> {
    Json(R {
        code: 200,
        msg: None,
        data: Some(String::from(format!(
            "{}:{}",
            env!("CARGO_PKG_VERSION"),
            params.a
        ))),
    })
}

#[derive(Serialize)]
struct R<T> {
    code: i32,
    msg: Option<String>,
    data: Option<T>,
}

#[derive(Deserialize)]
struct GetVersionParams {
    a: i32,
}
