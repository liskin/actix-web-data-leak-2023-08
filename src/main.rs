fn main() {}

#[cfg(test)]
mod test {
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::time::Duration;

    use actix_web::{rt, web, App, HttpServer};
    use anyhow::bail;
    use reqwest::Client;
    use tokio::join;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::ReceiverStream;

    struct State {
        _x: Arc<String>,
    }

    mod app {
        use actix_web::{post, web, HttpResponse, Responder};

        #[post("/echo")]
        async fn echo(_req_body: web::Json<String>) -> impl Responder {
            HttpResponse::Ok().body("xxx")
        }
    }

    enum ReqSpeed {
        Fast,
        Slow,
    }

    #[actix_web::test]
    async fn test_slow_request() -> anyhow::Result<()> {
        test_data_leak(Some(ReqSpeed::Slow)).await
    }

    #[actix_web::test]
    async fn test_fast_request() -> anyhow::Result<()> {
        test_data_leak(Some(ReqSpeed::Fast)).await
    }

    #[actix_web::test]
    async fn test_no_request() -> anyhow::Result<()> {
        test_data_leak(None).await
    }

    async fn test_data_leak(req: Option<ReqSpeed>) -> anyhow::Result<()> {
        let (x_weak, data) = {
            let x: Arc<String> = Arc::new(String::from("x"));
            (Arc::downgrade(&x), web::Data::new(State { _x: x }))
        };

        mod app {
            use actix_web::{post, web, HttpResponse, Responder};

            #[post("/echo")]
            async fn echo(_req_body: web::Json<String>) -> impl Responder {
                HttpResponse::Ok().body("xxx")
            }
        }

        {
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(actix_web::web::JsonConfig::default().limit(1024 * 1024))
                    .app_data(data.clone())
                    .service(app::echo)
            })
            .shutdown_timeout(2)
            .bind(("127.0.0.1", 0))?;
            let port = server.addrs()[0].port();
            let server = server.run();
            let server_handle = server.handle();

            let send_request = async move {
                rt::time::sleep(Duration::from_secs_f64(0.1)).await;
                if let Some(speed) = req {
                    request(format!("http://127.0.0.1:{}/echo", port), speed).await?;
                }
                Ok::<_, anyhow::Error>(())
            };

            let graceful_stop = async move {
                rt::time::sleep(Duration::from_secs(1)).await;
                server_handle.stop(/* graceful */ true).await;
            };

            let (server_res, _slow_req_res, ()) = join!(server, send_request, graceful_stop);
            server_res?;
        }

        for _ in 0..20 {
            rt::time::sleep(Duration::from_secs_f64(0.1)).await;
            if x_weak.upgrade().is_none() {
                return Ok(());
            }
        }

        bail!("x: Arc<String> is still referenced somewhere :-(");
    }

    async fn request(url: String, speed: ReqSpeed) -> anyhow::Result<()> {
        let client = Client::new();

        let req = client.post(url).header("Content-Type", "application/json");

        match speed {
            ReqSpeed::Fast => {
                let req = req.body("\"x\"");
                req.send().await?;
            }
            ReqSpeed::Slow => {
                let (body_tx, body_rx) = mpsc::channel(32);
                let slow_body = async move {
                    for _ in 0..60 {
                        rt::time::sleep(Duration::from_millis(500)).await;
                        body_tx.send(Ok::<_, Infallible>(vec![0u8; 1024])).await?;
                    }
                    Ok::<_, anyhow::Error>(())
                };

                let req = req.body(reqwest::Body::wrap_stream(ReceiverStream::new(body_rx)));
                let response = req.send();

                // Await the response and body generator
                let (response_res, slow_body_res) = join!(response, slow_body);
                response_res?;
                slow_body_res?;
            }
        }

        Ok(())
    }
}
