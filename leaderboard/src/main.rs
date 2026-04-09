use axum::{Router, routing::get};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new().route("/", get(root));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root<'a>() -> HelloTemplate<'a> {
    let hello = HelloTemplate {
        name: "Evil Elaina >:(",
    };
    hello
}

#[derive(askama::Template, askama_web::WebTemplate)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    name: &'a str,
}
