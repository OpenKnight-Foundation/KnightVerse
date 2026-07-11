use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use std::time::Instant;
use std::sync::Arc;
use metrics::{MetricsCollector, MiddlewareMetrics};

/// Metrics middleware for Actix-web
pub struct MetricsMiddleware {
    metrics_collector: Arc<MetricsCollector>,
}

impl MetricsMiddleware {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self { metrics_collector }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MetricsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = MetricsMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(MetricsMiddlewareService {
            service,
            metrics_collector: self.metrics_collector.clone(),
        })
    }
}

pub struct MetricsMiddlewareService<S> {
    service: S,
    metrics_collector: Arc<MetricsCollector>,
}

impl<S, B> Service<ServiceRequest> for MetricsMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let metrics_collector = self.metrics_collector.clone();
        let start_time = Instant::now();
        
        let method = req.method().to_string();
        let path = req.path().to_string();
        
        let future = self.service.call(req);
        
        Box::pin(async move {
            let res: ServiceResponse<B> = future.await?;
            
            let duration = start_time.elapsed().as_secs_f64();
            let status = res.status().as_u16();
            
            // Record metrics
            metrics_collector.inc_http_requests(&method, &path, status);
            metrics_collector.observe_http_duration(&method, &path, duration);
            
            Ok(res)
        })
    }
}

/// Helper function to create metrics middleware
pub fn create_metricsMiddleware(metrics_collector: Arc<MetricsCollector>) -> MetricsMiddleware {
    MetricsMiddleware::new(metrics_collector)
}
