use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, Pool, Postgres, Row};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let db = Arc::new(
        AppState::new("postgres://postgres:l1nux=nosex@localhost/BlogData")
            .await
            .expect("Failed to connect to the database"),
    );

    let router = Router::new()
        .route(
            "/posts/{id}",
            get(get_post).patch(update_post).delete(delete_post),
        )
        .route("/posts", get(get_all_posts).put(put_post))
        .with_state(db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;

    axum::serve(listener, router).await?;

    Ok(())
}

async fn get_post(State(db): State<Arc<AppState>>, uri: Uri) -> impl IntoResponse {
    let id = id_from_uri(uri).expect("Invalid request ID");

    match db.read_post(id).await {
        Ok(post) => (StatusCode::OK, Json(json!(post))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn get_all_posts(State(db): State<Arc<AppState>>) -> impl IntoResponse {
    match db.list_posts().await {
        Ok(posts) => (StatusCode::OK, Json(json!(posts))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn put_post(
    State(db): State<Arc<AppState>>,
    Json(new_post): Json<RawPost>,
) -> impl IntoResponse {
    match db.create_post(new_post).await {
        Ok(post) => (StatusCode::OK, Json(json!(post))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn delete_post(State(db): State<Arc<AppState>>, uri: Uri) -> impl IntoResponse {
    let id = id_from_uri(uri).expect("Invalid request ID");

    match db.delete_post(id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn update_post(
    State(db): State<Arc<AppState>>,
    uri: Uri,
    Json(updated_post): Json<RawPost>,
) -> impl IntoResponse {
    let id = id_from_uri(uri).expect("Invalid request ID");

    match db.update_post(id, updated_post).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct RawPost {
    post_title: String,
    post_content: String,
    post_category: String,
    post_tags: Vec<String>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
struct Post {
    id: i32,
    post_title: String,
    post_content: String,
    post_category: String,
    post_tags: Vec<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

struct AppState {
    pool: Arc<Pool<Postgres>>,
}

impl AppState {
    async fn new(url: &'static str) -> Result<AppState, sqlx::Error> {
        let db = AppState {
            pool: Arc::new(PgPoolOptions::new().max_connections(5).connect(url).await?),
        };

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS posts
                (id SERIAL PRIMARY KEY,
                post_title varchar(128) NOT NULL,
                post_content TEXT NOT NULL,
                post_category varchar(64) NOT NULL,
                post_tags varchar(64)[] NOT NULL,
                created_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at timestamp without time zone NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(&mut *db.pool.acquire().await?)
        .await?;

        Ok(db)
    }

    async fn create_post(&self, new_post: RawPost) -> Result<i32, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;

        let post= sqlx::query("INSERT INTO posts (post_title, post_content, post_category, post_tags) values ($1, $2, $3, $4) RETURNING id")
            .bind(new_post.post_title)
            .bind(new_post.post_content)
            .bind(new_post.post_category)
            .bind(new_post.post_tags)
            .fetch_one(&mut *conn).await?.get("id");

        Ok(post)
    }

    async fn delete_post(&self, post_id: i32) -> Result<(), sqlx::Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query("DELETE FROM posts WHERE id = $1")
            .bind(post_id)
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    async fn read_post(&self, post_id: i32) -> Result<Post, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;

        let post: Post = sqlx::query_as("SELECT * FROM posts WHERE id = $1")
            .bind(post_id)
            .fetch_one(&mut *conn)
            .await?;

        Ok(post)
    }

    async fn update_post(&self, post_id: i32, updated_post: RawPost) -> Result<(), sqlx::Error> {
        let mut conn = self.pool.acquire().await?;

        sqlx::query("SELECT 1 FROM posts WHERE id = $1")
            .bind(post_id)
            .fetch_one(&mut *conn)
            .await?;

        sqlx::query("UPDATE posts SET post_title = $1, post_content = $2, post_category = $3, post_tags = $4, updated_at = $5  WHERE id = $6")
            .bind(updated_post.post_title)
            .bind(updated_post.post_content)
            .bind(updated_post.post_category)
            .bind(updated_post.post_tags)
            .bind(Utc::now())
            .bind(post_id)
            .execute(&mut *conn).await?;

        Ok(())
    }

    async fn list_posts(&self) -> Result<Vec<Post>, sqlx::Error> {
        let mut conn = self.pool.acquire().await?;

        let posts: Vec<Post> = sqlx::query_as("SELECT * FROM posts")
            .fetch_all(&mut *conn)
            .await?;

        Ok(posts)
    }
}

fn id_from_uri(uri: Uri) -> Result<i32, &'static str> {
    match uri.path().split('/').last().unwrap().parse::<i32>() {
        Ok(id) => Ok(id),
        Err(_) => Err("invalid request ID"),
    }
}
