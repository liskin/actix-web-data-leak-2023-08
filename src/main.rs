use std::sync::Arc;
use std::time::Duration;

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

// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     let x: Arc<String> = Arc::new(String::from("x"));
//
//     HttpServer::new({
//         let x = x.clone();
//         move || {
//             App::new()
//                 .app_data(JsonConfig::default().limit(1024 * 1024))
//                 .app_data(web::Data::new(State { _x: x.clone() }))
//                 .service(hello)
//                 .service(echo)
//                 .route("/hey", web::get().to(manual_hello))
//         }
//     })
//     .bind(("127.0.0.1", 8080))?
//     .run()
//     .await?;
//
//     let xx = Arc::downgrade(&x);
//     drop(x);
//     let xx = xx.upgrade();
//     println!("{:?}", xx);
//
//     Ok(())
// }

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use actix_web::{rt, web, App, HttpServer};

    let x: Arc<String> = Arc::new(String::from("x"));

    let data = web::Data::new(State { _x: x.clone() });
    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::JsonConfig::default().limit(1024 * 1024))
            .app_data(data.clone())
            .service(app::echo)
    })
    .shutdown_timeout(5)
    .bind(("127.0.0.1", 8080))?
    .run()
    .await?;

    let xx = Arc::downgrade(&x);
    drop(x);

    let a1: Option<String> = xx.upgrade().map(|x| x.as_ref().clone());
    rt::time::sleep(Duration::from_secs(5)).await;
    let a2: Option<String> = xx.upgrade().map(|x| x.as_ref().clone());

    println!("{:?} {:?}", a1, a2);
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    use reqwest::Client;
    use tokio::sync::mpsc;
    use tokio::time::sleep;
    use tokio_stream::wrappers::ReceiverStream;

    #[actix_web::main]
    async fn server() -> std::io::Result<()> {
        use actix_web::{rt, web, App, HttpServer};

        let x: Arc<String> = Arc::new(String::from("x"));

        let data = web::Data::new(State { _x: x.clone() });
        HttpServer::new(move || {
            App::new()
                .app_data(actix_web::web::JsonConfig::default().limit(1024 * 1024))
                .app_data(data.clone())
                .service(app::echo)
        })
        .shutdown_timeout(5)
        .bind(("127.0.0.1", 8080))?
        .run()
        .await?;

        let xx = Arc::downgrade(&x);
        drop(x);

        let a1: Option<String> = xx.upgrade().map(|x| x.as_ref().clone());
        rt::time::sleep(Duration::from_secs(5)).await;
        let a2: Option<String> = xx.upgrade().map(|x| x.as_ref().clone());

        println!("{:?} {:?}", a1, a2);
        Ok(())
    }

    #[test]
    fn test_1() -> anyhow::Result<()> {
        let t1 = std::thread::spawn(|| super::main());
        std::thread::sleep(Duration::from_millis(500));
        let t2 = std::thread::spawn(|| xxx());
        t1.join().map_err(std::panic::resume_unwind)??;
        t2.join().map_err(std::panic::resume_unwind)??;
        // sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    #[tokio::main]
    async fn xxx() -> anyhow::Result<()> {
        // Create a Reqwest client
        let client = Client::new();

        // Create a channel to simulate a slow network
        let (tx, rx) = mpsc::channel(32);

        // Start a separate task to simulate the slow network
        tokio::spawn(async move {
            for _ in 0..60 {
                sleep(Duration::from_millis(500)).await;
                tx.send(Ok::<_, std::convert::Infallible>(vec![0u8; 1024]))
                    .await
                    .unwrap();
            }
        });

        // Prepare the request
        let request = client
            .post("http://127.0.0.1:8080/echo")
            .header("Content-Type", "application/json")
            .body(reqwest::Body::wrap_stream(ReceiverStream::new(rx)));

        // Send the request and wait for the response
        let _ = request.send().await;

        // let response_text = response.text().await?;
        // println!("Response: {}", response_text);

        Ok(())
    }
}

// dd if=/dev/zero count=10 bs=8K | jq -R . | curl --limit-rate 1K --json @- http://localhost:8080/echo
