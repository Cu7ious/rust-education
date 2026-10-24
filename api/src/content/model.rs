use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Localized {
    pub en: String,
    pub ru: String,
    pub uk: String,
}

impl Localized {
    pub fn pick<'a>(&'a self, requested: &str, default_locale: &str) -> (&'a str, &'a str) {
        let _ = (requested, default_locale);
        (self.en.as_str(), "en")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    SingleChoice,
    MultipleChoice,
    ShortAnswer,
}

impl TaskType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleChoice => "single_choice",
            Self::MultipleChoice => "multiple_choice",
            Self::ShortAnswer => "short_answer",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "single_choice" => Some(Self::SingleChoice),
            "multiple_choice" => Some(Self::MultipleChoice),
            "short_answer" => Some(Self::ShortAnswer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Lesson,
    Assessment,
    Both,
}

impl Purpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lesson => "lesson",
            Self::Assessment => "assessment",
            Self::Both => "both",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lesson" => Some(Self::Lesson),
            "assessment" => Some(Self::Assessment),
            "both" => Some(Self::Both),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicChoice {
    pub id: String,
    pub text: Localized,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicTaskPayload {
    pub prompt: Localized,
    #[serde(default)]
    pub choices: Vec<PublicChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GradingSpec {
    #[serde(default)]
    pub correct_choice_ids: Vec<String>,
    #[serde(default)]
    pub normalization: Option<String>,
    #[serde(default)]
    pub accepted_answers: Vec<String>,
    pub feedback: FeedbackSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackSpec {
    pub correct: Localized,
    pub incorrect: Localized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoursePackage {
    pub course_id: String,
    pub revision: i32,
    pub default_locale: String,
    pub title: Localized,
    pub description: Localized,
    pub manifest_hash: String,
    pub topics: Vec<TopicPackage>,
    pub assessment: AssessmentPackage,
}

impl CoursePackage {
    pub fn tasks(&self) -> Vec<&TaskPackage> {
        self.topics
            .iter()
            .flat_map(|topic| topic.subtopics.iter())
            .flat_map(|subtopic| subtopic.lessons.iter())
            .flat_map(|lesson| lesson.blocks.iter())
            .filter_map(|block| match block {
                BlockPackage::Task { task, .. } => Some(task.as_ref()),
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicPackage {
    pub id: String,
    pub title: Localized,
    pub description: Localized,
    pub position: i32,
    pub subtopics: Vec<SubtopicPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtopicPackage {
    pub id: String,
    pub title: Localized,
    pub description: Localized,
    pub position: i32,
    pub lessons: Vec<LessonPackage>,
    pub assessment: AssessmentPackage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonPackage {
    pub id: String,
    pub title: Localized,
    pub description: Localized,
    pub position: i32,
    pub blocks: Vec<BlockPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockPackage {
    Text {
        id: String,
        position: i32,
        markdown: Localized,
    },
    Video {
        id: String,
        position: i32,
        url: String,
        title: Localized,
    },
    Task {
        id: String,
        position: i32,
        task: Box<TaskPackage>,
    },
}

impl BlockPackage {
    pub fn id(&self) -> &str {
        match self {
            Self::Text { id, .. } | Self::Video { id, .. } | Self::Task { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPackage {
    pub id: String,
    pub task_type: TaskType,
    pub purpose: Purpose,
    pub public_payload: PublicTaskPayload,
    pub grading: GradingSpec,
    pub grading_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentPackage {
    pub id: String,
    pub task_ids: Vec<String>,
    pub pass_percent: i32,
}

const FIXTURE_MARKER: &str = "FIXTURE_GRADING_ONLY ";

pub fn visible_feedback(text: &str) -> &str {
    text.strip_prefix(FIXTURE_MARKER).unwrap_or(text)
}

pub fn casefold(value: &str) -> String {
    value.trim().chars().flat_map(char::to_lowercase).collect()
}
