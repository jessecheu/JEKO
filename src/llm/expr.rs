//! A tiny arithmetic evaluator (FR22). A generated problem carries the sum
//! behind its answer as an `expression` such as `(12 + 8) / 4`; this computes
//! it, and the problem is dropped if the result isn't the `answer` the model
//! gave. A wrong answer key is the worst failure an education game can have,
//! so nothing generated reaches a child without passing through here.
//!
//! Grammar: numbers (`12`, `3.5`), `+`, `-` (also `−`), `*` (also `×`),
//! `/` (also `÷`), unary minus and parentheses. Anything else is rejected,
//! and so is division by zero, a result that isn't finite, nesting deeper
//! than `MAX_DEPTH` and input longer than `MAX_LEN`. Pure, no dependencies.

use crate::llm::MathTask;

pub const MAX_LEN: usize = 120;
pub const MAX_DEPTH: usize = 16;

/// Evaluates `src`, or `None` if it isn't a plain arithmetic expression.
pub fn eval(src: &str) -> Option<f64> {
    if src.len() > MAX_LEN {
        return None;
    }
    let mut p = Parser {
        chars: src.chars().collect(),
        at: 0,
        depth: 0,
    };
    let v = p.sum()?;
    p.skip_spaces();
    (p.at == p.chars.len() && v.is_finite()).then_some(v)
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    depth: usize,
}

impl Parser {
    fn skip_spaces(&mut self) {
        while self.chars.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
    }

    /// The next character, past any spaces. Spaces never join the digits on
    /// either side of them: `1 2` is not 12.
    fn peek(&mut self) -> Option<char> {
        self.skip_spaces();
        self.chars.get(self.at).copied()
    }

    fn sum(&mut self) -> Option<f64> {
        let mut v = self.product()?;
        while let Some(c @ ('+' | '-' | '−')) = self.peek() {
            self.at += 1;
            let r = self.product()?;
            v = if c == '+' { v + r } else { v - r };
        }
        Some(v)
    }

    fn product(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        while let Some(c @ ('*' | '×' | '/' | '÷')) = self.peek() {
            self.at += 1;
            let r = self.unary()?;
            if matches!(c, '/' | '÷') {
                if r == 0.0 {
                    return None;
                }
                v /= r;
            } else {
                v *= r;
            }
        }
        Some(v)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek()? {
            '-' | '−' => {
                self.at += 1;
                self.depth_guard(|p| p.unary()).map(|v| -v)
            }
            '+' => {
                self.at += 1;
                self.depth_guard(|p| p.unary())
            }
            '(' => {
                self.at += 1;
                let v = self.depth_guard(|p| p.sum())?;
                (self.peek() == Some(')')).then(|| self.at += 1)?;
                Some(v)
            }
            _ => self.number(),
        }
    }

    fn depth_guard(&mut self, f: impl FnOnce(&mut Self) -> Option<f64>) -> Option<f64> {
        if self.depth >= MAX_DEPTH {
            return None;
        }
        self.depth += 1;
        let v = f(self);
        self.depth -= 1;
        v
    }

    fn number(&mut self) -> Option<f64> {
        self.skip_spaces();
        let start = self.at;
        while self
            .chars
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || *c == '.')
        {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse()
            .ok()
            .filter(|_: &f64| text.matches('.').count() <= 1)
    }
}

/// Why a generated problem was turned down.
#[derive(Debug, PartialEq, Eq)]
pub enum Reject {
    /// No `expression` to check the answer against.
    NoExpression,
    /// The expression isn't plain arithmetic.
    BadExpression,
    /// It evaluates to something other than the `answer`.
    WrongAnswer,
    /// The problem text or a step is empty, or the answer isn't a number.
    Malformed,
}

/// Checks a generated problem before a child sees it: the answer must equal
/// its expression, and the text must be there.
pub fn verify(task: &MathTask) -> Result<(), Reject> {
    if task.problem.trim().is_empty() || !task.answer.is_finite() {
        return Err(Reject::Malformed);
    }
    if task.explanation.iter().any(|s| s.trim().is_empty()) {
        return Err(Reject::Malformed);
    }
    let src = task.expression.as_deref().ok_or(Reject::NoExpression)?;
    let value = eval(src).ok_or(Reject::BadExpression)?;
    let tolerance = 1e-6 * (1.0 + task.answer.abs());
    if (value - task.answer).abs() <= tolerance {
        Ok(())
    } else {
        Err(Reject::WrongAnswer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(answer: f64, expression: Option<&str>) -> MathTask {
        MathTask {
            topic: "t".into(),
            problem: "p".into(),
            answer,
            hint: String::new(),
            difficulty: 1,
            explanation: Vec::new(),
            expression: expression.map(str::to_string),
        }
    }

    #[test]
    fn arithmetic_follows_the_usual_rules() {
        let cases = [
            ("1+1", 2.0),
            ("(12 + 8) / 4", 5.0),
            ("2 + 3 * 4", 14.0),
            ("(2 + 3) * 4", 20.0),
            ("10 - 4 - 3", 3.0),
            ("24 / 4 / 3", 2.0),
            ("6 × 7", 42.0),
            ("48 ÷ 6", 8.0),
            ("9 − 4", 5.0),
            ("-3 + 5", 2.0),
            ("2 * -3", -6.0),
            ("--4", 4.0),
            ("3.5 * 2", 7.0),
            ("1/4 + 1/4", 0.5),
            ("((((1))))", 1.0),
            ("  7  ", 7.0),
        ];
        for (src, want) in cases {
            let got = eval(src).unwrap_or_else(|| panic!("{src} didn't evaluate"));
            assert!((got - want).abs() < 1e-9, "{src} = {got}, want {want}");
        }
    }

    #[test]
    fn anything_that_isnt_plain_arithmetic_is_rejected() {
        for src in [
            "",
            "+",
            "1 +",
            "(1 + 2",
            "1 + 2)",
            "1 2",
            "1..2",
            "1.2.3",
            ".",
            "2 ^ 3",
            "2 ** 3",
            "sqrt(4)",
            "1e3",
            "0x10",
            "3 x 4",
            "5 % 2",
            "1 / 0",
            "(1 - 1) / (2 - 2)",
            "abc",
            "1; 2",
            "3 = 3",
            "２＋２",
            "1,000 + 1",
        ] {
            assert_eq!(eval(src), None, "{src:?}");
        }
    }

    #[test]
    fn hostile_input_is_bounded() {
        // Deep nesting stops at MAX_DEPTH instead of overflowing the stack.
        let deep = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
        assert_eq!(eval(&deep), None);
        let negs = format!("{}1", "-".repeat(5000));
        assert_eq!(eval(&negs), None);
        // Just inside the limits still works.
        let ok = format!("{}1{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert_eq!(eval(&ok), Some(1.0));
        // Long input is refused outright.
        assert_eq!(eval(&"1+".repeat(MAX_LEN)), None);
        // Spaces never glue digits together.
        assert_eq!(eval("1 2"), None);
        assert_eq!(eval("1 2 + 3"), None);
    }

    #[test]
    fn a_problem_passes_only_if_its_answer_matches_its_expression() {
        assert_eq!(verify(&task(5.0, Some("(12 + 8) / 4"))), Ok(()));
        assert_eq!(verify(&task(0.5, Some("1/4 + 1/4"))), Ok(()));
        assert_eq!(verify(&task(1.0 / 3.0, Some("1/3"))), Ok(()));
        assert_eq!(
            verify(&task(6.0, Some("(12 + 8) / 4"))),
            Err(Reject::WrongAnswer)
        );
        assert_eq!(
            verify(&task(5.0, Some("2 ^ 3"))),
            Err(Reject::BadExpression)
        );
        assert_eq!(verify(&task(5.0, None)), Err(Reject::NoExpression));
        assert_eq!(verify(&task(f64::NAN, Some("1"))), Err(Reject::Malformed));
        assert_eq!(
            verify(&task(f64::INFINITY, Some("1"))),
            Err(Reject::Malformed)
        );
    }

    #[test]
    fn empty_text_is_malformed() {
        let mut t = task(2.0, Some("1+1"));
        t.problem = "  ".into();
        assert_eq!(verify(&t), Err(Reject::Malformed));
        let mut t = task(2.0, Some("1+1"));
        t.explanation = vec!["Add them.".into(), " ".into()];
        assert_eq!(verify(&t), Err(Reject::Malformed));
    }
}
