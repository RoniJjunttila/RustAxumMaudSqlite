use axum::{
    extract::{Path, Query, State}, response::{sse::{Event, Sse}, Html, IntoResponse}, routing::{get, post}, Form, Json, Router
};
use tower_http::services::ServeDir;
use maud::{html, Markup};
use rusqlite::{params, Connection, Error};
use std::{convert::Infallible, sync::{Arc, Mutex as StdMutex}};

use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};

use serde::{Serialize, Deserialize};
use tokio_stream::wrappers::BroadcastStream;

use futures::{channel::mpsc, StreamExt};
use uuid::Uuid;
use std::collections::HashMap;

use chrono::{DateTime, Local, SecondsFormat, Utc};

#[derive(Clone)]
struct AppState {
    db: Arc<StdMutex<Connection>>,
    sender: broadcast::Sender<String>,
    clients: Arc<Mutex<HashMap<Uuid, mpsc::Sender<String>>>>,
    usernames: Arc<Mutex<HashMap<Uuid, String>>>,
}

#[derive(Deserialize)]
struct PostForm {
    post_title: String,
    post_tag: String,
    post_text: String,
    client_uuid: String,
    username: String
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

#[derive(Serialize)]
struct InsertResponse {
    success: bool,
    ucode: String,
}

#[derive(Serialize)]
struct SignInResponse {
    success: bool,
    username: String
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    message: String,
}

/* #[derive(Serialize)] 
struct PostResponse {
    post_title: String,
    post_text: String,
} */

#[derive(Serialize)] 
struct PostResp {
    posts: Vec<HashMap<String, String>>
}

#[derive(Deserialize)]
struct PostQuery {
    sorting: i32,
    offset: i32,
}

fn time_ago_string(diff: chrono::Duration) -> String {
    if diff.num_minutes() < 1 {
        "just now".into()
    } else if diff.num_hours() < 1 {
        format!("{} minutes ago", diff.num_minutes())
    } else if diff.num_days() < 1 {
        format!("{} hours ago", diff.num_hours())
    } else {
        format!("{} days ago", diff.num_days())
    }
}

fn render_front_page(posts: Vec<HashMap<String, String>>, b_render_share: bool, _share_id: String, shared_post: HashMap<String, String>) -> Markup {

    let sort = [("Newest", 0), ("Highest rating", 1), ("On the rise", 2)];

    let auth = ["Sign in", "Create user"];

   html! {
    (maud::DOCTYPE)
    html {
        head {
            meta charset="utf-8";
            title { "5F" }
            link rel="stylesheet" href="/static/styles.css" {}
        }
        body {
            nav {
                div class="logo" { "5F" }
                ul {
                    li {
                        button id="openSignIn" { "Sign in" }
                    }
                    li {
                        button id="openNewPost" { "New post" }
                    }
                }
            }

            @if b_render_share {
                 @let time_ago = {
                    let timestamp = shared_post.get("cAt").map(|string| string.as_str()).unwrap_or("");
                    let dt = DateTime::parse_from_rfc3339(timestamp).unwrap();
                    let diff = Local::now() - dt.with_timezone(&Local);
                    let time = time_ago_string(diff); 
                    time
                };  
                div id="posts-list" {
                    div class="post" {
                        div class="post-header" {
                            span { "u/" (shared_post.get("username").unwrap_or(&"".to_string()))}
                            span class="dot" { "•" }
                            span {  (time_ago) }
                        }
                        h3 { (shared_post.get("title").unwrap_or(&"".to_string())) }
                        p { "tag: " (shared_post.get("tag").unwrap_or(&"".to_string())) }
                        p class="post-tag" { (shared_post.get("text").unwrap_or(&"".to_string())) }
/*                             div class="post-footer" {
                                p { "Thumbs: "}
                            button id="btn-up" type="button" {
                                "👍 Up puuttuu "
                                (shared_post.get("tUp").unwrap_or(&"".to_string()))
                            }
                            button id="btn-down" type="button" {
                                "👎 Down puuttuu "
                                (shared_post.get("tDown").unwrap_or(&"".to_string()))
                            }
                        } */
                    }
                }
            }

            div class="sort-parent" {
                    @for (name, value) in &sort {
                        button
                            class={ "sortButton" (if *name == "Newest" { " active" } else { "" }) }
                            data-value=(value)
                            onclick="
                                document.querySelectorAll('.sortButton').forEach(
                                    b => b.classList.remove('active')
                                ); 
                                this.classList.add('active');"
                        { (name) }
                }
            }

            div id="posts-list" {
                 @for post in posts {
                     @let time_ago = {
                            let timestamp = post.get("cAt").map(|string| string.as_str()).unwrap_or("");
                            let dt = DateTime::parse_from_rfc3339(timestamp).unwrap();
                            let diff = Local::now() - dt.with_timezone(&Local);
                            let time = time_ago_string(diff); 
                            time
                        }; 

                        div class="post" {
                            div class="post-header" {
                                span { "u/" (post.get("username").unwrap_or(&"".to_string())) }
                                span class="dot" { "•" }
                                span { (time_ago) }
                            }
                            h3 { (post.get("title").unwrap_or(&"".to_string())) }
                            p { "tag: " (post.get("tag").unwrap_or(&"".to_string()))}
                            p class="post-tag" { (post.get("text").unwrap_or(&"".to_string())) }
                            div class="post-footer" {
/*                             p { "Thumbs: "}
                            button id="btn-up" type="button" {
                                "👍 Up puuttuu "
                                (shared_post.get("tUp").unwrap_or(&"".to_string()))
                            }
                            button id="btn-down" type="button" {
                                "👎 Down puuttuu "
                                (shared_post.get("tDown").unwrap_or(&"".to_string()))
                            } */
                            button class="btn-share" type="button" data-value=(post.get("id").unwrap_or(&"".to_string())) {
                                "Share post "
                            }
                        }
                    }
                } 
            }

            div id="authModal" class="modal" {
                div class="modal-content" {
                    button class="close" id="closeSignIn" { "×" }

                    div class="auth-header-parent" {
                        div {
                            @for title in &auth {
                            button
                                class={ "authButton" (if *title == "Sign in" { " active" } else { "" }) }
                                data-value=(title)
                                onclick="
                                    document.querySelectorAll('.authButton').forEach(
                                        b => b.classList.remove('active')
                                    ); 
                                    this.classList.add('active');
                                "
                            {
                                h2 { (title) }
                            }   
                        }

                        div id="userFormParent" {
                            form method="post" id="userForm" {
                            input type="text" id="username" name="username" placeholder="username" autocomplete="off" maxlength="20" minlength="4";
                            input data-value="Sign in" type="submit" value="Sign in" id="Sign in";
                            }
                        }
                            /*  form method="post" id="userForm" {
                                input type="text" id="username" name="username" placeholder="username";
                                input type="submit" value="Sign in";
                            }

                            form method="post" id="userForm" {
                                input type="text" id="username" name="username" placeholder="username";
                                input type="submit" value="Create user";
                            } */
                        }
                    }
                }
            }


            div id="newPostModal" class="modal" {
                div class="modal-content" {
                    button class="close" id="closeNewPost" { "×" }
                    h2 { "Create New Post" }
                    form method="post" id="postForm" {
                        label for="postTitle" { "Post title:" }
                        input type="text" id="post_title" name="post_title" placeholder="Title" autocomplete="off";
                        input type="text" id="post_tag" name="post_tag" placeholder="Tag your post" autocomplete="off"; 
                        textarea id="post_text" name="post_text" placeholder="Write your post..." {};
                        input type="submit" value="Post";
                    }
                }
            }

            div id="shareModal" class="modal" {
                div class="modal-content" {
                    button class="close" id="closeShareModal" { "×" }
                    h2 { "Share url" }
                    input type="text" id="share_id_text" name="share_id_text" readonly; 
                    
                }
            }
            script src="/static/app.js" {}
        }
    }
}
}

 async fn api_post_titles(State(state): State<AppState>, Query(params): Query<PostQuery>) -> Json<PostResp>  {

    let sorting_order = params.sorting;
    let offset = params.offset;

    let posts = get_post_titles_and_text_from_db(&state, offset, sorting_order).await;

    return Json(PostResp { posts });
} 

async fn get_post_titles_and_text_from_db(state: &AppState, offset: i32, sorting_order: i32) -> Vec<HashMap<String, String>> { 
    let connection = state.db.lock().unwrap();

    let sql = match sorting_order {
        0 => "SELECT post_title, post_text, tUp, tDown, cAt, tag, username, id FROM posts ORDER BY id DESC LIMIT 10 OFFSET ?", //newest
        1 => "SELECT post_title, post_text, tUp, tDown, cAt, tag, username, id FROM posts ORDER BY (tUp - tDown) DESC LIMIT 10 OFFSET ?", //highest rating
        2 => "SELECT post_title, post_text, tUp, tDown, cAt, tag,username, id FROM posts WHERE cAt >= datetime('now', '-1 day') ORDER BY tUp DESC LIMIT 10 OFFSET ?", //on the rise
        _ => "SELECT post_title, post_text, tUp, tDown, cAt, tag, username, id FROM posts ORDER BY id DESC LIMIT 10 OFFSET ?", //newest
    };

    let mut statement = connection.prepare(sql).unwrap(); 

    let rows = statement
        .query_map([offset], |row| {
            Ok((
                row.get::<_, String>(0)?, //title
                row.get::<_, String>(1)?, //text
                row.get::<_, i32>(2)?, //tUp
                row.get::<_, i32>(3)?, //tDown
                row.get::<_, String>(4)?,  //cAt
                row.get::<_, String>(5)?, // tag
                row.get::<_, String>(6)?, // username
                row.get::<_, i32>(7)?, // id
            ))
        })
        .unwrap();

    let mut result_mapped: Vec<HashMap<String, String>> = Vec::new();
    for row in rows {
            let mut single_post = HashMap::new();
            if let Ok((title, text, t_up, t_down, c_at, tag, username, id)) = row {
                single_post.insert("title".to_string(), title.to_string());
                single_post.insert("text".to_string(), text.to_string());
                single_post.insert("tUp".to_string(), t_up.to_string());
                single_post.insert("tDown".to_string(), t_down.to_string());
                single_post.insert("cAt".to_string(), c_at.to_string());
                single_post.insert("tag".to_string(), tag.to_string());
                single_post.insert("username".to_string(), username.to_string());
                single_post.insert("id".to_string(), id.to_string());
            }
            result_mapped.push(single_post);
        }

    return result_mapped;
    /* pitäs oikeestaan miettiä mikä on järkevintä... ja miten, otettais esim ascending ja sit tarvis clientiltä numeron monta in range käytännössää...  */
}

async fn get_shared_post(state: &AppState, post_id_str: &String) -> HashMap<String, String> {
    let connection = state.db.lock().unwrap();

    //println!("post_id type: {}", std::any::type_name_of_val(&post_id_str));

    let post_id: i64  = post_id_str.parse().unwrap(); //post_id_str.parse().unwrap()

    let result = connection.query_row(
        "SELECT post_title, post_text, tUp, tDown, cAt, tag, username FROM posts WHERE id = ?1",
        params![post_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?, // title
                row.get::<_, String>(1)?, // text
                row.get::<_, i32>(2)?,    // tUp
                row.get::<_, i32>(3)?,    // tDown
                row.get::<_, String>(4)?, // cAt
                row.get::<_, String>(5)?, // tag
                row.get::<_, String>(6)?, // username
            ))
        },
    );

    let mut single_post = HashMap::new();

    match result {
        Ok((title, text, t_up, t_down, c_at, tag, username)) => {
            single_post.insert("title".to_string(), title.to_string());
            single_post.insert("text".to_string(), text.to_string());
            single_post.insert("tUp".to_string(), t_up.to_string());
            single_post.insert("tDown".to_string(), t_down.to_string());
            single_post.insert("cAt".to_string(), c_at.to_string());
            single_post.insert("tag".to_string(), tag.to_string());
            single_post.insert("username".to_string(), username.to_string());
        }
        Err(Error::QueryReturnedNoRows) => {
            let current_date = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            single_post.insert("title".to_string(), "Post was not found!".to_string());
            single_post.insert("text".to_string(), "Womp womp :((".to_string());
            single_post.insert("tUp".to_string(), "0".to_string());
            single_post.insert("tDown".to_string(), "0".to_string());
            single_post.insert("cAt".to_string(), current_date.to_string());
            single_post.insert("tag".to_string(),  "unlucky".to_string());
            single_post.insert("username".to_string(), "mr.nobody".to_string());
        }
        Err(_e) => {{
            let current_date = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            single_post.insert("title".to_string(), "Post was not found!".to_string());
            single_post.insert("text".to_string(), "Womp womp :((".to_string());
            single_post.insert("tUp".to_string(), "0".to_string());
            single_post.insert("tDown".to_string(), "0".to_string());
            single_post.insert("cAt".to_string(), current_date.to_string());
            single_post.insert("tag".to_string(),  "unlucky".to_string());
            single_post.insert("username".to_string(), "mr.nobody".to_string());
        }}
    }
    return single_post;

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

async fn on_share_get_posts(Path(id): Path<String>, State(state): State<AppState>) -> Html<String> {
    let posts= get_post_titles_and_text_from_db(&state, 0, 0).await;
    
    let shared_post = get_shared_post(&state, &id).await;

    println!("{:?}", shared_post);

    let render_share = true;

    Html(render_front_page(posts, render_share, id, shared_post).into_string())
}

async fn list_names(State(state): State<AppState>) -> Html<String> {
   // let usernames: Vec<String> = get_usernames_from_db(&state).await;

    let posts= get_post_titles_and_text_from_db(&state, 0, 0).await;

    let shared_post: HashMap<String, String> = HashMap::new(); 

    let render_share = false;
    let share_id = "".to_string();

    Html(render_front_page(posts, render_share, share_id, shared_post).into_string())
}

#[tokio::main]
async fn main() {
    let db = Connection::open("app.db").unwrap();

    db.execute("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, username TEXT NOT NULL)",[]).unwrap();

    db.execute("CREATE TABLE IF NOT EXISTS posts (id INTEGER PRIMARY KEY, post_title TEXT NOT NULL, post_text TEXT NOT NULL, tUp INTEGER NOT NULL, tDown INTEGER NOT NULL, cAt TEXT NOT NULL, tag TEXT NOT NULL, username TEXT NOT NULL)", []).unwrap();

    let (sender, _receiver) = broadcast::channel::<String>(16);

    let state = AppState {
        db: Arc::new(StdMutex::new(db)),
        sender,
        clients: Arc::new(Mutex::new (HashMap::new())),
        usernames: Arc::new(Mutex::new (HashMap::new()))
    };

    let app = Router::new()
        .route("/", get(list_names))
        .route("/events", get(server_side_event_handler))
        .route("/api/usernames", get(api_usernames))
        .route("/api/posts", get(api_post_titles))
        .route("/share/:id", get(on_share_get_posts))
        .route("/insertPost", post(insert_post_into_db))
        .route("/insertUser", post(insert_user_into_db))
        .route("/signInUser", post(sign_in_user))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state.clone());

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("🚀 Running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.unwrap();
}

async fn insert_post_into_db(State(state): State<AppState>, Form(form): Form<PostForm>) -> impl IntoResponse {

    let auth_check = check_if_username_is_binded_to_uuid(state.clone(), form.client_uuid.to_string(), form.username.to_string()).await;

    if !auth_check {
        return Json(ErrorResponse {
            success: false,
            message: "No".to_string(),
        })
    }

    let connection = state.db.lock().unwrap();

    let date = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    println!("from this uuid {:?} get username from somewhere", form.client_uuid);

    connection.execute("INSERT INTO posts (post_title, post_text, tUp, tDown, cAt, tag, username) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![form.post_title.to_string(), form.post_text.to_string(), 0, 0, date.to_string(), form.post_tag.to_string(), form.username.to_string()]).unwrap();
    
    let message = format!("{}:{}", form.client_uuid, "new_post");

    println!("show your post as the newest when posting... eg highlight post jota vois käyttää sit sharen kanssa yhessä");
    
    let _ = state.sender.send(message);

    Json(ErrorResponse {
        success: true,
        message: "Post inserted successfully".to_string(),
    })

   // Redirect::to("/");
}

async fn insert_user_into_db(State(state): State<AppState>, Form(form): Form<UserForm>) -> impl IntoResponse {
    let connection = state.db.lock().unwrap();
    connection.execute("INSERT INTO users (username) VALUES(?1)", params![form.username]).unwrap();

    let name_length = form.username.chars().count();

    if name_length >= 20 && name_length <= 3 {
        return Json(InsertResponse {
            success: false,
            ucode: "2lng".to_string(),
        })
    }

    let ucode= generate_user_auth(form.username);

    Json(InsertResponse {
        success: true,
        ucode: ucode,
    })

   // Redirect::to("/");
}

async fn sign_in_user(State(state): State<AppState>, Form(form): Form<UserForm>) -> impl IntoResponse {
    
    let mut usernames = state.usernames.lock().await;

    let ucode = form.username;
    let user_uuid = form.client_uuid;

    let parsed_uuid = match Uuid::parse_str(&user_uuid) {
        Ok(u) => u,
        Err(_) => {
            println!("Invalid UUID string: {}", user_uuid);

            return Json(SignInResponse { success: (false), username: (String::new()) }) 
        }
    };

     println!("{:?}", ucode);

    let (success, username) = decrpyct_ucode(ucode);

    if success {
        if let Some(entry) = usernames.get_mut(&parsed_uuid) {
            *entry = username.clone();
            println!("UUID string: {} {}", user_uuid, username);
        }
    }

    Json(SignInResponse {
        success: success,
        username: username,
    })
}

#[allow(dead_code)]
async fn check_if_username_is_binded_to_uuid(state: AppState, uuid: String, username: String) -> bool {
    let parsed_uuid = match Uuid::parse_str(&uuid) {
        Ok(u) => u,
        Err(_) => {
            return false;
        }
    };

    let usernames = state.usernames.lock().await;

    println!("usernames {:?}", usernames); /* tyhjä */
    println!("username {:?}", username);
    println!("uuid {:?}", parsed_uuid);

     if let Some(stored_username) = usernames.get(&parsed_uuid) {
        if stored_username == &username {
            return true;
        } else {
            return false;
        }
    } else {
        return false;
    } 
}
#[allow(dead_code)]
struct ClientGuard {
    uuid: Uuid,
    state: AppState
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        let uuid = self.uuid;
        let state = self.state.clone();

        tokio::spawn(async move {
            println!("Client disconnected: {}", uuid);

            state.clients.lock().await.remove(&uuid);
            state.usernames.lock().await.remove(&uuid);
        });
    }
}

async fn server_side_event_handler(
    State(state): State<AppState>
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> 
{
    let uuid = Uuid::new_v4();

    println!("New client joined: {}", uuid);

    let (sender, _receiver) = mpsc::channel::<String>(16);
    state.clients.lock().await.insert(uuid, sender);
    state.usernames.lock().await.insert(uuid, String::new());

    //let _guard = ClientGuard { uuid, state: state.clone() };

    let receiver = state.sender.subscribe();

    let stream = BroadcastStream::new(receiver)
        .filter_map(move |msg| {
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
    
/*     let heartbeat = stream::repeat_with(|| {
        Ok(Event::default().comment("heartbeat"))
    })
    .throttle(std::time::Duration::from_secs(5)); */

    let chain_streams = init.chain(stream); /* .chain(heartbeat) */

    Sse::new(chain_streams)
}


fn generate_user_auth(username: String) -> String {

    let characters: Vec<char> = username.chars().collect();

    let mid = characters.len() / 2;
    let first: String = characters[..mid].iter().collect();
    let second: String = characters[mid..].iter().collect();
    let full: String = [first.clone(), second.clone()].join("");

    /* needs hash */
    let result: String = first + "-" + &full + "-" + &second;
    return result.to_string();
}

fn decrpyct_ucode(ucode: String) -> (bool, String) {

    let parts: Vec<&str> = ucode.split('-').collect();

    let username = 
        if let (Some(first_part), Some(second_part)) = (parts.first(), parts.last()) {
            [first_part.to_string(), second_part.to_string()].join("")
        } else {
            String::new()
        };
    
    let success: bool = username == parts[1];
    let user: String = 
        if success {
            username.to_string()
        } else {
            String::new()
        };

    return (success, user);
}
