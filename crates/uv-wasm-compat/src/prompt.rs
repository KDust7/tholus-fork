use std::cell::RefCell;

use rustc_hash::FxHashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PromptPolicy {
    AlwaysConfirm,
    AlwaysDeny,
    #[default]
    Refuse,
    Scripted(FxHashMap<String, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    Unavailable(String),
}

impl std::fmt::Display for PromptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(question) => write!(
                formatter,
                "interactive prompt required in the browser: {question}; pass --yes or configure a prompt policy"
            ),
        }
    }
}

impl std::error::Error for PromptError {}

thread_local! {
    static CURRENT: RefCell<PromptPolicy> = RefCell::new(PromptPolicy::default());
}

pub fn set(policy: PromptPolicy) {
    CURRENT.with(|current| {
        *current.borrow_mut() = policy;
    });
}

pub fn reset() {
    set(PromptPolicy::default());
}

pub fn confirm(question: &str) -> Result<bool, PromptError> {
    CURRENT.with(|current| match &*current.borrow() {
        PromptPolicy::AlwaysConfirm => Ok(true),
        PromptPolicy::AlwaysDeny => Ok(false),
        PromptPolicy::Refuse => Err(PromptError::Unavailable(question.to_owned())),
        PromptPolicy::Scripted(answers) => match answers.get(question) {
            Some(answer) => Ok(matches!(
                answer.trim().to_ascii_lowercase().as_str(),
                "y" | "yes"
            )),
            None => Err(PromptError::Unavailable(question.to_owned())),
        },
    })
}

pub fn answer(question: &str) -> Result<String, PromptError> {
    CURRENT.with(|current| match &*current.borrow() {
        PromptPolicy::Scripted(answers) => answers
            .get(question)
            .cloned()
            .ok_or_else(|| PromptError::Unavailable(question.to_owned())),
        _ => Err(PromptError::Unavailable(question.to_owned())),
    })
}

#[cfg(test)]
mod tests {
    use super::{PromptError, PromptPolicy, answer, confirm, reset, set};
    use rustc_hash::FxHashMap;

    fn scripted(pairs: &[(&str, &str)]) -> PromptPolicy {
        let mut answers = FxHashMap::default();
        for (question, response) in pairs {
            answers.insert((*question).to_owned(), (*response).to_owned());
        }
        PromptPolicy::Scripted(answers)
    }

    #[test]
    fn refuses_by_default() {
        reset();
        assert!(matches!(
            confirm("Proceed?"),
            Err(PromptError::Unavailable(_))
        ));
    }

    #[test]
    fn the_refusal_explains_the_remedy() {
        reset();
        let Err(error) = confirm("Proceed?") else {
            panic!("expected a refusal");
        };
        let text = error.to_string();
        assert!(text.contains("Proceed?"));
        assert!(text.contains("--yes"));
    }

    #[test]
    fn always_confirm_accepts() {
        set(PromptPolicy::AlwaysConfirm);
        assert_eq!(confirm("Proceed?"), Ok(true));
        reset();
    }

    #[test]
    fn always_deny_declines() {
        set(PromptPolicy::AlwaysDeny);
        assert_eq!(confirm("Proceed?"), Ok(false));
        reset();
    }

    #[test]
    fn a_scripted_yes_confirms() {
        set(scripted(&[("Proceed?", "y")]));
        assert_eq!(confirm("Proceed?"), Ok(true));
        reset();
    }

    #[test]
    fn a_scripted_no_declines() {
        set(scripted(&[("Proceed?", "n")]));
        assert_eq!(confirm("Proceed?"), Ok(false));
        reset();
    }

    #[test]
    fn scripted_answers_ignore_case_and_padding() {
        set(scripted(&[("Proceed?", "  YES ")]));
        assert_eq!(confirm("Proceed?"), Ok(true));
        reset();
    }

    #[test]
    fn an_unscripted_question_is_refused() {
        set(scripted(&[("Proceed?", "y")]));
        assert!(matches!(
            confirm("Something else?"),
            Err(PromptError::Unavailable(_))
        ));
        reset();
    }

    #[test]
    fn scripted_free_text_is_returned_verbatim() {
        set(scripted(&[("Name?", "rich")]));
        assert_eq!(answer("Name?"), Ok("rich".to_owned()));
        reset();
    }

    #[test]
    fn free_text_needs_a_script() {
        set(PromptPolicy::AlwaysConfirm);
        assert!(matches!(answer("Name?"), Err(PromptError::Unavailable(_))));
        reset();
    }
}
