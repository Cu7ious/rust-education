use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserBody {
    pub id: Uuid,
    pub email: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub user: UserBody,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CsrfResponse {
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AttemptRequest {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub choice_id: Option<String>,
    #[serde(default)]
    pub choice_ids: Option<Vec<String>>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AttemptResponse {
    pub attempt_id: Uuid,
    pub task_id: String,
    pub correct: bool,
    pub feedback: String,
    pub locale_used: String,
    pub correct_choice_ids: Option<Vec<String>>,
    pub submitted_answer: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CourseSummary {
    pub id: String,
    pub title: String,
    pub description: String,
    pub locale_used: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CourseListResponse {
    pub courses: Vec<CourseSummary>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LessonLink {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SubtopicNode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub lessons: Vec<LessonLink>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TopicNode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub subtopics: Vec<SubtopicNode>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CourseDetail {
    pub id: String,
    pub title: String,
    pub description: String,
    pub locale_used: String,
    pub topics: Vec<TopicNode>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChoiceView {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskView {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ChoiceView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_attempt: Option<AttemptResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockView {
    Text {
        id: String,
        markdown: String,
    },
    Video {
        id: String,
        url: String,
        title: String,
    },
    Task {
        id: String,
        task: Box<TaskView>,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LessonProgressBody {
    pub status: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LessonResponse {
    pub id: String,
    pub title: String,
    pub description: String,
    pub locale_used: String,
    pub course_id: String,
    pub topic_id: String,
    pub subtopic_id: String,
    pub progress: LessonProgressBody,
    pub blocks: Vec<BlockView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LessonProgressItem {
    pub lesson_id: String,
    pub status: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProgressResponse {
    pub lessons: Vec<LessonProgressItem>,
}

#[derive(Debug, Deserialize)]
pub struct LocaleQuery {
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProgressQuery {
    pub locale: Option<String>,
    pub course_id: Option<String>,
}
