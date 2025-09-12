use axum::{
    extract::State, response::{sse::{Event, Sse}, Html, IntoResponse}, routing::{get, post}, Form, Router, Json
};
use tower_http::services::ServeDir;
use maud::{html, Markup};
use rusqlite::{params, Connection};
use std::{convert::Infallible, sync::{Arc, Mutex as StdMutex}};

use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};

use serde::{Serialize, Deserialize};
use tokio_stream::wrappers::BroadcastStream;

use futures::{channel::mpsc, StreamExt};
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Clone)]
struct AppState {
    db: Arc<StdMutex<Connection>>,
    sender: broadcast::Sender<String>,
    clients: Arc<Mutex<HashMap<Uuid, mpsc::Sender<String>>>>,
}

#[derive(Deserialize)]
struct PostForm {
    post_title: String,
    post_text: String,
}

#[derive(Deserialize)]
struct UserForm {
    username: String,
    client_uuid: String,
}

#[derive(Serialize)] 
struct UsernamesResponse {
    usernames: Vec<String>,
}

fn render_front_page(items: Vec<String>) -> Markup {
    html!(
        (maud::DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "5F" }
            }
            body {
                h1 { "items from sql" }
                ul id="items-list" {
                    @for item in items {
                        li { (item) }
                    }
                }

                form method="post"  {
                    label for="postTitle" { "Post title:" }
                    input type="text" id="postTitle" name="postTitle" placeholder="jaadijaadijaa";
                    input type="text" id="postText" name="postText" placeholder="Write your post...";
                    input type="submit" value="Post";
                }

                form method="post" id="userForm"{
                    label for="username" { "Username:" }
                    input type="text" id="username" name="username" placeholder="username";
                    input type="submit" value="Add User";
                }

                script src="/static/app.js" {}
            }
        }
    )
}

async fn api_usernames(State(state): State<AppState>) -> Json<UsernamesResponse> {
    let usernames = get_usernames_from_db(&state).await;
    return Json(UsernamesResponse { usernames });
}

async fn get_usernames_from_db(state: &AppState) -> Vec<String> {
    let connection = state.db.lock().unwrap();

    let mut stmt = connection.prepare("SELECT username FROM users").unwrap();

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap();

    let mut usernames = Vec::new();

    for row in rows {
        usernames.push(row.unwrap());
    }

    return usernames;
}

async fn list_names(State(state): State<AppState>) -> Html<String> {
    let usernames: Vec<String> = get_usernames_from_db(&state).await;

    Html(render_front_page(usernames).into_string())
}

/* TODO:  */

/* kun dc poista uuid  */

/* username => käyttäjän määrittelemä, ja passaword = uuid, sekotetaan uuid sekaan x tavalla. Häsh ja tää ois se uuid jota passattais vaan client */
/* server ois dehäshtä sen..  */

/* 
##################################
###                            ###
###         title              ###
###         text               ###
###         comment            ###
###         comment            ###
###         comment            ###
###         comment            ###
###                            ###
###         update             ###
##################################
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
##################################
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
##################################
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
###                            ###
##################################


*/

#[tokio::main]
async fn main() {
    let db = Connection::open("app.db").unwrap();

    db.execute("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, username TEXT NOT NULL)",[]).unwrap();

    db.execute("CREATE TABLE IF NOT EXISTS posts (id INTEGER PRIMARY KEY, postName TEXT NOT NULL, postText TEXT NOT NULL)", []).unwrap();

    let (sender, _receiver) = broadcast::channel::<String>(16);

    let state = AppState {
        db: Arc::new(StdMutex::new(db)),
        sender,
        clients: Arc::new(Mutex::new (HashMap::new()))
    };

    let app = Router::new()
        .route("/", get(list_names))
        .route("/events", get(server_side_event_handler))
        .route("/api/usernames", get(api_usernames))
        .route("/insertPost", post(insert_post_into_db))
        .route("/insertUser", post(insert_user_into_db))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("🚀 Running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}

async fn insert_post_into_db(State(state): State<AppState>, Form(form): Form<PostForm>) -> impl IntoResponse {
    let connection = state.db.lock().unwrap();
    connection.execute("INSERT INTO posts (postName, postText) VALUES (?1, ?2)", params![form.post_title, form.post_text]).unwrap();

    //tässä skip new post on self
    let _ = state.sender.send("new_post".into()).ok();

   // Redirect::to("/");
}

async fn insert_user_into_db(State(state): State<AppState>, Form(form): Form<UserForm>) -> impl IntoResponse {
    let connection = state.db.lock().unwrap();
    connection.execute("INSERT INTO users (username) VALUES(?1)", params![form.username]).unwrap();

    let message = format!("{}:{}", form.client_uuid, "new_user");

    let _ = state.sender.send(message);
    println!("redirecting: {:?}", form.client_uuid);

    //tässä skip new post on self

   // Redirect::to("/");
}

async fn server_side_event_handler(State(state): State<AppState>) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.sender.subscribe();
    let uuid = Uuid::new_v4();
    
    println!("New client joined with UUID: {}", uuid);

    let (sender, _receiver) = mpsc::channel::<String>(16);
    state.clients.lock().await.insert(uuid, sender);

    let stream = BroadcastStream::new(receiver).filter_map(move |msg| {
        let uuid = uuid;
        async move {
            match msg {
                Ok(m) => {
                    let mut parts = m.splitn(2, ':');
                    let sender_uuid = parts.next().unwrap_or_default();
                    let message = parts.next().unwrap_or_default();

                    if sender_uuid == uuid.to_string() {
                        None
                    } else {
                        Some(Ok(Event::default().data(message.to_string())))
                    }
                }
                Err(_) => None,
            }
        }
    });

    let init = futures::stream::once(async move {
        Ok(Event::default().event("init").data(uuid.to_string()))
    });

    let cleanup_stream = async_stream::stream! {
        let mut stream = stream;
        while let Some(event) = stream.next().await {
            yield event;
        }
        println!("Client {} disconnected", uuid);

        state.clients.lock().await.remove(&uuid);
    };

    let chain_streams = init.chain(stream);


    Sse::new(chain_streams)
}

#[allow(dead_code)]
async fn generate_user_file() {
    //oisko tää vaan sit string jossa olis mergetettynä user ja password mutta kaikki häshättynä¨
    //kuin sit menee jos esim haluis että tää on reaali aikanen, voisko olla auto fetch tai joku??? 
    //Melkeen tehä niin että inserttaaminen ja sit miten vois päivittyä itekseen jos db päivittyny tms? 
} 