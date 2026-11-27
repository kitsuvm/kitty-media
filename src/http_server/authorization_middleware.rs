//! This module contains the authorization middleware for the HTTP server.

use std::future::{Ready, ready};

use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header,
};
use futures_util::future::LocalBoxFuture;

/// Represents the context of an authorized request, containing the authorization token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthContext {
    /// The authorization token extracted from the request.
    pub token: String,
}

/// The middleware itself, which checks for the presence of a valid authorization token in incoming requests.
pub struct AuthMiddleware<S> {
    /// The inner service that this middleware wraps.
    service: S,
    /// The expected authorization token that incoming requests must provide.
    token: String,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let is_authorized = req
            .headers()
            .get(header::AUTHORIZATION)
            .is_some_and(|auth_header| auth_header == self.token.as_str());

        if !is_authorized {
            let res = HttpResponse::Unauthorized();
            return Box::pin(async move { Ok(req.into_response(res).map_into_right_body()) });
        }

        req.extensions_mut().insert(AuthContext {
            token: self.token.clone(),
        });

        let fut = self.service.call(req);

        Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) })
    }
}

/// The factory for creating instances of the [`AuthMiddleware`].
pub struct AuthMiddlewareFactory {
    /// The expected authorization token that incoming requests must provide.
    pub token: String,
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddlewareFactory
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = AuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddleware {
            service,
            token: self.token.clone(),
        }))
    }
}
