use crate::content::model::{GradingSpec, TaskType, casefold};
use crate::dto::AttemptRequest;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeOutcome {
    pub correct: bool,
    pub correct_choice_ids: Option<Vec<String>>,
}

pub fn grade(
    task_type: TaskType,
    spec: &GradingSpec,
    answer: &AttemptRequest,
) -> Result<GradeOutcome, AppError> {
    match task_type {
        TaskType::SingleChoice => {
            if answer.kind != "single_choice" {
                return Err(AppError::validation("answer type does not match the task"));
            }
            let choice_id = answer
                .choice_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::validation("choice_id is required"))?;
            let correct = spec.correct_choice_ids.len() == 1
                && spec.correct_choice_ids.first().map(String::as_str) == Some(choice_id);
            Ok(GradeOutcome {
                correct,
                correct_choice_ids: Some(spec.correct_choice_ids.clone()),
            })
        }
        TaskType::MultipleChoice => {
            if answer.kind != "multiple_choice" {
                return Err(AppError::validation("answer type does not match the task"));
            }
            let submitted = answer.choice_ids.clone().unwrap_or_default();
            if submitted.is_empty() {
                return Err(AppError::validation("choice_ids is required"));
            }
            let mut submitted_set: Vec<&str> = submitted.iter().map(String::as_str).collect();
            submitted_set.sort_unstable();
            submitted_set.dedup();
            let mut expected: Vec<&str> =
                spec.correct_choice_ids.iter().map(String::as_str).collect();
            expected.sort_unstable();
            Ok(GradeOutcome {
                correct: submitted_set == expected,
                correct_choice_ids: Some(spec.correct_choice_ids.clone()),
            })
        }
        TaskType::ShortAnswer => {
            if answer.kind != "short_answer" {
                return Err(AppError::validation("answer type does not match the task"));
            }
            let text = answer
                .text
                .as_deref()
                .ok_or_else(|| AppError::validation("text is required"))?;
            let correct = match spec.normalization.as_deref() {
                Some("trim_casefold") => {
                    let folded = casefold(text);
                    spec.accepted_answers
                        .iter()
                        .any(|answer| casefold(answer) == folded)
                }
                _ => false,
            };
            Ok(GradeOutcome {
                correct,
                correct_choice_ids: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::model::{FeedbackSpec, Localized};

    fn spec(correct: &[&str], accepted: &[&str]) -> GradingSpec {
        GradingSpec {
            correct_choice_ids: correct.iter().map(|value| (*value).to_string()).collect(),
            normalization: Some("trim_casefold".to_string()),
            accepted_answers: accepted.iter().map(|value| (*value).to_string()).collect(),
            feedback: FeedbackSpec {
                correct: Localized {
                    en: "yes".to_string(),
                    ru: "да".to_string(),
                    uk: "так".to_string(),
                },
                incorrect: Localized {
                    en: "no".to_string(),
                    ru: "нет".to_string(),
                    uk: "ні".to_string(),
                },
            },
        }
    }

    #[test]
    fn grades_single_and_multiple_choice() {
        let single = spec(&["let"], &[]);
        let correct = grade(
            TaskType::SingleChoice,
            &single,
            &AttemptRequest {
                kind: "single_choice".to_string(),
                choice_id: Some("let".to_string()),
                choice_ids: None,
                text: None,
            },
        )
        .unwrap();
        assert!(correct.correct);
        let wrong = grade(
            TaskType::SingleChoice,
            &single,
            &AttemptRequest {
                kind: "single_choice".to_string(),
                choice_id: Some("var".to_string()),
                choice_ids: None,
                text: None,
            },
        )
        .unwrap();
        assert!(!wrong.correct);

        let multi = spec(&["immutable", "name"], &[]);
        let outcome = grade(
            TaskType::MultipleChoice,
            &multi,
            &AttemptRequest {
                kind: "multiple_choice".to_string(),
                choice_id: None,
                choice_ids: Some(vec!["name".to_string(), "immutable".to_string()]),
                text: None,
            },
        )
        .unwrap();
        assert!(outcome.correct);
    }

    #[test]
    fn grades_short_answer_with_casefold() {
        let spec = spec(&[], &["count"]);
        let outcome = grade(
            TaskType::ShortAnswer,
            &spec,
            &AttemptRequest {
                kind: "short_answer".to_string(),
                choice_id: None,
                choice_ids: None,
                text: Some("  Count  ".to_string()),
            },
        )
        .unwrap();
        assert!(outcome.correct);
        assert!(outcome.correct_choice_ids.is_none());
    }
}
