mod telemetry;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let tracer_provider = telemetry::init_telemetry()
        .expect("failed to initialize telemetry");

    // ... existing DB pool / redis pool / app state setup stays exactly as is ...

    let server = HttpServer::new(move || {
        App::new()
            .wrap(tracing_actix_web::TracingLogger::default()) // <-- add this, before/around other middleware
            // ... existing .app_data(...), .service(...), .route(...) calls unchanged ...
    })
    .bind(("0.0.0.0", 8080))? // match the existing bind address/port
    .run();

    server.await?;

    telemetry::shutdown_telemetry(tracer_provider);
    Ok(())
}