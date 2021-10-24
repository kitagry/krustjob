use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use krustjob::*;
use tracing::{info, warn};

#[get("/healthz")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthz")
}

#[actix_rt::main]
async fn main() -> Result<()> {
    let (manager, drainer) = Manager::new().await;

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(manager.clone()))
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(health)
    })
    .bind("0.0.0.0:8080")
    .expect("Can not bind to 0.0.0.0:8080")
    .shutdown_timeout(5);

    tokio::select! {
        _ = drainer => warn!("controller drained"),
        _ = server.run() => info!("actix exited"),
    }
    Ok(())
}
