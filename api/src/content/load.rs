use std::fs;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::content::model::{
    AssessmentPackage, BlockPackage, CoursePackage, FeedbackSpec, GradingSpec, LessonPackage,
    Localized, PublicChoice, PublicTaskPayload, Purpose, SubtopicPackage, TaskPackage, TaskType,
    TopicPackage,
};
use crate::error::AppError;

pub fn load_package(root: &Path) -> Result<CoursePackage, AppError> {
    let manifest_hash = manifest_hash(root)?;
    let course_path = root.join("course.toml");
    let course_file: CourseFile = read_struct(
        &course_path,
        &[
            "id",
            "revision",
            "default_locale",
            "title",
            "description",
            "topics",
        ],
    )?;
    require_slug(&course_file.id, &display(&course_path))?;
    if course_file.revision < 1 {
        return Err(AppError::content(format!(
            "{}: revision must be >= 1",
            display(&course_path)
        )));
    }
    if course_file.default_locale != "en" && course_file.default_locale != "uk" {
        return Err(AppError::content(
            "default_locale must be en or uk".to_string(),
        ));
    }
    require_localized(&course_file.title, &display(&course_path), "title")?;
    require_localized(
        &course_file.description,
        &display(&course_path),
        "description",
    )?;
    if course_file.topics.is_empty() {
        return Err(AppError::content("course must contain a topic"));
    }
    expect_dir_set(&root.join("topics"), &course_file.topics)?;

    let mut topics = Vec::new();
    for (index, topic_id) in course_file.topics.iter().enumerate() {
        topics.push(load_topic(
            root,
            topic_id,
            i32::try_from(index + 1).unwrap_or(1),
        )?);
    }
    let assessment = load_assessment(&root.join("assessment.toml"), "course")?;
    let package = CoursePackage {
        course_id: course_file.id,
        revision: course_file.revision,
        default_locale: course_file.default_locale,
        title: course_file.title,
        description: course_file.description,
        manifest_hash,
        topics,
        assessment,
    };
    validate_references(&package)?;
    Ok(package)
}

fn load_topic(root: &Path, topic_id: &str, position: i32) -> Result<TopicPackage, AppError> {
    let dir = root.join("topics").join(topic_id);
    let path = dir.join("topic.toml");
    let file: NodeFile = read_struct(&path, &["id", "title", "description", "subtopics"])?;
    if file.id != topic_id {
        return Err(AppError::content(format!(
            "{}: id must match the directory name",
            display(&path)
        )));
    }
    require_slug(&file.id, &display(&path))?;
    require_localized(&file.title, &display(&path), "title")?;
    require_localized(&file.description, &display(&path), "description")?;
    let subtopics_ids = file.subtopics.unwrap_or_default();
    if subtopics_ids.is_empty() {
        return Err(AppError::content(format!(
            "{}: topic must contain a subtopic",
            display(&path)
        )));
    }
    expect_dir_set(&dir.join("subtopics"), &subtopics_ids)?;
    let mut subtopics = Vec::new();
    for (index, subtopic_id) in subtopics_ids.iter().enumerate() {
        subtopics.push(load_subtopic(
            &dir,
            subtopic_id,
            i32::try_from(index + 1).unwrap_or(1),
        )?);
    }
    Ok(TopicPackage {
        id: file.id,
        title: file.title,
        description: file.description,
        position,
        subtopics,
    })
}

fn load_subtopic(
    topic_dir: &Path,
    subtopic_id: &str,
    position: i32,
) -> Result<SubtopicPackage, AppError> {
    let dir = topic_dir.join("subtopics").join(subtopic_id);
    let path = dir.join("subtopic.toml");
    let file: NodeFile = read_struct(&path, &["id", "title", "description", "lessons"])?;
    if file.id != subtopic_id {
        return Err(AppError::content(format!(
            "{}: id must match the directory name",
            display(&path)
        )));
    }
    require_slug(&file.id, &display(&path))?;
    require_localized(&file.title, &display(&path), "title")?;
    require_localized(&file.description, &display(&path), "description")?;
    let lesson_ids = file.lessons.unwrap_or_default();
    if lesson_ids.is_empty() {
        return Err(AppError::content(format!(
            "{}: subtopic must contain a lesson",
            display(&path)
        )));
    }
    expect_dir_set(&dir.join("lessons"), &lesson_ids)?;
    let mut lessons = Vec::new();
    for (index, lesson_id) in lesson_ids.iter().enumerate() {
        lessons.push(load_lesson(
            &dir,
            lesson_id,
            i32::try_from(index + 1).unwrap_or(1),
        )?);
    }
    let assessment = load_assessment(&dir.join("assessment.toml"), "subtopic")?;
    Ok(SubtopicPackage {
        id: file.id,
        title: file.title,
        description: file.description,
        position,
        lessons,
        assessment,
    })
}

fn load_lesson(
    subtopic_dir: &Path,
    lesson_id: &str,
    position: i32,
) -> Result<LessonPackage, AppError> {
    let dir = subtopic_dir.join("lessons").join(lesson_id);
    let path = dir.join("lesson.toml");
    let file: LessonFile = read_struct(&path, &["id", "title", "description", "blocks"])?;
    if file.id != lesson_id {
        return Err(AppError::content(format!(
            "{}: id must match the directory name",
            display(&path)
        )));
    }
    require_slug(&file.id, &display(&path))?;
    require_localized(&file.title, &display(&path), "title")?;
    require_localized(&file.description, &display(&path), "description")?;
    if file.blocks.is_empty() {
        return Err(AppError::content(format!(
            "{}: lesson must contain a block",
            display(&path)
        )));
    }
    let mut blocks = Vec::new();
    for (index, block_id) in file.blocks.iter().enumerate() {
        blocks.push(load_block(
            &dir.join("blocks"),
            block_id,
            i32::try_from(index + 1).unwrap_or(1),
        )?);
    }
    Ok(LessonPackage {
        id: file.id,
        title: file.title,
        description: file.description,
        position,
        blocks,
    })
}

fn load_block(dir: &Path, block_id: &str, position: i32) -> Result<BlockPackage, AppError> {
    require_slug(block_id, block_id)?;
    let toml_path = dir.join(format!("{block_id}.toml"));
    let en_path = dir.join(format!("{block_id}.en.md"));
    let ru_path = dir.join(format!("{block_id}.ru.md"));
    let uk_path = dir.join(format!("{block_id}.uk.md"));
    match (
        toml_path.exists(),
        en_path.exists() || ru_path.exists() || uk_path.exists(),
    ) {
        (true, true) => Err(AppError::content(format!(
            "{block_id}: a block cannot have both markdown and toml"
        ))),
        (false, false) => Err(AppError::content(format!(
            "{}: missing block `{block_id}`",
            display(dir)
        ))),
        (false, _) => {
            let en = read_markdown(&en_path)?;
            let ru = read_markdown(&ru_path)?;
            let uk = read_markdown(&uk_path)?;
            Ok(BlockPackage::Text {
                id: block_id.to_string(),
                position,
                markdown: Localized { en, ru, uk },
            })
        }
        (true, false) => {
            let file: BlockFile = read_struct(
                &toml_path,
                &[
                    "id",
                    "kind",
                    "task_id",
                    "task_type",
                    "purpose",
                    "prompt",
                    "choices",
                    "url",
                    "title",
                ],
            )?;
            if file.id != block_id {
                return Err(AppError::content(format!(
                    "{}: id must match the file name",
                    display(&toml_path)
                )));
            }
            match file.kind.as_str() {
                "video" => {
                    let url = file.url.unwrap_or_default();
                    if !url.starts_with("https://") && !url.starts_with("http://") {
                        return Err(AppError::content(format!(
                            "{}: video url must be http or https",
                            display(&toml_path)
                        )));
                    }
                    let title = file.title.ok_or_else(|| {
                        AppError::content(format!(
                            "{}: video title is required",
                            display(&toml_path)
                        ))
                    })?;
                    require_localized(&title, &display(&toml_path), "title")?;
                    Ok(BlockPackage::Video {
                        id: file.id,
                        position,
                        url,
                        title,
                    })
                }
                "task" => {
                    let task_id = file.task_id.unwrap_or_else(|| file.id.clone());
                    require_slug(&task_id, &display(&toml_path))?;
                    let task_type = TaskType::parse(file.task_type.as_deref().unwrap_or(""))
                        .ok_or_else(|| {
                            AppError::content(format!("{}: unknown task_type", display(&toml_path)))
                        })?;
                    let purpose = Purpose::parse(file.purpose.as_deref().unwrap_or("lesson"))
                        .ok_or_else(|| {
                            AppError::content(format!("{}: unknown purpose", display(&toml_path)))
                        })?;
                    let prompt = file.prompt.ok_or_else(|| {
                        AppError::content(format!("{}: prompt is required", display(&toml_path)))
                    })?;
                    require_localized(&prompt, &display(&toml_path), "prompt")?;
                    let mut choices = Vec::new();
                    for choice in file.choices {
                        require_slug(&choice.id, &display(&toml_path))?;
                        require_localized(&choice.text, &display(&toml_path), "choice")?;
                        choices.push(choice);
                    }
                    if matches!(task_type, TaskType::SingleChoice | TaskType::MultipleChoice)
                        && choices.is_empty()
                    {
                        return Err(AppError::content(format!(
                            "{}: choices are required",
                            display(&toml_path)
                        )));
                    }
                    Ok(BlockPackage::Task {
                        id: file.id,
                        position,
                        task: Box::new(TaskPackage {
                            id: task_id,
                            task_type,
                            purpose,
                            public_payload: PublicTaskPayload { prompt, choices },
                            grading: GradingSpec {
                                correct_choice_ids: Vec::new(),
                                normalization: None,
                                accepted_answers: Vec::new(),
                                feedback: FeedbackSpec {
                                    correct: Localized {
                                        en: String::new(),
                                        ru: String::new(),
                                        uk: String::new(),
                                    },
                                    incorrect: Localized {
                                        en: String::new(),
                                        ru: String::new(),
                                        uk: String::new(),
                                    },
                                },
                            },
                            grading_hash: String::new(),
                        }),
                    })
                }
                _ => Err(AppError::content(format!(
                    "{}: unknown block kind",
                    display(&toml_path)
                ))),
            }
        }
    }
}

fn load_assessment(path: &Path, expected_scope: &str) -> Result<AssessmentPackage, AppError> {
    let file: AssessmentFile = read_struct(path, &["id", "scope", "task_ids", "pass_percent"])?;
    require_slug(&file.id, &display(path))?;
    if file.scope != expected_scope {
        return Err(AppError::content(format!(
            "{}: scope must be {expected_scope}",
            display(path)
        )));
    }
    if file.task_ids.is_empty() {
        return Err(AppError::content(format!(
            "{}: assessment needs at least one task",
            display(path)
        )));
    }
    let pass_percent = file.pass_percent.unwrap_or(80);
    if !(1..=100).contains(&pass_percent) {
        return Err(AppError::content(format!(
            "{}: pass_percent must be from 1 to 100",
            display(path)
        )));
    }
    Ok(AssessmentPackage {
        id: file.id,
        task_ids: file.task_ids,
        pass_percent,
    })
}

fn validate_references(package: &CoursePackage) -> Result<(), AppError> {
    let mut topics = std::collections::BTreeSet::new();
    let mut subtopics = std::collections::BTreeSet::new();
    let mut lessons = std::collections::BTreeSet::new();
    let mut blocks = std::collections::BTreeSet::new();
    let mut tasks = std::collections::BTreeMap::new();
    let mut assessments = std::collections::BTreeSet::new();
    insert_id(&mut assessments, &package.assessment.id)?;
    for topic in &package.topics {
        insert_id(&mut topics, &topic.id)?;
        for subtopic in &topic.subtopics {
            insert_id(&mut subtopics, &subtopic.id)?;
            insert_id(&mut assessments, &subtopic.assessment.id)?;
            for lesson in &subtopic.lessons {
                insert_id(&mut lessons, &lesson.id)?;
                for block in &lesson.blocks {
                    insert_id(&mut blocks, block.id())?;
                    if let BlockPackage::Task { task, .. } = block
                        && tasks.insert(task.id.clone(), task.as_ref()).is_some()
                    {
                        return Err(AppError::content(format!(
                            "duplicate task id `{}`",
                            task.id
                        )));
                    }
                }
            }
        }
    }
    check_assessment(&package.assessment, &tasks)?;
    for topic in &package.topics {
        for subtopic in &topic.subtopics {
            check_assessment(&subtopic.assessment, &tasks)?;
        }
    }
    Ok(())
}

fn check_assessment(
    assessment: &AssessmentPackage,
    tasks: &std::collections::BTreeMap<String, &TaskPackage>,
) -> Result<(), AppError> {
    for task_id in &assessment.task_ids {
        if !tasks.contains_key(task_id) {
            return Err(AppError::content(format!(
                "assessment `{}` references unknown task `{task_id}`",
                assessment.id
            )));
        }
    }
    Ok(())
}

fn insert_id(seen: &mut std::collections::BTreeSet<String>, id: &str) -> Result<(), AppError> {
    if !seen.insert(id.to_string()) {
        return Err(AppError::content(format!("duplicate id `{id}`")));
    }
    Ok(())
}

fn manifest_hash(root: &Path) -> Result<String, AppError> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(
    base: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), AppError> {
    let entries = fs::read_dir(dir)
        .map_err(|error| AppError::content(format!("{}: {error}", display(dir))))?;
    for entry in entries {
        let entry = entry.map_err(|error| AppError::content(error.to_string()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_files(base, &path, out)?;
            continue;
        }
        let relative = path.strip_prefix(base).unwrap_or(&path);
        let key = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let bytes = fs::read(&path)
            .map_err(|error| AppError::content(format!("{}: {error}", display(&path))))?;
        out.push((key, bytes));
    }
    Ok(())
}

fn read_markdown(path: &Path) -> Result<String, AppError> {
    let text = fs::read_to_string(path)
        .map_err(|_| AppError::content(format!("{}: markdown is required", display(path))))?;
    if text.trim().is_empty() {
        return Err(AppError::content(format!(
            "{}: markdown is empty",
            display(path)
        )));
    }
    if text.len() > 100_000 {
        return Err(AppError::content(format!(
            "{}: markdown is too large",
            display(path)
        )));
    }
    Ok(text)
}

fn read_struct<T: for<'de> Deserialize<'de>>(path: &Path, allowed: &[&str]) -> Result<T, AppError> {
    let text = fs::read_to_string(path)
        .map_err(|error| AppError::content(format!("{}: {error}", display(path))))?;
    let value: toml::Value = toml::from_str(&text)
        .map_err(|error| AppError::content(format!("{}: {error}", display(path))))?;
    let table = value
        .as_table()
        .ok_or_else(|| AppError::content(format!("{}: expected a table", display(path))))?;
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(AppError::content(format!(
                "{}: unknown field `{key}`",
                display(path)
            )));
        }
    }
    value
        .try_into()
        .map_err(|error: toml::de::Error| AppError::content(format!("{}: {error}", display(path))))
}

fn expect_dir_set(dir: &Path, expected: &[String]) -> Result<(), AppError> {
    let mut found = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|error| AppError::content(format!("{}: {error}", display(dir))))?;
    for entry in entries {
        let entry = entry.map_err(|error| AppError::content(error.to_string()))?;
        if entry.path().is_dir() {
            found.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    found.sort();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort();
    if found != expected_sorted {
        return Err(AppError::content(format!(
            "{}: directories {found:?} do not match {expected_sorted:?}",
            display(dir)
        )));
    }
    Ok(())
}

fn require_slug(id: &str, path: &str) -> Result<(), AppError> {
    let ok = !id.is_empty()
        && id.len() <= 80
        && id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
    if ok {
        Ok(())
    } else {
        Err(AppError::content(format!(
            "{path}: `{id}` is not a stable slug"
        )))
    }
}

fn require_localized(value: &Localized, path: &str, field: &str) -> Result<(), AppError> {
    if value.en.trim().is_empty() || value.ru.trim().is_empty() || value.uk.trim().is_empty() {
        Err(AppError::content(format!(
            "{path}: {field} requires en, ru, and uk text"
        )))
    } else {
        Ok(())
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[derive(Deserialize)]
struct CourseFile {
    id: String,
    revision: i32,
    default_locale: String,
    title: Localized,
    description: Localized,
    topics: Vec<String>,
}

#[derive(Deserialize)]
struct NodeFile {
    id: String,
    title: Localized,
    description: Localized,
    subtopics: Option<Vec<String>>,
    lessons: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct LessonFile {
    id: String,
    title: Localized,
    description: Localized,
    blocks: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BlockFile {
    id: String,
    kind: String,
    task_id: Option<String>,
    task_type: Option<String>,
    purpose: Option<String>,
    prompt: Option<Localized>,
    #[serde(default)]
    choices: Vec<PublicChoice>,
    url: Option<String>,
    title: Option<Localized>,
}

#[derive(Deserialize)]
struct AssessmentFile {
    id: String,
    scope: String,
    task_ids: Vec<String>,
    pass_percent: Option<i32>,
}

pub fn grading_hash(spec: &GradingSpec) -> Result<String, AppError> {
    let mut correct = spec.correct_choice_ids.clone();
    correct.sort();
    let mut accepted = spec.accepted_answers.clone();
    accepted.sort();
    let canonical = serde_json::json!({
        "accepted_answers": accepted,
        "correct_choice_ids": correct,
        "feedback": spec.feedback,
        "normalization": spec.normalization,
    });
    let bytes = serde_json::to_vec(&canonical).map_err(|_| AppError::Internal)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub fn load_grading(root: &Path, task_id: &str) -> Result<GradingSpec, AppError> {
    let path = root.join("grading").join(format!("{task_id}.toml"));
    let spec: GradingSpec = read_struct(
        &path,
        &[
            "correct_choice_ids",
            "normalization",
            "accepted_answers",
            "feedback",
        ],
    )?;
    require_localized(&spec.feedback.correct, &display(&path), "feedback.correct")?;
    require_localized(
        &spec.feedback.incorrect,
        &display(&path),
        "feedback.incorrect",
    )?;
    Ok(spec)
}

pub fn finish_package(root: &Path, package: &mut CoursePackage) -> Result<(), AppError> {
    let mut expected = std::collections::BTreeSet::new();
    for task in package.tasks() {
        expected.insert(task.id.clone());
    }
    let grading_dir = root.join("grading");
    let mut found = std::collections::BTreeSet::new();
    let entries = fs::read_dir(&grading_dir)
        .map_err(|error| AppError::content(format!("{}: {error}", display(&grading_dir))))?;
    for entry in entries {
        let entry = entry.map_err(|error| AppError::content(error.to_string()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(task_id) = name.strip_suffix(".toml") {
            found.insert(task_id.to_string());
        }
    }
    if found != expected {
        return Err(AppError::content(format!(
            "grading files {found:?} do not match tasks {expected:?}"
        )));
    }
    // tasks() returns references into package, so reload ids then mutate.
    let task_ids: Vec<String> = package
        .tasks()
        .into_iter()
        .map(|task| task.id.clone())
        .collect();
    for task_id in task_ids {
        let spec = load_grading(root, &task_id)?;
        let hash = grading_hash(&spec)?;
        apply_grading(package, &task_id, spec, hash)?;
    }
    Ok(())
}

fn apply_grading(
    package: &mut CoursePackage,
    task_id: &str,
    spec: GradingSpec,
    hash: String,
) -> Result<(), AppError> {
    for topic in &mut package.topics {
        for subtopic in &mut topic.subtopics {
            for lesson in &mut subtopic.lessons {
                for block in &mut lesson.blocks {
                    if let BlockPackage::Task { task, .. } = block
                        && task.id == task_id
                    {
                        validate_grading(task.task_type, &task.public_payload, &spec)?;
                        task.grading = spec;
                        task.grading_hash = hash;
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(AppError::content(format!("missing task `{task_id}`")))
}

fn validate_grading(
    task_type: TaskType,
    public: &PublicTaskPayload,
    spec: &GradingSpec,
) -> Result<(), AppError> {
    let choice_ids: std::collections::BTreeSet<&str> = public
        .choices
        .iter()
        .map(|choice| choice.id.as_str())
        .collect();
    if choice_ids.len() != public.choices.len() {
        return Err(AppError::content("choice ids must be unique"));
    }
    match task_type {
        TaskType::SingleChoice => {
            if spec.correct_choice_ids.len() != 1 {
                return Err(AppError::content(
                    "single_choice grading needs exactly one correct choice",
                ));
            }
            if !choice_ids.contains(spec.correct_choice_ids[0].as_str()) {
                return Err(AppError::content(
                    "correct choice is not one of the public choices",
                ));
            }
        }
        TaskType::MultipleChoice => {
            if spec.correct_choice_ids.is_empty() {
                return Err(AppError::content(
                    "multiple_choice grading needs a correct choice",
                ));
            }
            for id in &spec.correct_choice_ids {
                if !choice_ids.contains(id.as_str()) {
                    return Err(AppError::content(format!(
                        "correct choice `{id}` is not public"
                    )));
                }
            }
        }
        TaskType::ShortAnswer => {
            if spec.normalization.as_deref() != Some("trim_casefold") {
                return Err(AppError::content(
                    "short_answer normalization must be trim_casefold",
                ));
            }
            if spec.accepted_answers.is_empty() {
                return Err(AppError::content(
                    "short_answer grading needs an accepted answer",
                ));
            }
        }
    }
    Ok(())
}

pub fn load_course(root: &Path) -> Result<CoursePackage, AppError> {
    let mut package = load_package(root)?;
    finish_package(root, &mut package)?;
    Ok(package)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_keeps_answers_out_of_public_payloads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/rust-mini");
        let package = load_course(&root).expect("fixture");
        assert_eq!(package.tasks().len(), 3);
        for task in package.tasks() {
            let public = serde_json::to_string(&task.public_payload).unwrap();
            assert!(!public.contains("FIXTURE_GRADING_ONLY"));
            assert!(!public.contains("accepted_answers"));
            assert!(!task.grading_hash.is_empty());
        }
    }
}
