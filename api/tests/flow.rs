use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use edurust_api::config::Config;
use edurust_api::content::{ImportOutcome, import_course};
use edurust_api::db;
use edurust_api::http;
use edurust_api::state::AppState;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct TestApp {
    app: axum::Router,
    pool: PgPool,
}

async fn test_app() -> TestApp {
    let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://edurust:edurust@localhost:5432/edurust_test".to_string());
    ensure_database(&url).await;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("test database");
    db::migrate(&pool).await.expect("migrations");
    truncate(&pool).await;
    let state = AppState::new(pool.clone(), Config::for_tests(url)).expect("state");
    TestApp {
        app: http::router(state),
        pool,
    }
}

async fn ensure_database(url: &str) {
    let db_name = database_name(url);
    let admin = with_database(url, "edurust");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin)
        .await
        .expect("admin database");
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(db_name)
            .fetch_one(&pool)
            .await
            .expect("database lookup");
    if !exists {
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db_name}")))
            .execute(&pool)
            .await
            .expect("create test database");
    }
}

fn database_name(url: &str) -> &str {
    let base = url.split('?').next().unwrap_or(url);
    let name = base.rsplit('/').next().unwrap_or("edurust_test");
    assert!(
        name.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'),
        "test database name must be a plain identifier"
    );
    name
}

fn with_database(url: &str, database: &str) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let Some((prefix, _)) = base.rsplit_once('/') else {
        return url.to_string();
    };
    if query.is_empty() {
        format!("{prefix}/{database}")
    } else {
        format!("{prefix}/{database}?{query}")
    }
}

async fn truncate(pool: &PgPool) {
    sqlx::query(
        r#"
        TRUNCATE TABLE
            assessment_attempt_answers,
            assessment_attempts,
            task_attempts,
            lesson_progress,
            question_mastery,
            verification_tokens,
            sessions,
            password_credentials,
            user_identities,
            users,
            assessment_items,
            assessments,
            task_grading,
            tasks,
            blocks,
            lesson_localizations,
            lessons,
            subtopic_localizations,
            subtopics,
            topic_localizations,
            topics,
            course_localizations,
            courses,
            content_releases
        RESTART IDENTITY CASCADE
        "#,
    )
    .execute(pool)
    .await
    .expect("truncate");
}

struct Client {
    app: axum::Router,
    cookies: Vec<(String, String)>,
}

impl Client {
    fn new(app: axum::Router) -> Self {
        Self {
            app,
            cookies: Vec::new(),
        }
    }

    async fn send(
        &mut self,
        method: &str,
        path: &str,
        body: Option<Value>,
        idempotency_key: Option<Uuid>,
        csrf: bool,
    ) -> (StatusCode, Value, HeaderMap) {
        let mut builder = Request::builder().method(method).uri(path);
        if !self.cookies.is_empty() {
            let cookie = self
                .cookies
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            builder = builder.header(header::COOKIE, cookie);
        }
        if csrf {
            let token = self
                .cookies
                .iter()
                .find(|(name, _)| name == "edurust_csrf")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            builder = builder.header("x-csrf-token", token);
        }
        if let Some(key) = idempotency_key {
            builder = builder.header("idempotency-key", key.to_string());
        }
        let request = if let Some(body) = body {
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        if let Some(values) = response
            .headers()
            .get_all(header::SET_COOKIE)
            .into_iter()
            .next()
        {
            let _ = values;
        }
        for value in response.headers().get_all(header::SET_COOKIE) {
            let text = value.to_str().unwrap_or("");
            let pair = text.split(';').next().unwrap_or("");
            if let Some((name, cookie_value)) = pair.split_once('=') {
                self.cookies.retain(|(existing, _)| existing != name);
                if !cookie_value.is_empty() {
                    self.cookies
                        .push((name.to_string(), cookie_value.to_string()));
                }
            }
        }
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, json, headers)
    }
}

use axum::http::HeaderMap;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/rust-mini")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[tokio::test]
async fn auth_and_learning_flow() {
    let test = test_app().await;
    let mut client = Client::new(test.app.clone());
    let (status, csrf, _) = client.send("GET", "/v1/auth/csrf", None, None, false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(csrf["token"].as_str().is_some());

    let email = format!("learner-{}@example.com", Uuid::new_v4().simple());
    let (status, created, _) = client
        .send(
            "POST",
            "/v1/auth/register",
            Some(json!({ "email": email, "password": "correct-horse" })),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, duplicate, _) = client
        .send(
            "POST",
            "/v1/auth/register",
            Some(json!({ "email": email, "password": "correct-horse" })),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{duplicate}");
    assert_eq!(duplicate["error"]["code"], "identifier_taken");

    let (status, missing, _) = client
        .send(
            "POST",
            "/v1/auth/login",
            Some(json!({ "email": "missing@example.com", "password": "correct-horse" })),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{missing}");
    assert_eq!(missing["error"]["code"], "invalid_credentials");
    let (status, bad_password, _) = client
        .send(
            "POST",
            "/v1/auth/login",
            Some(json!({ "email": email, "password": "wrong-password" })),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{bad_password}");
    assert_eq!(bad_password["error"]["code"], "invalid_credentials");

    let (status, logged_in, _) = client
        .send(
            "POST",
            "/v1/auth/login",
            Some(json!({ "email": email, "password": "correct-horse" })),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{logged_in}");
    let (status, me, _) = client.send("GET", "/v1/auth/me", None, None, false).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["user"]["email"], email);

    let session = client
        .cookies
        .iter()
        .find(|(name, _)| name == "edurust_session")
        .unwrap()
        .1
        .clone();
    client.cookies.retain(|(name, _)| name != "edurust_session");
    let (status, _) = {
        let request = Request::builder()
            .method("GET")
            .uri("/v1/auth/me")
            .header(header::AUTHORIZATION, format!("Bearer {session}"))
            .body(Body::empty())
            .unwrap();
        let response = client.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, bytes)
    };
    assert_eq!(status, StatusCode::OK);
    client
        .cookies
        .push(("edurust_session".to_string(), session));

    let (status, _, _) = client
        .send("POST", "/v1/auth/logout", None, None, true)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = client.send("GET", "/v1/auth/me", None, None, false).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _, _) = client
        .send(
            "POST",
            "/v1/auth/login",
            Some(json!({ "email": email, "password": "correct-horse" })),
            None,
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let first = import_course(&test.pool, &fixture()).await.unwrap();
    let second = import_course(&test.pool, &fixture()).await.unwrap();
    assert!(matches!(first, ImportOutcome::Imported { .. }));
    assert!(matches!(second, ImportOutcome::Unchanged { .. }));
    let courses: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM courses")
        .fetch_one(&test.pool)
        .await
        .unwrap();
    assert_eq!(courses, 1);

    let temp = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), temp.path());
    let course_toml = temp.path().join("course.toml");
    let mut text = fs::read_to_string(&course_toml).unwrap();
    text = text.replace(
        "A short fixture about bindings in Rust.",
        "A changed fixture about bindings in Rust.",
    );
    fs::write(&course_toml, text).unwrap();
    let conflict = import_course(&test.pool, temp.path()).await;
    assert!(matches!(
        conflict,
        Err(edurust_api::error::AppError::RevisionConflict)
    ));

    let (status, lesson, _) = client
        .send("GET", "/v1/lessons/bindings?locale=en", None, None, false)
        .await;
    assert_eq!(status, StatusCode::OK, "{lesson}");
    let raw = lesson.to_string();
    assert!(!raw.contains("FIXTURE_GRADING_ONLY"));
    assert!(!raw.contains("accepted_answers"));
    assert!(!raw.contains("correct_choice_ids"));

    let (status, hidden_ru, _) = client
        .send("GET", "/v1/lessons/bindings?locale=ru", None, None, false)
        .await;
    assert_eq!(status, StatusCode::OK, "{hidden_ru}");
    assert_eq!(hidden_ru["locale_used"], "en");
    assert_eq!(hidden_ru["title"], "What is a binding");
    let hidden_raw = hidden_ru.to_string();
    assert!(!hidden_raw.contains("Что такое связывание"));
    assert!(!hidden_raw.contains("Що таке зв'язування"));

    let (status, hidden_uk, _) = client
        .send("GET", "/v1/lessons/bindings?locale=uk", None, None, false)
        .await;
    assert_eq!(status, StatusCode::OK, "{hidden_uk}");
    assert_eq!(hidden_uk["locale_used"], "en");
    assert_eq!(hidden_uk["title"], "What is a binding");
    assert!(!hidden_uk.to_string().contains("Що таке зв'язування"));

    let wrong_key = Uuid::new_v4();
    let (status, wrong, _) = client
        .send(
            "POST",
            "/v1/tasks/choose-let/attempts?locale=en",
            Some(json!({ "type": "single_choice", "choice_id": "var" })),
            Some(wrong_key),
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{wrong}");
    assert_eq!(wrong["correct"], false);
    let feedback = wrong["feedback"].as_str().unwrap();
    assert!(!feedback.contains("FIXTURE_GRADING_ONLY"));
    assert!(feedback.contains("binding keyword"));
    assert_eq!(wrong["correct_choice_ids"], json!(["let"]));

    let (status, replay, _) = client
        .send(
            "POST",
            "/v1/tasks/choose-let/attempts?locale=en",
            Some(json!({ "type": "single_choice", "choice_id": "let" })),
            Some(wrong_key),
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(replay["attempt_id"], wrong["attempt_id"]);
    assert_eq!(replay["correct"], false);
    let attempt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_attempts")
        .fetch_one(&test.pool)
        .await
        .unwrap();
    assert_eq!(attempt_count, 1);

    let (status, correct, _) = client
        .send(
            "POST",
            "/v1/tasks/choose-let/attempts?locale=en",
            Some(json!({ "type": "single_choice", "choice_id": "let" })),
            Some(Uuid::new_v4()),
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{correct}");
    assert_eq!(correct["correct"], true);
    let (status, multi, _) = client
        .send(
            "POST",
            "/v1/tasks/choose-mut/attempts?locale=en",
            Some(json!({ "type": "multiple_choice", "choice_ids": ["name", "immutable"] })),
            Some(Uuid::new_v4()),
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{multi}");
    assert_eq!(multi["correct"], true);
    let (status, short, _) = client
        .send(
            "POST",
            "/v1/tasks/binding-name/attempts?locale=ru",
            Some(json!({ "type": "short_answer", "text": " Count " })),
            Some(Uuid::new_v4()),
            true,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{short}");
    assert_eq!(short["correct"], true);
    assert_eq!(short["locale_used"], "en");

    let (status, reloaded, _) = client
        .send("GET", "/v1/lessons/bindings?locale=en", None, None, false)
        .await;
    assert_eq!(status, StatusCode::OK, "{reloaded}");
    assert_eq!(reloaded["progress"]["status"], "completed");
    assert!(
        reloaded
            .to_string()
            .contains(correct["attempt_id"].as_str().unwrap())
    );

    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&test.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO question_mastery (user_id, task_id, state) VALUES ($1, 'choose-let', 'good')",
    )
    .bind(user_id)
    .execute(&test.pool)
    .await
    .unwrap();
    let release_before: Uuid = sqlx::query_scalar(
        "SELECT release_id FROM task_attempts WHERE task_id = 'choose-let' AND correct = true",
    )
    .fetch_one(&test.pool)
    .await
    .unwrap();

    let revised = tempfile::tempdir().unwrap();
    copy_dir(&fixture(), revised.path());
    let mut course = fs::read_to_string(revised.path().join("course.toml")).unwrap();
    course = course.replace("revision = 2", "revision = 3");
    fs::write(revised.path().join("course.toml"), course).unwrap();
    let mut grading = fs::read_to_string(revised.path().join("grading/choose-let.toml")).unwrap();
    grading = grading.replace("introduces a binding", "introduces the binding");
    fs::write(revised.path().join("grading/choose-let.toml"), grading).unwrap();
    let imported = import_course(&test.pool, revised.path()).await.unwrap();
    assert!(matches!(
        imported,
        ImportOutcome::Imported { revision: 3, .. }
    ));
    let state: String = sqlx::query_scalar(
        "SELECT state FROM question_mastery WHERE user_id = $1 AND task_id = 'choose-let'",
    )
    .bind(user_id)
    .fetch_one(&test.pool)
    .await
    .unwrap();
    assert_eq!(state, "needs_review");
    let release_after: Uuid = sqlx::query_scalar(
        "SELECT release_id FROM task_attempts WHERE task_id = 'choose-let' AND correct = true",
    )
    .fetch_one(&test.pool)
    .await
    .unwrap();
    assert_eq!(release_before, release_after);
    let releases: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content_releases")
        .fetch_one(&test.pool)
        .await
        .unwrap();
    assert_eq!(releases, 2);
}
