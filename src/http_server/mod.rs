//! Contains the HTTP server implementation for Kitty Media using [`actix_web`].

use std::{io, net::ToSocketAddrs};

use actix_web::{App, HttpServer, Result, dev::Server, post, web};

use crate::{
    http_server::authorization_middleware::AuthMiddlewareFactory, models::Task,
    task_queue::TaskQueueDatabase,
};

mod authorization_middleware;

/// Represents the shared application state, including the task queue database.
pub struct AppState {
    /// The task queue database used for managing tasks.
    pub task_queue: TaskQueueDatabase,
}

/// Handles the creation of a new task. This endpoint is protected by the authorization middleware.
#[post("/task")]
async fn create_task(
    remote_task_queue: web::Data<AppState>,
    auth: web::ReqData<authorization_middleware::AuthContext>,
) -> Result<String> {
    tracing::info!("Authorization token: {}", auth.token);

    Ok("todo".to_string())
}

/// Creates and configures the HTTP server.
pub fn create_http_server<T: ToSocketAddrs>(
    host: T,
    remote_task_queue: TaskQueueDatabase,
    token: String,
) -> io::Result<Server> {
    let app_state = web::Data::new(AppState {
        task_queue: remote_task_queue,
    });

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(AuthMiddlewareFactory {
                token: token.clone(),
            })
            .app_data(app_state.clone())
            .service(web::scope("/v1").service(create_task))
    })
    .bind(host)?
    .run())
}
